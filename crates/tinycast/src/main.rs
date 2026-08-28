mod app_core;
mod app_settings;
mod features;
mod palette;
mod platform;
mod surfaces;

fn main() {
    if let Err(e) = platform::run() {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
