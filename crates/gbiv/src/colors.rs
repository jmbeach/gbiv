// @spec CLI-COLOR-002 through CLI-COLOR-009, CLI-COLOR-017
// The `_ => RESET` arm also covers extra (non-base) palette names: extras are
// labels, not hues, so they render with no color (CLI-COLOR-017).
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

// @spec CLI-COLOR-010
pub const RESET: &str = "\x1b[0m";
// @spec CLI-COLOR-012
pub const YELLOW: &str = "\x1b[33m";
// @spec CLI-COLOR-013
pub const GREEN: &str = "\x1b[32m";
// @spec CLI-COLOR-014
pub const RED: &str = "\x1b[31m";
// @spec CLI-COLOR-011
pub const DIM: &str = "\x1b[2m";
