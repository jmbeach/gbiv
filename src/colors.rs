pub const COLORS: [&str; 7] = ["red", "orange", "yellow", "green", "blue", "indigo", "violet"];

pub fn ansi_color(color: &str) -> &'static str {
    match color {
        "red" => "\x1b[31m",
        "orange" => "\x1b[38;5;208m",
        "yellow" => "\x1b[33m",
        "green" => "\x1b[32m",
        "blue" => "\x1b[34m",
        "indigo" => "\x1b[38;5;54m",
        "violet" => "\x1b[35m",
        _ => "\x1b[0m",
    }
}

pub const RESET: &str = "\x1b[0m";
pub const YELLOW: &str = "\x1b[33m";
pub const GREEN: &str = "\x1b[32m";
pub const RED: &str = "\x1b[31m";
pub const DIM: &str = "\x1b[2m";
