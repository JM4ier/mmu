use std::{
    fmt::{format, write, Display},
    ops::{Add, Deref, DerefMut, Index, IndexMut},
};

#[derive(Copy, Clone)]
pub struct PhysAddr(u64);
impl Display for PhysAddr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "P0x{:x}", self.0)
    }
}
impl PhysAddr {
    pub fn from_frame_offset(frame: u64, offset: u64) -> Self {
        Self((frame << 12) | offset)
    }
    pub fn with_offset(mut self, offset: u64) -> Self {
        self.0 &= !4095;
        self.0 |= offset & 4095;
        self
    }
    pub const fn frame_offset(self) -> usize {
        self.0 as usize & 4095
    }
    pub const fn frame_number(self) -> u64 {
        self.0 >> 12
    }
}

#[derive(Copy, Clone)]
pub struct VirtAddr(u64);
impl Display for VirtAddr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "V0x{:x}", self.0)
    }
}

impl Add<u64> for VirtAddr {
    type Output = VirtAddr;

    fn add(mut self, rhs: u64) -> Self::Output {
        self.0 += rhs;
        self
    }
}

impl VirtAddr {
    pub const fn page_offset(self) -> u64 {
        self.0 & 4095
    }
    pub const fn virtual_page_number(self) -> u64 {
        self.0 >> 12
    }
    pub const fn vpn1(self) -> usize {
        ((self.0 >> 39) & 511) as usize
    }
    pub const fn vpn2(self) -> usize {
        ((self.0 >> 30) & 511) as usize
    }
    pub const fn vpn3(self) -> usize {
        ((self.0 >> 21) & 511) as usize
    }
    pub const fn vpn4(self) -> usize {
        ((self.0 >> 12) & 511) as usize
    }
}

#[derive(Copy, Clone)]
pub struct PageTableEntry {
    bits: u64,
}

impl PageTableEntry {
    pub const fn new_present(target: PhysAddr) -> Self {
        Self {
            bits: target.0 & 0x000FFFFFFFFFFFF000 | 1,
        }
    }

    pub const fn new_unmapped() -> Self {
        Self { bits: 0 }
    }

    pub const fn is_present(self) -> bool {
        self.bits & 1 > 0
    }

    pub const fn phys_addr(self) -> PhysAddr {
        PhysAddr(self.bits & 0x000FFFFFFFFFFFF000)
    }
}

#[derive(Copy, Clone)]
pub struct PageTable {
    table: [PageTableEntry; 512],
}
impl Deref for PageTable {
    type Target = [PageTableEntry; 512];
    fn deref(&self) -> &Self::Target {
        &self.table
    }
}
impl DerefMut for PageTable {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.table
    }
}

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
    pub fn display(&self) -> String {
        let mut buf = String::new();
        for i in 0..64 {
            if self.entries[i].has_entry() {
                buf += &format!("{i:02}:");
                for e in self.entries[i].entries.iter() {
                    if e.valid {
                        let phys = PhysAddr::from_frame_offset(e.tag, (i as u64) << 6);
                        buf += &format!(" {phys}");
                    }
                }
                buf += "\n";
            }
        }
        buf
    }
}

pub struct Memory {
    memory: Vec<u8>,
}

impl Memory {
    fn megabytes(mb: usize) -> Self {
        Self {
            memory: vec![0; mb << 20],
        }
    }
    fn read<T: Copy>(&self, addr: PhysAddr) -> T {
        let a = addr.0 as usize;
        let len = core::mem::size_of::<T>();

        if a + len > self.memory.len() {
            panic!("memory access out of bounds...");
        }

        let addr = (&self.memory[a]) as *const u8 as *const T;

        unsafe { *addr }
    }
    fn mutate<T>(&mut self, addr: PhysAddr) -> &mut T {
        let a = addr.0 as usize;
        let len = core::mem::size_of::<T>();

        if a + len > self.memory.len() {
            panic!("memory access out of bounds...");
        }

        let addr = (&mut self.memory[a]) as *mut u8 as *mut T;

        unsafe { addr.as_mut().unwrap() }
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
    pub const fn invalid() -> Self {
        Self {
            valid: false,
            access: 0,
            tag: 0,
            addr: PhysAddr(0),
        }
    }
}

pub type TlbEntrySet = [TlbEntry; 4];

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
            sets: [[TlbEntry::invalid(); 4]; 128],
        }
    }
}

impl Display for Tlb {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut empty = true;

        for (i, set) in self.sets.iter().enumerate() {
            if set.iter().any(|e| e.valid) {
                let mut first = true;
                for entry in set.iter() {
                    if entry.valid {
                        empty = false;
                        if !first {
                            write!(f, " ")?;
                        }
                        first = false;

                        let virt = (entry.tag | i as u64) << 12;

                        write!(
                            f,
                            "[{} -> {}, {}]",
                            VirtAddr(virt),
                            entry.addr,
                            entry.access
                        )?;
                    }
                }
                write!(f, "\n")?;
            }
        }

        if empty {
            write!(f, "(empty)")?;
        }
        Ok(())
    }
}

pub struct CacheStats {
    hit: usize,
    miss: usize,
}
impl CacheStats {
    fn new() -> Self {
        Self { hit: 0, miss: 0 }
    }
    fn hit(&mut self) {
        self.hit += 1;
    }
    fn miss(&mut self) {
        self.miss += 1;
    }
}

pub struct Stats {
    page_faults: usize,
    l1: CacheStats,
    tlb: CacheStats,
}
impl Stats {
    fn new() -> Self {
        Self {
            page_faults: 0,
            l1: CacheStats::new(),
            tlb: CacheStats::new(),
        }
    }
    fn reset(&mut self) {
        *self = Self::new();
    }
}

struct Log {
    enable: bool,
    depth: usize,
}
impl Log {
    pub fn new() -> Self {
        Self {
            enable: true,
            depth: 0,
        }
    }
    pub fn log(&self, msg: impl ToString) {
        if self.enable {
            println!("{}{}", "  ".repeat(self.depth), msg.to_string());
        }
    }
    pub fn begin_context(&mut self) {
        self.depth += 1;
    }
    pub fn end_context(&mut self) {
        self.depth -= 1;
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

        self.stats.page_faults += 1;

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

        self.stats.page_faults -= 1;

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
            let evicted = VirtAddr((tlb_set[k].tag | tlb_index) << 12);
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
            let evicted = PhysAddr(cache_set[k].tag << 12 | (index as u64) << 6);
            self.log.log(format!("Evicting L1 Entry: {evicted}"));
        }

        let block_addr = PhysAddr(addr.0 & !63);

        cache_set[k].valid = true;
        cache_set[k].tag = tag;
        cache_set[k].line = self.memory.read(block_addr);

        self.log.log(format!("Loaded {block_addr} into cache."));

        cache_set[k].line[offset]
    }

    pub fn read(&mut self, addr: VirtAddr) -> Result<u8, PageFault> {
        self.log.log(format!("Memory Access at {addr}"));
        self.log.begin_context();

        let phys_addr = self.translate(addr)?;
        self.log.log(format!("Found physical address {phys_addr}"));

        let byte = self.read_phys(phys_addr);

        self.log.end_context();
        Ok(byte)
    }

    pub fn invalidate_tlb(&mut self) {
        self.log.log("Invalidate TLB");
        self.tlb = Tlb::empty();
    }

    pub fn map_page(
        &mut self,
        table_location: PhysAddr,
        table_entry: usize,
        target_frame: PhysAddr,
    ) {
        self.log.log(format!("Page-Table Edit at address {table_location}: Mapping entry {table_entry:03} to {target_frame}"));
        let table = self.memory.mutate::<PageTable>(table_location);
        table[table_entry] = PageTableEntry::new_present(target_frame);
    }

    pub fn unmap_page(&mut self, table_location: PhysAddr, table_entry: usize) {
        self.log.log(format!(
            "Page-Table Edit at address {table_location}: Unmapping entry {table_entry:03}"
        ));
        let table = self.memory.mutate::<PageTable>(table_location);
        table[table_entry] = PageTableEntry::new_unmapped();
    }
}

// pretty printing
impl Machine {
    pub fn page_map(&self) -> String {
        let mut buf = String::new();
        self.page_map_rec(&mut buf, 1, self.cr3, VirtAddr(0));
        buf
    }
    fn num_mapped_entries(&self, table_location: PhysAddr) -> usize {
        let table = self.memory.read::<PageTable>(table_location);
        table.iter().filter(|e| e.is_present()).count()
    }
    fn page_map_rec(&self, buf: &mut String, depth: i32, phys_base: PhysAddr, virt_base: VirtAddr) {
        let table = self.memory.read::<PageTable>(phys_base);

        let indent = " | ".repeat(depth as usize - 1);
        let stride = 4096 << (9 * (4 - depth));

        for (i, entry) in table.iter().enumerate() {
            if !entry.is_present() {
                continue;
            }

            let virt = virt_base + stride * i as u64;
            let phys = entry.phys_addr();
            if depth == 4 {
                *buf += &(format!("{indent}{i:03}: {virt} -> {phys}\n"));
            } else {
                let x = self.num_mapped_entries(phys);
                *buf += &format!("{indent}{i:03}: [{x} mapped entries]\n");
                self.page_map_rec(buf, depth + 1, phys, virt);
            }
        }
    }
}

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
        println!("{}", boxed("Pages", &self.page_map()));
        println!("{}", boxed("TLB", &format!("{}", self.tlb)));
        println!("{}", boxed("L1-Cache", &self.cache.display()));
        println!("{}", boxed("Stats", &self.stats()));
        self.stats.reset();
    }
}

fn boxed(title: &str, content: &str) -> String {
    let lines: Vec<_> = content.lines().collect();
    let width = lines
        .iter()
        .map(|l| l.len())
        .max()
        .unwrap_or(0)
        .max(4 + title.len());
    let mut buf = String::new();

    let width = width + 1;

    buf += "╭─";
    buf += title;
    for _ in 0..(width - title.len()) {
        buf += "─";
    }
    buf += "╮\n";

    for line in lines {
        buf += "│ ";
        buf += line;
        for _ in 0..(width - line.len()) {
            buf += " ";
        }
        buf += "│\n";
    }
    buf += "╰";
    for _ in 0..=width {
        buf += "─";
    }
    buf += "╯";

    buf
}

#[derive(Copy, Clone)]
pub enum Action {
    Map {
        table: PhysAddr,
        index: usize,
        target: PhysAddr,
    },
    UnMap {
        table: PhysAddr,
        index: usize,
    },
    InvalidateTlb,
    Read(VirtAddr),
    DumpStats,
}
impl Machine {
    pub fn run_one(&mut self, action: Action) {
        match action {
            Action::Map {
                table,
                index,
                target,
            } => {
                self.map_page(table, index, target);
            }
            Action::UnMap { table, index } => self.unmap_page(table, index),
            Action::Read(addr) => {
                self.read(addr);
            }
            Action::DumpStats => {
                self.dump_stats();
            }
            Action::InvalidateTlb => self.invalidate_tlb(),
        }
    }
    pub fn run_many(&mut self, actions: &[Action]) {
        for action in actions {
            self.run_one(*action);
        }
    }
}

fn main() {
    let mut allocator = 99;
    let mut next_page = || {
        allocator += 1;
        PhysAddr(4096 * allocator)
    };

    let mut mmu = Machine {
        cr3: next_page(),
        tlb: Tlb::empty(),
        memory: Memory::megabytes(200),
        cache: L1dCache::empty(),
        stats: Stats::new(),
        log: Log::new(),
    };

    let p1 = next_page();
    let p2 = next_page();
    let p3 = next_page();
    let p4 = next_page();
    let p5 = next_page();
    let p6 = next_page();
    let p7 = next_page();
    let p8 = next_page();
    let p9 = next_page();

    use Action::*;
    let actions = [
        Map {
            table: mmu.cr3,
            index: 10,
            target: p1,
        },
        Map {
            table: p1,
            index: 0,
            target: p2,
        },
        // we map p2[0] and p2[1] to p3 to simulate homonyms
        Map {
            table: p2,
            index: 0,
            target: p3,
        },
        Map {
            table: p2,
            index: 1,
            target: p3,
        },
        Map {
            table: p3,
            index: 0,
            target: p4,
        },
        Map {
            table: p3,
            index: 1,
            target: p5,
        },
        Map {
            table: p3,
            index: 2,
            target: p6,
        },
        Map {
            table: mmu.cr3,
            index: 32,
            target: p7,
        },
        Map {
            table: p7,
            index: 0,
            target: p8,
        },
        Map {
            table: p8,
            index: 0,
            target: p9,
        },
        Map {
            table: p9,
            index: 200,
            target: next_page(),
        },
        Map {
            table: p9,
            index: 201,
            target: next_page(),
        },
        Map {
            table: p9,
            index: 202,
            target: next_page(),
        },
        Map {
            table: p9,
            index: 203,
            target: next_page(),
        },
        Map {
            table: p9,
            index: 204,
            target: next_page(),
        },
        Map {
            table: p9,
            index: 205,
            target: next_page(),
        },
        Map {
            table: p9,
            index: 206,
            target: next_page(),
        },
        Map {
            table: p9,
            index: 207,
            target: next_page(),
        },
        InvalidateTlb,
        Read(VirtAddr(0x50000000000)),
        Read(VirtAddr(0x50000202200)),
        Read(VirtAddr(0x50000202200)),
        Read(VirtAddr(0x50000202200)),
        Read(VirtAddr(0x50000202200)),
        Read(VirtAddr(0x50000202200)),
        Read(VirtAddr(0x50000202200)),
        Read(VirtAddr(0x1000000c8000)),
        Read(VirtAddr(0x1000000c8000 + 64)),
        Read(VirtAddr(0x1000000c8000 + 2 * 64)),
        Read(VirtAddr(0x1000000c8100)),
        Read(VirtAddr(0x1000000c8200)),
        Read(VirtAddr(0x1000000c9000)),
        Read(VirtAddr(0x1000000ca000)),
        Read(VirtAddr(0x1000000cb000)),
        Read(VirtAddr(0x1000000cc000)),
        Read(VirtAddr(0x1000000cd000)),
        Read(VirtAddr(0x1000000ce000)),
        Read(VirtAddr(0x1000000cf000)),
        DumpStats,
    ];

    mmu.run_many(&actions);
}
