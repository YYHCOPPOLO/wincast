/// When the support window may next ask. Pure: every date is handed in (unix seconds).
pub struct SupportReminderSchedule;

impl SupportReminderSchedule {
    pub const INTERVAL: i64 = 30 * 24 * 3600;

    /// Seconds until the next ask; `0` is due now. Anchor on the last ask, or first run if none.
    pub fn wait(since_anchor: i64, now: i64) -> i64 {
        let elapsed = (now - since_anchor).clamp(0, Self::INTERVAL);
        Self::INTERVAL - elapsed
    }
}

#[cfg(test)]
mod tests {
    use super::SupportReminderSchedule;

    #[test]
    fn first_ask_is_one_interval_after_first_seen() {
        assert_eq!(SupportReminderSchedule::wait(0, 0), SupportReminderSchedule::INTERVAL);
        assert_eq!(
            SupportReminderSchedule::wait(0, SupportReminderSchedule::INTERVAL),
            0
        );
        assert_eq!(
            SupportReminderSchedule::wait(100, 50),
            SupportReminderSchedule::INTERVAL
        );
    }
}
