//! Module for the boring formatting stuff of all the components

use crate::{
    addr::{PhysAddr, VirtAddr},
    page::PageTable,
    L1dCache, Machine, Tlb,
};

pub trait Draw {
    fn draw(&self) -> String;
}

impl Draw for Tlb {
    fn draw(&self) -> String {
        let mut empty = true;
        let mut buf = String::new();

        for (i, set) in self.sets.iter().enumerate() {
            if set.iter().any(|e| e.valid) {
                let mut first = true;
                for entry in set.iter() {
                    if entry.valid {
                        empty = false;
                        if !first {
                            buf += " ";
                        }
                        first = false;

                        let virt = (entry.tag | i as u64) << 12;

                        buf += &format!(
                            "[{} -> {}, {}]",
                            VirtAddr::from_bits(virt),
                            entry.addr,
                            entry.access
                        );
                    }
                }
                buf += "\n";
            }
        }

        if empty {
            buf += "(empty)";
        }

        buf
    }
}

impl Draw for L1dCache {
    fn draw(&self) -> String {
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

pub trait MachineDraw {
    fn draw_page_map(&self) -> String;
}

impl MachineDraw for Machine {
    fn draw_page_map(&self) -> String {
        fn inner(
            m: &Machine,
            buf: &mut String,
            depth: i32,
            phys_base: PhysAddr,
            virt_base: VirtAddr,
        ) {
            let table = m.memory.read::<PageTable>(phys_base);

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
                    let x = num_mapped_entries(m, phys);
                    *buf += &format!("{indent}{i:03}: [{x} mapped entries]\n");
                    inner(m, buf, depth + 1, phys, virt);
                }
            }
        }

        fn num_mapped_entries(m: &Machine, table_location: PhysAddr) -> usize {
            let table = m.memory.read::<PageTable>(table_location);
            table.iter().filter(|e| e.is_present()).count()
        }

        let mut buf = String::new();
        inner(&self, &mut buf, 1, self.cr3, VirtAddr::from_bits(0));
        buf
    }
}
