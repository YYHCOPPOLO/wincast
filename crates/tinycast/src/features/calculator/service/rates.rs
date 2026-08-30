//! Fiat + crypto snapshot store. Fetch is Effect; merge is pure.

use std::sync::{Arc, Mutex};

use tinycast_pure::calc::{lookup, merge_feeds, prices_coins, CurrencyRates};
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::Globalization::GetUserDefaultLocaleName;
use windows::Win32::UI::WindowsAndMessaging::PostMessageW;

use crate::platform::messages::WM_RATES;
use crate::platform::paths;
use crate::platform::winhttp;

pub const FIAT_URL: &str = "https://api.frankfurter.app/latest";
pub const COIN_URL: &str =
    "https://api.coingecko.com/api/v3/simple/price?ids=bitcoin,ethereum,solana&vs_currencies=usd";

const REFRESH_SECS: i64 = 24 * 3600;
const RETRY_SECS: i64 = 30 * 60;

pub struct CurrencyRateStore {
    rates: Option<CurrencyRates>,
    completed_at: Option<i64>,
    pending: Arc<Mutex<Option<FetchOutcome>>>,
    running: bool,
}

struct FetchOutcome {
    rates: CurrencyRates,
    complete: bool,
}

impl CurrencyRateStore {
    pub fn new() -> Self {
        let mut store = Self {
            rates: None,
            completed_at: None,
            pending: Arc::new(Mutex::new(None)),
            running: false,
        };
        if let Some(cached) = load_cache() {
            if prices_coins(&cached) {
                store.completed_at = Some(cached.fetched_at);
            }
            store.rates = Some(cached);
        }
        store
    }

    pub fn rates(&self) -> Option<&CurrencyRates> {
        self.rates.as_ref()
    }

    pub fn region(&self) -> Option<String> {
        region_currency()
    }

    pub fn start(&mut self, host: HWND) {
        if self.running || host.is_invalid() {
            return;
        }
        self.running = true;
        let pending = Arc::clone(&self.pending);
        let completed_at = self.completed_at;
        let host_bits = host.0 as isize;
        let _ = std::thread::Builder::new()
            .name("tinycast-fx".into())
            .spawn(move || pump(host_bits, completed_at, pending));
    }

    pub fn install(&mut self) {
        let Some(outcome) = self.pending.lock().ok().and_then(|mut g| g.take()) else {
            return;
        };
        self.rates = Some(outcome.rates.clone());
        if outcome.complete {
            self.completed_at = Some(outcome.rates.fetched_at);
            save_cache(&outcome.rates);
        }
    }
}

fn pump(
    host_bits: isize,
    mut completed_at: Option<i64>,
    pending: Arc<Mutex<Option<FetchOutcome>>>,
) {
    loop {
        let now = unix_now();
        let age = completed_at
            .map(|stamp| now.saturating_sub(stamp).max(0))
            .unwrap_or(i64::MAX);
        if age < REFRESH_SECS {
            sleep_secs(REFRESH_SECS - age);
            continue;
        }
        match fetch_snapshot(now) {
            Some((rates, complete)) => {
                if let Ok(mut slot) = pending.lock() {
                    *slot = Some(FetchOutcome {
                        rates: rates.clone(),
                        complete,
                    });
                }
                post_ready(host_bits);
                if complete {
                    completed_at = Some(rates.fetched_at);
                    sleep_secs(REFRESH_SECS);
                } else {
                    sleep_secs(RETRY_SECS);
                }
            }
            None => sleep_secs(RETRY_SECS),
        }
    }
}

fn fetch_snapshot(now: i64) -> Option<(CurrencyRates, bool)> {
    let fiat_body = winhttp::get_text(FIAT_URL).ok()?;
    let crypto_body = winhttp::get_text(COIN_URL).ok();
    snapshot(&fiat_body, crypto_body.as_deref(), now)
}

pub(crate) fn snapshot(
    fiat_json: &str,
    crypto_json: Option<&str>,
    now: i64,
) -> Option<(CurrencyRates, bool)> {
    let fiat = parse_frankfurter(fiat_json)?;
    let crypto = crypto_json
        .map(parse_coingecko)
        .unwrap_or_default();
    let fiat_refs: Vec<(&str, f64)> = fiat.iter().map(|(c, r)| (c.as_str(), *r)).collect();
    let crypto_refs: Vec<(&str, f64)> = crypto.iter().map(|(c, r)| (c.as_str(), *r)).collect();
    let mut rates = merge_feeds(&fiat_refs, &crypto_refs);
    rates.fetched_at = now;
    let complete = prices_coins(&rates);
    Some((rates, complete))
}

fn parse_frankfurter(json: &str) -> Option<Vec<(String, f64)>> {
    let value: serde_json::Value = serde_json::from_str(json).ok()?;
    let base = value.get("base")?.as_str()?.to_ascii_uppercase();
    if base.len() != 3 {
        return None;
    }
    let object = value.get("rates")?.as_object()?;
    let mut pairs = Vec::new();
    for (code, rate) in object {
        let rate = rate.as_f64()?;
        if usable(rate) {
            pairs.push((code.to_ascii_uppercase(), rate));
        }
    }
    if pairs.is_empty() {
        return None;
    }
    pairs.push((base, 1.0));
    rebase_to_usd(pairs)
}

fn rebase_to_usd(pairs: Vec<(String, f64)>) -> Option<Vec<(String, f64)>> {
    let usd = pairs.iter().find(|(code, _)| code == "USD").map(|(_, r)| *r)?;
    if !usable(usd) {
        return None;
    }
    Some(
        pairs
            .into_iter()
            .map(|(code, rate)| {
                if code == "USD" {
                    ("USD".into(), 1.0)
                } else {
                    (code, rate / usd)
                }
            })
            .collect(),
    )
}

fn parse_coingecko(json: &str) -> Vec<(String, f64)> {
    const MAP: &[(&str, &str)] = &[
        ("bitcoin", "BTC"),
        ("ethereum", "ETH"),
        ("solana", "SOL"),
    ];
    let Ok(value) = serde_json::from_str::<serde_json::Value>(json) else {
        return Vec::new();
    };
    let Some(object) = value.as_object() else {
        return Vec::new();
    };
    let mut pairs = Vec::new();
    for (id, code) in MAP {
        let Some(price) = object
            .get(*id)
            .and_then(|v| v.get("usd"))
            .and_then(|v| v.as_f64())
        else {
            continue;
        };
        let inverted = 1.0 / price;
        if usable(price) && usable(inverted) {
            pairs.push(((*code).to_string(), inverted));
        }
    }
    pairs
}

fn usable(rate: f64) -> bool {
    rate > 0.0 && rate.is_finite()
}

fn cache_path() -> std::path::PathBuf {
    paths::local_dir().join("currency-rates.json")
}

fn load_cache() -> Option<CurrencyRates> {
    let bytes = std::fs::read(cache_path()).ok()?;
    serde_json::from_slice(&bytes).ok()
}

fn save_cache(rates: &CurrencyRates) {
    let dir = paths::local_dir();
    let _ = std::fs::create_dir_all(&dir);
    if let Ok(bytes) = serde_json::to_vec_pretty(rates) {
        let _ = std::fs::write(dir.join("currency-rates.json"), bytes);
    }
}

fn region_currency() -> Option<String> {
    let mut buf = [0u16; 85];
    let len = unsafe { GetUserDefaultLocaleName(&mut buf) };
    if len <= 1 {
        return None;
    }
    let locale = String::from_utf16_lossy(&buf[..len as usize - 1]);
    tinycast_pure::calc::currency_for_locale(&locale).map(str::to_string)
}

fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn sleep_secs(secs: i64) {
    let secs = secs.max(1) as u64;
    std::thread::sleep(std::time::Duration::from_secs(secs));
}

fn post_ready(host_bits: isize) {
    let host = HWND(host_bits as *mut core::ffi::c_void);
    if host.is_invalid() {
        return;
    }
    unsafe {
        let _ = PostMessageW(host, WM_RATES, WPARAM(0), LPARAM(0));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frankfurter_rebase_and_coin_invert() {
        let fiat = r#"{"amount":1.0,"base":"EUR","date":"2024-01-01","rates":{"USD":1.1,"GBP":0.88}}"#;
        let crypto = r#"{"bitcoin":{"usd":55000.0},"ethereum":{"usd":2000.0}}"#;
        let (rates, complete) = snapshot(fiat, Some(crypto), 42).unwrap();
        assert!(complete);
        assert_eq!(rates.fetched_at, 42);
        assert_eq!(lookup(&rates, "USD"), Some(1.0));
        let eur = lookup(&rates, "EUR").unwrap();
        assert!((eur - 1.0 / 1.1).abs() < 1e-12);
        let btc = lookup(&rates, "BTC").unwrap();
        assert!((btc - 1.0 / 55_000.0).abs() < 1e-12);
        assert_eq!(FIAT_URL, "https://api.frankfurter.app/latest");
        assert_eq!(
            COIN_URL,
            "https://api.coingecko.com/api/v3/simple/price?ids=bitcoin,ethereum,solana&vs_currencies=usd"
        );
    }

    #[test]
    fn missing_crypto_is_incomplete_and_not_required() {
        let fiat = r#"{"amount":1.0,"base":"EUR","date":"2024-01-01","rates":{"USD":1.0}}"#;
        let (rates, complete) = snapshot(fiat, None, 1).unwrap();
        assert!(!complete);
        assert_eq!(lookup(&rates, "USD"), Some(1.0));
        assert!(lookup(&rates, "BTC").is_none());
    }
}
