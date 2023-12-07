use std::fmt::Display;

pub fn print_box(title: &str, content: impl Display) {
    let content = format!("{}", content);
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

    println!("{buf}");
}
