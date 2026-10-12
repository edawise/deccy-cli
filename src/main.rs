//! The binary: a thin `fn main` over the crate's assembled entry.
//!
//! The entry lives at the crate root (`deccy_cli::run`) so a test drives the same path production
//! runs; this file only collects argv past the program name and returns the status.

fn main() {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    std::process::exit(deccy_cli::run(&argv));
}
