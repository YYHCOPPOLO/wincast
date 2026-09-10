use super::link::MeetingLink;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MeetingEvent {
    pub id: String,
    pub title: String,
    pub start: i64,
    pub end: i64,
    pub is_all_day: bool,
    pub is_declined: bool,
    pub calendar_id: String,
    pub calendar_name: String,
    pub calendar_item_id: String,
    pub link: Option<MeetingLink>,
}

impl MeetingEvent {
    pub fn is_in_progress(&self, now: i64) -> bool {
        self.start <= now && now < self.end
    }
}

/// Which meeting is worth showing. Every clock read is an injected `now` (unix seconds).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UpcomingWindow {
    pub lead_minutes: i64,
}

impl UpcomingWindow {
    fn lead_secs(self) -> i64 {
        self.lead_minutes.saturating_mul(60)
    }

    pub fn agenda(events: &[MeetingEvent], now: i64) -> Vec<MeetingEvent> {
        let mut out: Vec<MeetingEvent> = events
            .iter()
            .filter(|e| !e.is_all_day && !e.is_declined && e.end > now)
            .cloned()
            .collect();
        out.sort_by_key(|e| e.start);
        out
    }

    /// `[start - lead, min(start + lead, end))`. A short meeting never outlives its end.
    pub fn carded(&self, events: &[MeetingEvent], now: i64) -> Option<MeetingEvent> {
        let lead = self.lead_secs();
        Self::agenda(events, now).into_iter().find(|e| {
            e.link.is_some()
                && now >= e.start - lead
                && now < e.start.saturating_add(lead).min(e.end)
        })
    }

    pub fn joinable(&self, events: &[MeetingEvent], now: i64) -> Option<MeetingEvent> {
        if let Some(carded) = self.carded(events, now) {
            return Some(carded);
        }
        let linked: Vec<MeetingEvent> = Self::agenda(events, now)
            .into_iter()
            .filter(|e| e.link.is_some())
            .collect();
        linked
            .iter()
            .find(|e| e.is_in_progress(now))
            .cloned()
            .or_else(|| linked.into_iter().find(|e| e.start > now))
    }

    pub fn countdown(start: i64, now: i64) -> String {
        let delta = start - now;
        if delta > 0 {
            let mins = ((delta as f64) / 60.0).ceil() as i64;
            format!("in {mins} min")
        } else {
            let elapsed = ((-delta) as f64 / 60.0).floor() as i64;
            if elapsed == 0 {
                "now".into()
            } else {
                format!("{elapsed} min ago")
            }
        }
    }
}

pub struct AutoJoinPolicy {
    pub armed_at: i64,
}

impl AutoJoinPolicy {
    pub fn meeting(
        &self,
        events: &[MeetingEvent],
        now: i64,
        window: UpcomingWindow,
        joined: &std::collections::HashSet<String>,
    ) -> Option<MeetingEvent> {
        let carded = window.carded(events, now)?;
        if carded.start >= self.armed_at && now >= carded.start && !joined.contains(&carded.id) {
            Some(carded)
        } else {
            None
        }
    }
}

pub struct MenuBarSummary {
    pub lead_minutes: i64,
    pub hide_after_minutes: Option<i64>,
    pub linked_only: bool,
}

impl MenuBarSummary {
    pub const TITLE_CAP: usize = 24;

    pub fn event(&self, events: &[MeetingEvent], now: i64) -> Option<MeetingEvent> {
        let lead = self.lead_minutes.saturating_mul(60);
        UpcomingWindow::agenda(events, now).into_iter().find(|e| {
            if self.linked_only && e.link.is_none() {
                return false;
            }
            now >= e.start - lead && now < self.hides_at(e)
        })
    }

    pub fn title(title: &str) -> String {
        if title.chars().count() <= Self::TITLE_CAP {
            return title.to_string();
        }
        let mut s: String = title.chars().take(Self::TITLE_CAP - 1).collect();
        s = s.trim_end().to_string();
        s.push('…');
        s
    }

    fn hides_at(&self, event: &MeetingEvent) -> i64 {
        match self.hide_after_minutes {
            None => event.start,
            Some(mins) => event
                .start
                .saturating_add(mins.saturating_mul(60))
                .min(event.end),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::meeting::link::{MeetingLink, Provider};

    fn sample(end: i64) -> MeetingEvent {
        MeetingEvent {
            id: "m1".into(),
            title: "Standup".into(),
            start: 1_000,
            end,
            is_all_day: false,
            is_declined: false,
            calendar_id: String::new(),
            calendar_name: String::new(),
            calendar_item_id: String::new(),
            link: Some(MeetingLink {
                provider: Provider::Generic,
                url: "https://example.com/meet".into(),
            }),
        }
    }

    #[test]
    fn card_window_does_not_outlive_short_meeting() {
        // lead 5min, meeting 2min long → window ends at end
        let event = sample(1_120);
        let window = UpcomingWindow { lead_minutes: 5 };
        assert!(window.carded(&[event.clone()], 1_090).is_some());
        assert!(window.carded(&[event.clone()], 1_120).is_none());
        assert!(window.carded(&[event], 1_200).is_none());
    }

    #[test]
    fn auto_join_once_per_id() {
        let event = sample(2_000);
        let window = UpcomingWindow { lead_minutes: 5 };
        let policy = AutoJoinPolicy { armed_at: 900 };
        let mut joined = std::collections::HashSet::new();
        assert!(policy
            .meeting(&[event.clone()], 1_000, window, &joined)
            .is_some());
        joined.insert(event.id.clone());
        assert!(policy.meeting(&[event], 1_000, window, &joined).is_none());
    }
}
