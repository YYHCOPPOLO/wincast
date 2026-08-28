use std::path::PathBuf;

const BUNDLE_ID: &str = "com.tinycast.win";

pub fn roaming_dir() -> PathBuf {
    known_dir("APPDATA")
}

pub fn local_dir() -> PathBuf {
    known_dir("LOCALAPPDATA")
}

fn known_dir(var: &str) -> PathBuf {
    std::env::var_os(var)
        .map(PathBuf::from)
        .unwrap_or_default()
        .join(BUNDLE_ID)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dirs_use_bundle_id() {
        assert_eq!(
            roaming_dir().file_name().and_then(|n| n.to_str()),
            Some("com.tinycast.win")
        );
        assert_eq!(
            local_dir().file_name().and_then(|n| n.to_str()),
            Some("com.tinycast.win")
        );
    }
}
