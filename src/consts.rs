pub const RESET: &str = "\x1B[0m";
pub const BLACK: &str = "\x1B[0;30m"; // Black
pub const RED: &str = "\x1B[0;31m"; // Red
pub const GREEN: &str = "\x1B[0;32m"; // Green
pub const YELLOW: &str = "\x1B[0;33m"; // Yellow
pub const BLUE: &str = "\x1B[0;34m"; // Blue
pub const PURPLE: &str = "\x1B[0;35m"; // Purple
pub const CYAN: &str = "\x1B[0;36m"; // Cyan
pub const WHITE: &str = "\x1B[0;37m"; // White

/// Move cusor up a line, erase it, and go to beginning
pub const ERASE: &str = "\x1b[1A\x1b[2K";
