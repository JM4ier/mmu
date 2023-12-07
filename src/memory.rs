use crate::addr::PhysAddr;

pub struct Memory {
    memory: Vec<u8>,
}

impl Memory {
    pub fn megabytes(mb: usize) -> Self {
        Self {
            memory: vec![0; mb << 20],
        }
    }
    pub fn read<T: Copy>(&self, addr: PhysAddr) -> T {
        let a = addr.bits() as usize;
        let len = core::mem::size_of::<T>();

        if a + len > self.memory.len() {
            panic!("memory access out of bounds...");
        }

        let addr = (&self.memory[a]) as *const u8 as *const T;

        unsafe { *addr }
    }
    pub fn edit<T>(&mut self, addr: PhysAddr) -> &mut T {
        let a = addr.bits() as usize;
        let len = core::mem::size_of::<T>();

        if a + len > self.memory.len() {
            panic!("memory access out of bounds...");
        }

        let addr = (&mut self.memory[a]) as *mut u8 as *mut T;

        unsafe { addr.as_mut().unwrap() }
    }
}
