use std::ops::{Deref, DerefMut};
use crate::addr::PhysAddr;

#[derive(Copy, Clone)]
pub struct PageTableEntry {
    bits: u64,
}

impl PageTableEntry {
    pub const fn new_present(target: PhysAddr) -> Self {
        Self {
            bits: target.bits() & 0x000FFFFFFFFFFFF000 | 1,
        }
    }

    pub const fn new_unmapped() -> Self {
        Self { bits: 0 }
    }

    pub const fn is_present(self) -> bool {
        self.bits & 1 > 0
    }

    pub const fn phys_addr(self) -> PhysAddr {
        PhysAddr::from_bits(self.bits & 0x000FFFFFFFFFFFF000)
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
