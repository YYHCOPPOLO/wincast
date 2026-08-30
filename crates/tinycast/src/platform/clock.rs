//! Local civil time encoded as a naive unix timestamp for CalcEngine.

use windows::Win32::System::SystemInformation::GetLocalTime;

/// Local wall-clock fields interpreted as UTC, so datetime queries use civil time.
pub fn local_naive_unix() -> i64 {
    let st = unsafe { GetLocalTime() };
    civil_to_unix(
        st.wYear as i32,
        st.wMonth as u32,
        st.wDay as u32,
        st.wHour as u32,
        st.wMinute as u32,
        st.wSecond as u32,
    )
}

pub fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Howard Hinnant's days_from_civil, treating the fields as UTC.
fn civil_to_unix(year: i32, month: u32, day: u32, hour: u32, minute: u32, second: u32) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = (y - era * 400) as u32;
    let mp = if month > 2 { month - 3 } else { month + 9 };
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = i64::from(era) * 146097 + i64::from(doe) - 719468;
    days * 86_400 + i64::from(hour) * 3600 + i64::from(minute) * 60 + i64::from(second)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn naive_unix_hour_matches_local_systemtime() {
        let st = unsafe { GetLocalTime() };
        let naive = local_naive_unix();
        let hour = (naive.rem_euclid(86_400) / 3600) as u16;
        assert_eq!(hour, st.wHour);
    }

    #[test]
    fn civil_to_unix_known_utc_midnight() {
        // 2026-07-24 00:18:00 UTC used by calc datetime tests.
        assert_eq!(civil_to_unix(2026, 7, 24, 0, 18, 0), 1_784_852_280);
    }
}
