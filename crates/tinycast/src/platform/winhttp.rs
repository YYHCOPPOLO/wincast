//! Cacheless WinHTTP. GET for rates; POST + chunked/SSE for AI. No extra HTTP crate.

use windows::core::{w, PCWSTR};
use windows::Win32::Networking::WinHttp::{
    WinHttpAddRequestHeaders, WinHttpCloseHandle, WinHttpConnect, WinHttpOpen, WinHttpOpenRequest,
    WinHttpQueryHeaders, WinHttpReadData, WinHttpReceiveResponse, WinHttpSendRequest,
    WinHttpSetTimeouts, WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY, WINHTTP_ADDREQ_FLAG_ADD,
    WINHTTP_FLAG_REFRESH, WINHTTP_FLAG_SECURE, WINHTTP_OPEN_REQUEST_FLAGS,
    WINHTTP_QUERY_FLAG_NUMBER, WINHTTP_QUERY_STATUS_CODE,
};

const GET_TIMEOUT_MS: i32 = 20_000;
const POST_TIMEOUT_MS: i32 = 90_000;

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
    unsafe { fetch_get(&parts) }
}

/// POST `body` and invoke `on_chunk` for each read. Return `Ok(false)` from the callback to abort.
pub fn post_stream(
    url: &str,
    headers: &[(&str, &str)],
    body: &[u8],
    mut on_chunk: impl FnMut(&[u8]) -> Result<bool, String>,
) -> Result<u32, String> {
    let parts = split_url(url).ok_or_else(|| format!("bad url: {url}"))?;
    unsafe { fetch_post(&parts, headers, body, &mut on_chunk) }
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
    let (host, port) = split_host_port(host_port, if https { 443 } else { 80 })?;
    if host.is_empty() {
        return None;
    }
    Some(UrlParts {
        host,
        path,
        port,
        https,
    })
}

fn split_host_port(authority: &str, default_port: u16) -> Option<(String, u16)> {
    if let Some(inner) = authority.strip_prefix('[') {
        let (host, rest) = inner.split_once(']')?;
        if host.is_empty() {
            return None;
        }
        let port = if rest.is_empty() {
            default_port
        } else {
            rest.strip_prefix(':')?.parse().ok()?
        };
        return Some((host.to_string(), port));
    }
    if let Some((host, port)) = authority.rsplit_once(':') {
        if host.contains(':') {
            return None;
        }
        return Some((host.to_string(), port.parse().ok()?));
    }
    Some((authority.to_string(), default_port))
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

unsafe fn open_request(
    parts: &UrlParts,
    method: PCWSTR,
    timeout_ms: i32,
) -> Result<(Handle, Handle, Handle), String> {
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
    let _ = WinHttpSetTimeouts(session.0, timeout_ms, timeout_ms, timeout_ms, timeout_ms);
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
        method,
        PCWSTR(path.as_ptr()),
        PCWSTR::null(),
        PCWSTR::null(),
        std::ptr::null(),
        WINHTTP_OPEN_REQUEST_FLAGS(flag_bits),
    ));
    if request.0.is_null() {
        return Err("WinHttpOpenRequest failed".into());
    }
    Ok((session, connect, request))
}

unsafe fn query_status(request: &Handle) -> u32 {
    let mut status: u32 = 0;
    let mut status_size = std::mem::size_of::<u32>() as u32;
    let mut index = 0u32;
    let _ = WinHttpQueryHeaders(
        request.0,
        WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
        PCWSTR::null(),
        Some((&mut status as *mut u32).cast()),
        &mut status_size,
        &mut index,
    );
    status
}

unsafe fn fetch_get(parts: &UrlParts) -> Result<String, String> {
    let (_session, _connect, request) = open_request(parts, w!("GET"), GET_TIMEOUT_MS)?;
    WinHttpSendRequest(request.0, None, None, 0, 0, 0).map_err(|e| e.to_string())?;
    WinHttpReceiveResponse(request.0, std::ptr::null_mut()).map_err(|e| e.to_string())?;
    let status = query_status(&request);
    if status != 0 && status != 200 {
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

unsafe fn fetch_post(
    parts: &UrlParts,
    headers: &[(&str, &str)],
    body: &[u8],
    on_chunk: &mut dyn FnMut(&[u8]) -> Result<bool, String>,
) -> Result<u32, String> {
    let (_session, _connect, request) = open_request(parts, w!("POST"), POST_TIMEOUT_MS)?;
    if !headers.is_empty() {
        let joined = headers
            .iter()
            .map(|(k, v)| format!("{k}: {v}"))
            .collect::<Vec<_>>()
            .join("\r\n");
        let header_wide = wide(&joined);
        WinHttpAddRequestHeaders(
            request.0,
            &header_wide[..header_wide.len().saturating_sub(1)],
            WINHTTP_ADDREQ_FLAG_ADD,
        )
        .map_err(|_| "WinHttpAddRequestHeaders failed".to_string())?;
    }
    let optional = if body.is_empty() {
        None
    } else {
        Some(body.as_ptr().cast())
    };
    WinHttpSendRequest(
        request.0,
        None,
        optional,
        body.len() as u32,
        body.len() as u32,
        0,
    )
    .map_err(|e| e.to_string())?;
    WinHttpReceiveResponse(request.0, std::ptr::null_mut()).map_err(|e| e.to_string())?;
    let status = query_status(&request);
    if status != 0 && status != 200 {
        return Err(status_message(status));
    }
    loop {
        let mut chunk = vec![0u8; 2 * 1024];
        let mut read = 0u32;
        WinHttpReadData(
            request.0,
            chunk.as_mut_ptr().cast(),
            chunk.len() as u32,
            &mut read,
        )
        .map_err(|_| "The network request failed.".to_string())?;
        if read == 0 {
            break;
        }
        if !on_chunk(&chunk[..read as usize])? {
            break;
        }
    }
    Ok(status)
}

pub fn status_message(status: u32) -> String {
    match status {
        401 | 403 => "API key rejected — check it in Settings.".into(),
        429 => "Rate limit reached — try again later.".into(),
        500..=599 => format!("The provider is temporarily unavailable (HTTP {status})."),
        _ => format!("The provider rejected the model or request (HTTP {status})."),
    }
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

    #[test]
    fn splits_loopback_http() {
        let parts = split_url("http://127.0.0.1:11434/v1/chat/completions").unwrap();
        assert_eq!(parts.host, "127.0.0.1");
        assert_eq!(parts.port, 11434);
        assert!(!parts.https);
        let v6 = split_url("http://[::1]/v1").unwrap();
        assert_eq!(v6.host, "::1");
        assert!(!v6.https);
    }
}
