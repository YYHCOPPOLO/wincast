//! DPAPI wrap for API keys. Entropy is the connection UUID so a key cannot follow a retarget.

use windows::core::PCWSTR;
use windows::Win32::Foundation::{LocalFree, HLOCAL};
use windows::Win32::Security::Cryptography::{
    CryptProtectData, CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
};

pub fn protect(plain: &[u8], entropy: &[u8]) -> Result<Vec<u8>, String> {
    crypt(plain, entropy, true)
}

pub fn unprotect(blob: &[u8], entropy: &[u8]) -> Result<Vec<u8>, String> {
    crypt(blob, entropy, false)
}

fn crypt(input: &[u8], entropy: &[u8], seal: bool) -> Result<Vec<u8>, String> {
    let data_in = CRYPT_INTEGER_BLOB {
        cbData: input.len() as u32,
        pbData: input.as_ptr() as *mut u8,
    };
    let entropy_blob = CRYPT_INTEGER_BLOB {
        cbData: entropy.len() as u32,
        pbData: entropy.as_ptr() as *mut u8,
    };
    let mut data_out = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: std::ptr::null_mut(),
    };
    let entropy_ptr = if entropy.is_empty() {
        None
    } else {
        Some(&entropy_blob as *const CRYPT_INTEGER_BLOB)
    };
    unsafe {
        let result = if seal {
            CryptProtectData(
                &data_in,
                PCWSTR::null(),
                entropy_ptr,
                None,
                None,
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut data_out,
            )
        } else {
            CryptUnprotectData(
                &data_in,
                None,
                entropy_ptr,
                None,
                None,
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut data_out,
            )
        };
        if result.is_err() {
            return Err("DPAPI failed".into());
        }
        if data_out.pbData.is_null() {
            return Err("DPAPI empty".into());
        }
        let bytes = std::slice::from_raw_parts(data_out.pbData, data_out.cbData as usize).to_vec();
        let _ = LocalFree(HLOCAL(data_out.pbData.cast()));
        Ok(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_is_keyed_by_entropy() {
        let secret = b"sk-test-never-log";
        let id = b"connection-uuid";
        let sealed = protect(secret, id).expect("protect");
        assert_ne!(sealed, secret);
        assert_eq!(unprotect(&sealed, id).unwrap(), secret);
        assert!(unprotect(&sealed, b"other-uuid").is_err());
    }
}
