pub struct Log {
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
