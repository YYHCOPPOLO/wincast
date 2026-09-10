//! `.rayconfig` v1/v2 decode. A wrong passphrase is never reported as bad format.

use std::io::{Read, Write};

use aes::Aes256;
use aes_gcm::aead::generic_array::typenum::U16;
use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{AesGcm, Nonce};

type Aes256Gcm16 = AesGcm<Aes256, U16>;
use cbc::cipher::{block_padding::Pkcs7, BlockDecryptMut, BlockEncryptMut, KeyIvInit};
use flate2::read::{GzDecoder, ZlibDecoder};
use flate2::write::GzEncoder;
use flate2::Compression;
use scrypt::{scrypt, Params};
use sha2::{Digest, Sha256};

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
    let json = match detect(bytes) {
        Some(RaycastFormat::V1) => decrypt_v1(bytes, passphrase)?,
        Some(RaycastFormat::V2) => decrypt_v2(bytes, passphrase)?,
        None => return Err(RaycastError::BadFormat),
    };
    Ok(tinycast_pure::settings_backup::filter_import(
        &map_to_tinycast(json),
    ))
}

fn decrypt_v1(bytes: &[u8], passphrase: &str) -> Result<serde_json::Value, RaycastError> {
    if bytes.len() < 32 || bytes.len() % 16 != 0 {
        return Err(RaycastError::WrongPassphrase);
    }
    let iv: [u8; 16] = bytes[..16]
        .try_into()
        .map_err(|_| RaycastError::WrongPassphrase)?;
    let key = Sha256::digest(passphrase.as_bytes());
    type Aes256CbcDec = cbc::Decryptor<Aes256>;
    let dec = Aes256CbcDec::new((&*key).into(), &iv.into());
    let plain = dec
        .decrypt_padded_vec_mut::<Pkcs7>(&bytes[16..])
        .map_err(|_| RaycastError::WrongPassphrase)?;
    if plain.len() < 3 || plain[0] != 0x1f || plain[1] != 0x8b || plain[2] != 0x08 {
        return Err(RaycastError::WrongPassphrase);
    }
    let json = gunzip(&plain).ok_or(RaycastError::WrongPassphrase)?;
    serde_json::from_slice(&json).map_err(|_| RaycastError::WrongPassphrase)
}

fn decrypt_v2(bytes: &[u8], passphrase: &str) -> Result<serde_json::Value, RaycastError> {
    const MAGIC: &[u8] = b"RAYCFG3\n";
    const FIXED: usize = 12;
    const TAG: usize = 16;
    if !bytes.starts_with(MAGIC) || bytes.len() < FIXED + TAG {
        return Err(RaycastError::WrongPassphrase);
    }
    let header_len = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
    if header_len == 0 || FIXED + header_len + TAG > bytes.len() {
        return Err(RaycastError::WrongPassphrase);
    }
    let header_bytes =
        gunzip(&bytes[FIXED..FIXED + header_len]).ok_or(RaycastError::WrongPassphrase)?;
    let header: serde_json::Value =
        serde_json::from_slice(&header_bytes).map_err(|_| RaycastError::WrongPassphrase)?;
    if header.get("schemaVersion").and_then(|v| v.as_i64()) != Some(3) {
        return Err(RaycastError::WrongPassphrase);
    }
    let (iv, salt) = v2_iv_salt(&header).ok_or(RaycastError::WrongPassphrase)?;
    if iv.len() != 16 || salt.len() != 16 {
        return Err(RaycastError::WrongPassphrase);
    }
    let payload_start = FIXED + header_len;
    let payload_end = bytes.len() - TAG;
    if payload_end <= payload_start {
        return Err(RaycastError::WrongPassphrase);
    }
    let mut key = [0u8; 32];
    let params = Params::new(14, 8, 1, 32).map_err(|_| RaycastError::WrongPassphrase)?;
    scrypt(passphrase.as_bytes(), &salt, &params, &mut key)
        .map_err(|_| RaycastError::WrongPassphrase)?;
    let cipher = Aes256Gcm16::new_from_slice(&key).map_err(|_| RaycastError::WrongPassphrase)?;
    let nonce = Nonce::<U16>::from_slice(&iv);
    let mut boxed = bytes[payload_start..payload_end].to_vec();
    boxed.extend_from_slice(&bytes[payload_end..]);
    let gz = cipher
        .decrypt(nonce, boxed.as_ref())
        .map_err(|_| RaycastError::WrongPassphrase)?;
    let json = gunzip(&gz).ok_or(RaycastError::WrongPassphrase)?;
    serde_json::from_slice(&json).map_err(|_| RaycastError::WrongPassphrase)
}

fn v2_iv_salt(header: &serde_json::Value) -> Option<(Vec<u8>, Vec<u8>)> {
    let enc = header.get("encryption").unwrap_or(header);
    let iv = hex_decode(enc.get("iv")?.as_str()?)?;
    let salt = hex_decode(enc.get("salt")?.as_str()?)?;
    Some((iv, salt))
}

fn gunzip(data: &[u8]) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    if GzDecoder::new(data).read_to_end(&mut out).is_ok() && !out.is_empty() {
        return Some(out);
    }
    out.clear();
    if ZlibDecoder::new(data).read_to_end(&mut out).is_ok() && !out.is_empty() {
        return Some(out);
    }
    None
}

fn hex_decode(s: &str) -> Option<Vec<u8>> {
    if s.len() % 2 != 0 {
        return None;
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).ok())
        .collect()
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn gzip(data: &[u8]) -> Vec<u8> {
    let mut enc = GzEncoder::new(Vec::new(), Compression::default());
    let _ = enc.write_all(data);
    enc.finish().unwrap_or_default()
}

fn map_to_tinycast(value: serde_json::Value) -> serde_json::Value {
    let mut out = serde_json::Map::new();
    collect_keys(&value, &mut out);
    extract_raycast(&value, &mut out);
    serde_json::Value::Object(out)
}

fn collect_keys(value: &serde_json::Value, out: &mut serde_json::Map<String, serde_json::Value>) {
    let Some(obj) = value.as_object() else {
        return;
    };
    for (k, v) in obj {
        match k.as_str() {
            "compactMode"
            | "showFavoritesInCompactMode"
            | "showInMenuBar"
            | "emojiSkinTone"
            | "popToRootTimeout"
            | "clipboardDisabledApps"
            | "hyperKeyPhysicalKey"
            | "hyperKeyIncludesShift"
            | "fileSearchEnabled"
            | "fileSearchScopes"
            | "notesEnabled" => {
                out.insert(k.clone(), v.clone());
            }
            _ => {}
        }
    }
}

fn extract_raycast(
    value: &serde_json::Value,
    out: &mut serde_json::Map<String, serde_json::Value>,
) {
    match value {
        serde_json::Value::Object(map) => {
            if let Some(v) = map.get("raycastPreferredWindowMode") {
                out.insert(
                    "compactMode".into(),
                    serde_json::json!(v.as_str() == Some("compact")),
                );
            }
            if let Some(v) = map.get("showFavoritesInCompactMode") {
                out.insert("showFavoritesInCompactMode".into(), v.clone());
            }
            if let Some(v) = map.get("statusBarIsVisible") {
                out.insert("showInMenuBar".into(), v.clone());
            }
            if let Some(v) = map.get("emojiSkinTone") {
                out.insert("emojiSkinTone".into(), v.clone());
            }
            if let Some(v) = map.get("popToRootTimeout") {
                out.insert("popToRootTimeout".into(), v.clone());
            }
            if let Some(v) = map.get("clipboardHistoryDisabledApplications") {
                out.insert("clipboardDisabledApps".into(), v.clone());
            }
            if let Some(settings) = map.get("settings") {
                extract_raycast(settings, out);
            }
            for v in map.values() {
                extract_raycast(v, out);
            }
        }
        serde_json::Value::Array(items) => {
            for v in items {
                extract_raycast(v, out);
            }
        }
        _ => {}
    }
}

/// Restore prior consent; an import must not enable (or disable) snippets listening.
pub fn apply_raycast_snippets_never_enables(
    settings: &mut crate::app_settings::AppSettings,
    was_enabled: bool,
) {
    settings.snippets_enabled = was_enabled;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_settings::AppSettings;

    fn encrypt_v1(json: &[u8], pass: &str) -> Vec<u8> {
        let gz = gzip(json);
        let key = Sha256::digest(pass.as_bytes());
        type Aes256CbcEnc = cbc::Encryptor<Aes256>;
        let iv = [9u8; 16];
        let enc = Aes256CbcEnc::new((&*key).into(), &iv.into());
        let ct = enc.encrypt_padded_vec_mut::<Pkcs7>(&gz);
        let mut out = iv.to_vec();
        out.extend(ct);
        out
    }

    fn encrypt_v2(json: &[u8], pass: &str) -> Vec<u8> {
        let salt = [3u8; 16];
        let iv = [4u8; 16];
        let mut key = [0u8; 32];
        let params = Params::new(14, 8, 1, 32).unwrap();
        scrypt(pass.as_bytes(), &salt, &params, &mut key).unwrap();
        let header = serde_json::json!({
            "schemaVersion": 3,
            "encryption": {
                "iv": hex_encode(&iv),
                "salt": hex_encode(&salt),
            }
        });
        let header_gz = gzip(&serde_json::to_vec(&header).unwrap());
        let payload_gz = gzip(json);
        let cipher = Aes256Gcm16::new_from_slice(&key).unwrap();
        let nonce = Nonce::<U16>::from_slice(&iv);
        let sealed = cipher.encrypt(nonce, payload_gz.as_ref()).unwrap();
        let mut out = b"RAYCFG3\n".to_vec();
        out.extend_from_slice(&(header_gz.len() as u32).to_le_bytes());
        out.extend(header_gz);
        out.extend(sealed);
        out
    }

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

    #[test]
    fn v1_round_trip_decrypts_settings() {
        let payload = serde_json::json!({
            "compactMode": true,
            "snippetsEnabled": true,
            "raycastPreferredWindowMode": "compact"
        });
        let bytes = encrypt_v1(&serde_json::to_vec(&payload).unwrap(), "secret");
        let json = read(&bytes, "secret").unwrap();
        assert_eq!(json["compactMode"], true);
        assert!(json.get("snippetsEnabled").is_none());
        assert_eq!(read(&bytes, "wrong"), Err(RaycastError::WrongPassphrase));
    }

    #[test]
    fn v2_round_trip_decrypts_settings() {
        let payload = serde_json::json!({
            "settings": { "showFavoritesInCompactMode": false },
            "snippetsEnabled": true
        });
        let bytes = encrypt_v2(&serde_json::to_vec(&payload).unwrap(), "secret");
        let json = read(&bytes, "secret").unwrap();
        assert_eq!(json["showFavoritesInCompactMode"], false);
        assert!(json.get("snippetsEnabled").is_none());
        assert_eq!(read(&bytes, "nope"), Err(RaycastError::WrongPassphrase));
    }

    #[test]
    fn raycast_leaves_existing_snippets_consent() {
        let mut s = AppSettings::default();
        s.snippets_enabled = true;
        apply_raycast_snippets_never_enables(&mut s, true);
        assert!(s.snippets_enabled);
        s.snippets_enabled = true;
        apply_raycast_snippets_never_enables(&mut s, false);
        assert!(!s.snippets_enabled);
    }
}
