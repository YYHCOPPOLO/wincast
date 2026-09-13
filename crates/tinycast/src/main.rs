#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

fn main() {
    if let Err(e) = tinycast::run() {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
