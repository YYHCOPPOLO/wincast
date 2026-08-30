//! Cacheless HTTPS GET via WinHTTP. No extra HTTP crate.

use windows::core::{w, PCWSTR};
use windows::Win32::Networking::WinHttp::{
    WinHttpCloseHandle, WinHttpConnect, WinHttpOpen, WinHttpOpenRequest, WinHttpQueryHeaders,
    WinHttpReadData, WinHttpReceiveResponse, WinHttpSendRequest, WinHttpSetTimeouts,
    WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY, WINHTTP_FLAG_REFRESH, WINHTTP_FLAG_SECURE,
    WINHTTP_OPEN_REQUEST_FLAGS, WINHTTP_QUERY_FLAG_NUMBER, WINHTTP_QUERY_STATUS_CODE,
};

const TIMEOUT_MS: i32 = 20_000;

struct Handle(*mut core::ffi::c_void);

impl Drop for Handle {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe {
                let _ = WinHttpCloseHandle(self.0);
            }
        }
    }
}

pub fn get_text(url: &str) -> Result<String, String> {
    let parts = split_url(url).ok_or_else(|| format!("bad url: {url}"))?;
    unsafe { fetch(&parts) }
}

struct UrlParts {
    host: String,
    path: String,
    port: u16,
    https: bool,
}

fn split_url(url: &str) -> Option<UrlParts> {
    let (https, rest) = if let Some(rest) = url.strip_prefix("https://") {
        (true, rest)
    } else if let Some(rest) = url.strip_prefix("http://") {
        (false, rest)
    } else {
        return None;
    };
    let (host_port, path) = match rest.split_once('/') {
        Some((host, path)) => (host, format!("/{path}")),
        None => (rest, "/".into()),
    };
    if host_port.is_empty() {
        return None;
    }
    let (host, port) = if let Some((host, port)) = host_port.split_once(':') {
        (host, port.parse().ok()?)
    } else {
        (host_port, if https { 443 } else { 80 })
    };
    if host.is_empty() {
        return None;
    }
    Some(UrlParts {
        host: host.to_string(),
        path,
        port,
        https,
    })
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

unsafe fn fetch(parts: &UrlParts) -> Result<String, String> {
    let session = Handle(WinHttpOpen(
        w!("Tinycast/0.10.2"),
        WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY,
        PCWSTR::null(),
        PCWSTR::null(),
        0,
    ));
    if session.0.is_null() {
        return Err("WinHttpOpen failed".into());
    }
    let _ = WinHttpSetTimeouts(session.0, TIMEOUT_MS, TIMEOUT_MS, TIMEOUT_MS, TIMEOUT_MS);
    let host = wide(&parts.host);
    let connect = Handle(WinHttpConnect(
        session.0,
        PCWSTR(host.as_ptr()),
        parts.port,
        0,
    ));
    if connect.0.is_null() {
        return Err(format!("WinHttpConnect {}", parts.host));
    }
    let path = wide(&parts.path);
    let mut flag_bits = WINHTTP_FLAG_REFRESH.0;
    if parts.https {
        flag_bits |= WINHTTP_FLAG_SECURE.0;
    }
    let request = Handle(WinHttpOpenRequest(
        connect.0,
        w!("GET"),
        PCWSTR(path.as_ptr()),
        PCWSTR::null(),
        PCWSTR::null(),
        std::ptr::null(),
        WINHTTP_OPEN_REQUEST_FLAGS(flag_bits),
    ));
    if request.0.is_null() {
        return Err("WinHttpOpenRequest failed".into());
    }
    WinHttpSendRequest(request.0, None, None, 0, 0, 0).map_err(|e| e.to_string())?;
    WinHttpReceiveResponse(request.0, std::ptr::null_mut()).map_err(|e| e.to_string())?;
    let mut status: u32 = 0;
    let mut status_size = std::mem::size_of::<u32>() as u32;
    let mut index = 0u32;
    if WinHttpQueryHeaders(
        request.0,
        WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
        PCWSTR::null(),
        Some((&mut status as *mut u32).cast()),
        &mut status_size,
        &mut index,
    )
    .is_ok()
        && status != 200
    {
        return Err(format!("HTTP {status}"));
    }
    let mut body = Vec::new();
    loop {
        let mut chunk = vec![0u8; 16 * 1024];
        let mut read = 0u32;
        WinHttpReadData(
            request.0,
            chunk.as_mut_ptr().cast(),
            chunk.len() as u32,
            &mut read,
        )
        .map_err(|e| e.to_string())?;
        if read == 0 {
            break;
        }
        body.extend_from_slice(&chunk[..read as usize]);
        if body.len() > 2 * 1024 * 1024 {
            return Err("response too large".into());
        }
    }
    String::from_utf8(body).map_err(|err| err.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_https_path_and_query() {
        let parts = split_url(
            "https://api.coingecko.com/api/v3/simple/price?ids=bitcoin&vs_currencies=usd",
        )
        .unwrap();
        assert_eq!(parts.host, "api.coingecko.com");
        assert_eq!(
            parts.path,
            "/api/v3/simple/price?ids=bitcoin&vs_currencies=usd"
        );
        assert_eq!(parts.port, 443);
        assert!(parts.https);
    }
}
