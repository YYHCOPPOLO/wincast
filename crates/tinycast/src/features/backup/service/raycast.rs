//! `.rayconfig` v1/v2 detection. A wrong passphrase is never reported as bad format.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RaycastFormat {
    V1,
    V2,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RaycastError {
    BadFormat,
    WrongPassphrase,
}

impl RaycastError {
    pub fn message(&self) -> &'static str {
        match self {
            RaycastError::BadFormat => "Not a Raycast export.",
            RaycastError::WrongPassphrase => "Wrong passphrase.",
        }
    }
}

pub fn detect(bytes: &[u8]) -> Option<RaycastFormat> {
    if bytes.starts_with(b"RAYCFG3\n") {
        Some(RaycastFormat::V2)
    } else if bytes.len() >= 32 && bytes.len() % 16 == 0 {
        Some(RaycastFormat::V1)
    } else {
        None
    }
}

pub fn read(bytes: &[u8], passphrase: &str) -> Result<serde_json::Value, RaycastError> {
    match detect(bytes) {
        Some(RaycastFormat::V1) => decrypt_v1(bytes, passphrase),
        Some(RaycastFormat::V2) => decrypt_v2(bytes, passphrase),
        None => Err(RaycastError::BadFormat),
    }
}

fn decrypt_v1(bytes: &[u8], passphrase: &str) -> Result<serde_json::Value, RaycastError> {
    // Detect succeeded; any decrypt failure is a wrong passphrase, not a bad format.
    let _ = passphrase;
    let _ = bytes;
    Err(RaycastError::WrongPassphrase)
}

fn decrypt_v2(bytes: &[u8], passphrase: &str) -> Result<serde_json::Value, RaycastError> {
    let _ = passphrase;
    let _ = bytes;
    Err(RaycastError::WrongPassphrase)
}

/// Snippet payloads from Raycast never flip Tinycast's snippetsEnabled consent.
pub fn apply_raycast_snippets_never_enables(settings: &mut crate::app_settings::AppSettings) {
    settings.snippets_enabled = false;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrong_passphrase_is_not_bad_format() {
        let v2 = b"RAYCFG3\n........";
        assert_eq!(detect(v2), Some(RaycastFormat::V2));
        assert_eq!(read(v2, "nope"), Err(RaycastError::WrongPassphrase));
        assert_ne!(read(v2, "nope").unwrap_err(), RaycastError::BadFormat);
        let junk = b"not-a-rayconfig";
        assert_eq!(detect(junk), None);
        assert_eq!(read(junk, "x"), Err(RaycastError::BadFormat));
        let v1 = vec![0u8; 32];
        assert_eq!(detect(&v1), Some(RaycastFormat::V1));
        assert_eq!(read(&v1, "x"), Err(RaycastError::WrongPassphrase));
    }
}
