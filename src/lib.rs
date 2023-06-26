use anyhow::Result;
use consts::{BLUE, ERASE, GREEN, RED, YELLOW};
use std::{
    io::{self, Write},
    thread,
    time::Duration,
};
use walkdir::DirEntry;

pub mod consts;
pub mod project;

#[macro_export]
macro_rules! color {
    ($color:ident, $($args:expr),* $(,)?) => {
        format!("{}{}{}", $color, format!($($args),*), $crate::consts::RESET)
    };
}

#[macro_export]
macro_rules! output {
    ($stream:ident, $($args:expr),* $(,)?) => {
        {
            use ::std::io::Write;
            write!(
                $stream,
                "{purple}=> {reset}{}",
                format!($($args),*),
                purple = $crate::consts::PURPLE,
                reset = $crate::consts::RESET,
            )
        }
    };

    ($($args:expr),* $(,)?) => {
        println!(
            "{purple}=> {reset}{}",
            format!($($args),*),
            purple = $crate::consts::PURPLE,
            reset = $crate::consts::RESET,
         )
    };
}

pub fn to_memory_string(bytes: Option<u64>) -> String {
    match bytes {
        Some(bytes) if bytes >= 1_000_000_000 => color!(RED, "{:3} GB", bytes / 1_000_000_000),
        Some(bytes) if bytes >= 1_000_000 => color!(BLUE, "{:3} MB", bytes / 1_000_000),
        Some(bytes) if bytes >= 1_000 => color!(GREEN, "{:3} KB", bytes / 1_000),
        Some(bytes) =>
        // One extra space between the letters and B because the other units
        // have G/M/B
        {
            format!("{bytes:3}  B")
        }
        None => color!(YELLOW, "N/A --"),
    }
}

pub fn is_hidden(entry: &DirEntry) -> bool {
    entry
        .file_name()
        .to_str()
        .map(|s| s.starts_with("."))
        .unwrap_or(false)
}

pub fn wait(millis: u64) {
    thread::sleep(Duration::from_millis(millis));
}

/// Print and flush stdout (avoids line-buffering issue)
pub fn print(contents: &str) {
    let mut out = io::stdout();
    let _ = out.write_all(contents.as_bytes());
    let _ = out.flush();
}

pub fn erase() -> Result<()> {
    let mut out = io::stdout();
    out.write_all(ERASE.as_bytes())?;
    out.flush()?;
    Ok(())
}
