# mmu

This  is a basic simulator for virtual memory on X86_64.

It is loosely based on the Sandy Bridge MMU, as described [here](https://www.realworldtech.com/sandy-bridge/7/)

## Usage

We can easily set up paging on a per-table granularity:

```rs
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

mmu.dump_stats();
```

If we print the mmu stats now, we get a nice visualization:
```
╭─Pages────────────────────────────────────╮
│ 010: [1 mapped entries]                  │
│  | 000: [2 mapped entries]               │
│  |  | 000: [10 mapped entries]           │
│  |  |  | 000: V0x50000000000 -> P0x68000 │
│  |  |  | 001: V0x50000001000 -> P0x69000 │
│  |  |  | 002: V0x50000002000 -> P0x6a000 │
│  |  |  | 003: V0x50000003000 -> P0x6b000 │
│  |  |  | 004: V0x50000004000 -> P0x6c000 │
│  |  |  | 005: V0x50000005000 -> P0x6d000 │
│  |  |  | 006: V0x50000006000 -> P0x6e000 │
│  |  |  | 007: V0x50000007000 -> P0x6f000 │
│  |  |  | 008: V0x50000008000 -> P0x70000 │
│  |  |  | 009: V0x50000009000 -> P0x71000 │
│  |  | 001: [10 mapped entries]           │
│  |  |  | 000: V0x50000200000 -> P0x68000 │
│  |  |  | 001: V0x50000201000 -> P0x69000 │
│  |  |  | 002: V0x50000202000 -> P0x6a000 │
│  |  |  | 003: V0x50000203000 -> P0x6b000 │
│  |  |  | 004: V0x50000204000 -> P0x6c000 │
│  |  |  | 005: V0x50000205000 -> P0x6d000 │
│  |  |  | 006: V0x50000206000 -> P0x6e000 │
│  |  |  | 007: V0x50000207000 -> P0x6f000 │
│  |  |  | 008: V0x50000208000 -> P0x70000 │
│  |  |  | 009: V0x50000209000 -> P0x71000 │
╰──────────────────────────────────────────╯
```

Simulating memory accesses is also rather easy.

As a first example, if we access memory contiguously we can see how that treats the cache nicely:
```rs
let addr = VirtAddr::from_bits(0x50000200000);
for i in 0..100 {
    mmu.read(addr + i).ok();
}
mmu.dump_stats();
```
```
╭─TLB───────────────────────────────╮
│ [V0x50000200000 -> P0x68000, 100] │
╰───────────────────────────────────╯
╭─L1-Cache─────╮
│ 00: P0x68000 │
│ 01: P0x68040 │
╰──────────────╯
╭─Stats───────────╮
│ TLB hits:    99 │
│ TLB misses:  1  │
│ L1 hits:     98 │
│ L1 misses:   2  │
│ Page Faults: 0  │
╰─────────────────╯
```

However, if we access memory locations that are sufficiently far apart, the data will never be in our cache:
```rs
let addr = VirtAddr::from_bits(0x50000200000);
for i in 0..100 {
    mmu.read(addr + 64 * i).ok();
}
mmu.dump_stats();
```
```
╭─TLB──────────────────────────────╮
│ [V0x50000200000 -> P0x68000, 64] │
│ [V0x50000201000 -> P0x69000, 36] │
╰──────────────────────────────────╯
╭─L1-Cache──────────────╮
│ [omitted for brevity) │
╰───────────────────────╯
╭─Stats────────────╮
│ TLB hits:    98  │
│ TLB misses:  2   │
│ L1 hits:     0   │
│ L1 misses:   100 │
│ Page Faults: 0   │
╰──────────────────╯
```
