use std::{
    fmt::Display,
    ops::{Add, Deref, DerefMut, Index, IndexMut},
};

#[derive(Copy, Clone)]
pub struct PhysAddr(u64);
impl Display for PhysAddr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "P0x{:x}", self.0)
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
    pub fn is_present(self) -> bool {
        self.bits & 1 > 0
    }
    pub fn phys_addr(self) -> PhysAddr {
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
    pub fn invalidate(&mut self) {
        *self = Self::empty();
    }
}

pub struct Machine {
    cr3: PhysAddr,
    tlb: Tlb,
    memory: Memory,
    cache: L1dCache,
}

pub struct PageFault;

impl Machine {
    pub fn translate(&mut self, virt_addr: VirtAddr) -> Result<PhysAddr, PageFault> {
        let tlb_index = virt_addr.virtual_page_number() & 127;
        let tlb_tag = virt_addr.virtual_page_number() >> 7;
        let tlb_set = &mut self.tlb[tlb_index];

        for i in 0..4 {
            if tlb_set[i].tag == tlb_tag && tlb_set[i].valid {
                tlb_set[i].mark_accessed();
                return Ok(tlb_set[i].addr);
            }
        }

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

        // replace old entry
        tlb_set[k].valid = true;
        tlb_set[k].addr = phys_addr;
        tlb_set[k].tag = tlb_tag;

        Ok(phys_addr)
    }

    pub fn map_page(
        &mut self,
        table_location: PhysAddr,
        table_entry: usize,
        target_frame: PhysAddr,
    ) {
        let table = self.memory.mutate::<PageTable>(table_location);
        table[table_entry] = PageTableEntry::new_present(target_frame);
        self.tlb.invalidate();
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
                *buf += &(format!("{indent}{i:03}: {virt} --> {phys}\n"));
            } else {
                let x = self.num_mapped_entries(phys);
                *buf += &format!("{indent}{i:03}: [{x} mapped entries]\n");
                self.page_map_rec(buf, depth + 1, phys, virt);
            }
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
    };

    let p1 = next_page();
    let p2 = next_page();
    let p3 = next_page();
    let p4 = next_page();
    let p5 = next_page();
    let p6 = next_page();

    mmu.map_page(mmu.cr3, 10, p1);
    mmu.map_page(p1, 0, p2);

    // mapping both p2.0 --> p3 and p2.1 --> p3 (homonyms)
    mmu.map_page(p2, 0, p3);
    mmu.map_page(p2, 1, p3);

    mmu.map_page(p3, 0, p4);
    mmu.map_page(p3, 1, p5);
    mmu.map_page(p3, 2, p6);
    mmu.map_page(mmu.cr3, 24, next_page());

    println!("{}", mmu.page_map());
}
