pub mod settings;

#[cfg(test)]
mod tests {
    #[test]
    fn no_js_engine_in_lockfile_or_manifest() {
        let toml = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml"));
        let lock = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../Cargo.lock"));
        for forbid in ["v8", "quickjs", "wry", "deno"] {
            assert!(!toml.contains(forbid), "{forbid} in tinycast Cargo.toml");
            assert!(
                !lock_has_package(lock, forbid),
                "{forbid} in workspace Cargo.lock"
            );
        }
    }

    fn lock_has_package(lock: &str, name: &str) -> bool {
        let needle = format!("name = \"{name}\"");
        lock.lines().any(|line| line.trim() == needle)
    }
}
