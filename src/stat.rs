pub struct CacheStats {
    pub hit: usize,
    pub miss: usize,
}
impl CacheStats {
    fn new() -> Self {
        Self { hit: 0, miss: 0 }
    }
    pub fn hit(&mut self) {
        self.hit += 1;
    }
    pub fn miss(&mut self) {
        self.miss += 1;
    }
}

pub struct Stats {
    pub page_faults: usize,
    pub l1: CacheStats,
    pub tlb: CacheStats,
}
impl Stats {
    pub fn new() -> Self {
        Self {
            page_faults: 0,
            l1: CacheStats::new(),
            tlb: CacheStats::new(),
        }
    }
    pub fn reset(&mut self) {
        *self = Self::new();
    }
}
