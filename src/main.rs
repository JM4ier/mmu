use std::ops::{Index, IndexMut};

mod addr;
use addr::{PhysAddr, VirtAddr};

mod page;
use page::{PageTable, PageTableEntry};

mod memory;
use memory::Memory;

mod log;
use log::Log;

mod cli;

mod draw;
use draw::{Draw, MachineDraw};
use stat::Stats;

mod stat;

#[derive(Copy, Clone)]
pub struct L1dCacheEntry {
    valid: bool,
    tag: u64,
    line: [u8; 64],
}

#[derive(Copy, Clone)]
pub struct L1dCacheSet {
    entries: [L1dCacheEntry; 8],
}

impl L1dCacheSet {
    fn has_entry(&self) -> bool {
        self.entries.iter().any(|e| e.valid)
    }
}

impl Index<usize> for L1dCacheSet {
    type Output = L1dCacheEntry;

    fn index(&self, index: usize) -> &Self::Output {
        &self.entries[index]
    }
}
impl IndexMut<usize> for L1dCacheSet {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.entries[index]
    }
}

/// An L1 Data Cache:
/// - 32KB in size
/// - 64 sets
/// - 8-way associative
/// - 64 bytes per block
pub struct L1dCache {
    entries: [L1dCacheSet; 64],
}

impl L1dCacheEntry {
    pub const fn empty() -> Self {
        Self {
            valid: false,
            tag: 0,
            line: [0; 64],
        }
    }
}
impl L1dCacheSet {
    pub const fn empty() -> Self {
        Self {
            entries: [L1dCacheEntry::empty(); 8],
        }
    }
}
impl L1dCache {
    pub const fn empty() -> Self {
        Self {
            entries: [L1dCacheSet::empty(); 64],
        }
    }
}

#[derive(Copy, Clone)]
pub struct TlbEntry {
    valid: bool,
    access: u8,
    tag: u64,
    addr: PhysAddr,
}

impl TlbEntry {
    pub fn mark_accessed(&mut self) {
        self.access = self.access.saturating_add(1);
    }

    pub const fn new() -> Self {
        Self {
            valid: false,
            access: 0,
            tag: 0,
            addr: PhysAddr::from_bits(0),
        }
    }
}

pub type TlbEntrySet = [TlbEntry; 4];


/// A Translation Lookaside Buffer - basically a small cache for address translations
/// - 128 sets
/// - 4-way associative
pub struct Tlb {
    sets: [TlbEntrySet; 128],
}
impl Index<u64> for Tlb {
    type Output = TlbEntrySet;

    fn index(&self, index: u64) -> &Self::Output {
        &self.sets[index as usize]
    }
}
impl IndexMut<u64> for Tlb {
    fn index_mut(&mut self, index: u64) -> &mut Self::Output {
        &mut self.sets[index as usize]
    }
}
impl Tlb {
    pub const fn empty() -> Self {
        Self {
            sets: [[TlbEntry::new(); 4]; 128],
        }
    }
}

pub struct Machine {
    cr3: PhysAddr,
    tlb: Tlb,
    memory: Memory,
    cache: L1dCache,
    stats: Stats,
    log: Log,
}

#[derive(Copy, Clone, Debug)]
pub struct PageFault;

impl Machine {
    /// simulate an address translation
    pub fn translate(&mut self, virt_addr: VirtAddr) -> Result<PhysAddr, PageFault> {
        let tlb_index = virt_addr.virtual_page_number() & 127;
        let tlb_tag = (virt_addr.virtual_page_number() >> 7) << 7;
        let tlb_set = &mut self.tlb[tlb_index];
        let page_offset = virt_addr.page_offset();

        for i in 0..4 {
            if tlb_set[i].tag == tlb_tag && tlb_set[i].valid {
                tlb_set[i].mark_accessed();
                self.stats.tlb.hit();
                self.log.log("TLB Hit");
                return Ok(tlb_set[i].addr.with_offset(page_offset));
            }
        }

        self.stats.tlb.miss();
        self.log.log("TLB Miss");

        let addr1 = self.cr3;
        let page_table_1 = self.memory.read::<PageTable>(addr1);
        let pte1 = page_table_1[virt_addr.vpn1()];
        if !pte1.is_present() {
            return Err(PageFault);
        }

        let addr2 = pte1.phys_addr();
        let page_table_2 = self.memory.read::<PageTable>(addr2);
        let pte2 = page_table_2[virt_addr.vpn2()];
        if !pte2.is_present() {
            return Err(PageFault);
        }

        let addr3 = pte2.phys_addr();
        let page_table_3 = self.memory.read::<PageTable>(addr3);
        let pte3 = page_table_3[virt_addr.vpn3()];
        if !pte3.is_present() {
            return Err(PageFault);
        }

        let addr4 = pte3.phys_addr();
        let page_table_4 = self.memory.read::<PageTable>(addr4);
        let pte4 = page_table_4[virt_addr.vpn4()];
        if !pte4.is_present() {
            return Err(PageFault);
        }

        let phys_addr = pte4.phys_addr();

        let mut k = 0;
        for i in 0..4 {
            if !tlb_set[i].valid {
                // if not valid, chose this entry
                k = i;
                break;
            }

            // choose entry with least accesses
            if tlb_set[i].access < tlb_set[k].access {
                k = i;
            }
        }

        for i in 0..4 {
            // reset access counter
            tlb_set[i].access = 0;
        }

        if tlb_set[k].valid {
            let evicted = VirtAddr::from_bits((tlb_set[k].tag | tlb_index) << 12);
            self.log.log(format!("Evicting TLB Entry {evicted}"));
        }

        // replace old entry
        tlb_set[k].valid = true;
        tlb_set[k].addr = phys_addr;
        tlb_set[k].tag = tlb_tag;
        tlb_set[k].mark_accessed();

        self.log.log(format!("New TLB Entry: {virt_addr}"));

        Ok(phys_addr.with_offset(page_offset))
    }

    /// simulate an access of physical memory
    pub fn read_phys(&mut self, addr: PhysAddr) -> u8 {
        let offset = addr.frame_offset() & 63;
        let index = (addr.frame_offset() >> 6) & 63;
        let tag = addr.frame_number();

        let cache_set = &mut self.cache.entries[index];

        // cache associativity (imagine this happens in parallel)
        for i in 0..8 {
            if cache_set[i].valid && cache_set[i].tag == tag {
                self.stats.l1.hit();
                self.log.log("Cache Hit");
                return cache_set[i].line[offset];
            }
        }

        self.stats.l1.miss();
        self.log.log("Cache Miss");

        // none is the value we need
        // --> evict some entry
        // TODO: better eviction strategy

        let mut k = 0;
        for i in 0..8 {
            if !cache_set[i].valid {
                k = i;
            }
        }

        if cache_set[k].valid {
            let evicted = PhysAddr::from_bits(cache_set[k].tag << 12 | (index as u64) << 6);
            self.log.log(format!("Evicting L1 Entry: {evicted}"));
        }

        let block_addr = addr & !63;

        cache_set[k].valid = true;
        cache_set[k].tag = tag;
        cache_set[k].line = self.memory.read(block_addr);

        self.log.log(format!("Loaded {block_addr} into cache."));

        cache_set[k].line[offset]
    }

    /// simulate an access of virtual memory
    pub fn read(&mut self, addr: VirtAddr) -> Result<u8, PageFault> {
        self.log.log(format!("Memory Access at {addr}"));
        self.log.begin_context();

        if let Ok(phys_addr) = self.translate(addr) {
            self.log.log(format!("Found physical address {phys_addr}"));

            let byte = self.read_phys(phys_addr);
            self.log.end_context();
            Ok(byte)
        } else {
            self.stats.page_faults += 1;
            self.log.log("Page Fault.");
            self.log.end_context();
            Err(PageFault)
        }
    }

    /// remove all TLB entries
    pub fn invalidate_tlb(&mut self) {
        self.log.log("Invalidate TLB");
        self.tlb = Tlb::empty();
    }

    pub fn invalidate_cache(&mut self) {
        self.log.log("Invalidate Cache");
        self.cache = L1dCache::empty();
    }

    /// Adds a new mapping to the page table at the given location
    pub fn map_page(
        &mut self,
        table_location: PhysAddr,
        table_entry: usize,
        target_frame: PhysAddr,
    ) {
        self.log.log(format!("Page-Table Edit at address {table_location}: Mapping entry {table_entry:03} to {target_frame}"));
        let table = self.memory.edit::<PageTable>(table_location);
        table[table_entry] = PageTableEntry::new_present(target_frame);
    }

    /// removes a mapping from the table at the given location
    pub fn unmap_page(&mut self, table_location: PhysAddr, table_entry: usize) {
        self.log.log(format!(
            "Page-Table Edit at address {table_location}: Unmapping entry {table_entry:03}"
        ));
        let table = self.memory.edit::<PageTable>(table_location);
        table[table_entry] = PageTableEntry::new_unmapped();
    }
}

// stats
impl Machine {
    fn stats(&self) -> String {
        format!(
            "TLB hits:    {}\nTLB misses:  {}\nL1 hits:     {}\nL1 misses:   {}\nPage Faults: {}",
            self.stats.tlb.hit,
            self.stats.tlb.miss,
            self.stats.l1.hit,
            self.stats.l1.miss,
            self.stats.page_faults
        )
    }

    fn dump_stats(&mut self) {
        cli::print_box("Pages", self.draw_page_map());
        cli::print_box("TLB", self.tlb.draw());
        cli::print_box("L1-Cache", self.cache.draw());
        cli::print_box("Stats", self.stats());
        self.stats.reset();
    }
}

fn main() {
    let mut allocator = 99;
    let mut next_frame = || {
        allocator += 1;
        PhysAddr::from_bits(4096 * allocator)
    };

    let mut mmu = Machine {
        cr3: next_frame(),
        tlb: Tlb::empty(),
        memory: Memory::megabytes(200),
        cache: L1dCache::empty(),
        stats: Stats::new(),
        log: Log::new(),
    };

    let f1 = next_frame();
    let f2 = next_frame();
    let f3 = next_frame();

    mmu.map_page(mmu.cr3, 10, f1);
    mmu.map_page(f1, 0, f2);

    // mapping the same page twice to see what happens with homonyms
    mmu.map_page(f2, 0, f3);
    mmu.map_page(f2, 1, f3);

    for i in 0..10 {
        mmu.map_page(f3, i, next_frame());
    }

    mmu.invalidate_tlb();

    let addr = VirtAddr::from_bits(0x50000200000);

    // consecutive accesses should be very good for cache & TLB
    for i in 0..100 {
        mmu.read(addr + i).ok();
    }

    mmu.dump_stats();

    mmu.invalidate_tlb();
    mmu.invalidate_cache();

    // accesses with stride 64 should be rather bad for the cache...
    for i in 0..100 {
        mmu.read(addr + 64 * i).ok();
    }

    mmu.dump_stats();
}
