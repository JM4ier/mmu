use std::{fmt::Display, ops::{Add, BitAnd}};

#[derive(Copy, Clone)]
pub struct PhysAddr(u64);
impl Display for PhysAddr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "P0x{:x}", self.0)
    }
}
impl PhysAddr {
    pub const fn from_bits(bits: u64) -> Self {
        Self(bits)
    }
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
    pub const fn bits(self) -> u64 {
        self.0
    }
}

impl BitAnd<u64> for PhysAddr {
    type Output = Self;

    fn bitand(mut self, rhs: u64) -> Self::Output {
        self.0 &= rhs;
        self
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
    pub const fn from_bits(bits: u64) -> Self {
        Self(bits)
    }
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

