//! WinRT Appointments: `[startOfToday, endOfTomorrow+1day)`. Empty when unauthorized.

use tinycast_pure::meeting::{detect_link, MeetingEvent};
use windows::core::Interface;
use windows::Foundation::TimeSpan;
use windows::ApplicationModel::Appointments::{
    AppointmentManager, AppointmentStoreAccessType, FindAppointmentsOptions,
};

use crate::platform::clock::unix_now;

pub struct CalendarStore {
    events: Vec<MeetingEvent>,
    joined: std::collections::HashSet<String>,
    armed_at: i64,
}

impl CalendarStore {
    pub fn new() -> Self {
        Self {
            events: Vec::new(),
            joined: std::collections::HashSet::new(),
            armed_at: unix_now(),
        }
    }

    pub fn events(&self) -> &[MeetingEvent] {
        &self.events
    }

    pub fn joined(&self) -> &std::collections::HashSet<String> {
        &self.joined
    }

    pub fn mark_joined(&mut self, id: &str) {
        self.joined.insert(id.to_string());
    }

    pub fn armed_at(&self) -> i64 {
        self.armed_at
    }

    pub fn arm_now(&mut self) {
        self.armed_at = unix_now();
    }

    /// Consent already granted: prompt the OS store, then fetch. Deny is an error (empty list).
    pub fn request_and_refresh(&mut self) -> Result<(), String> {
        match fetch_appointments() {
            Ok(events) => {
                self.events = events;
                Ok(())
            }
            Err(err) => {
                self.events.clear();
                Err(err)
            }
        }
    }

    pub fn refresh(&mut self) {
        if let Ok(events) = fetch_appointments() {
            self.events = events;
        }
    }
}

fn fetch_appointments() -> Result<Vec<MeetingEvent>, String> {
    let store = AppointmentManager::RequestStoreAsync(AppointmentStoreAccessType::AllCalendarsReadOnly)
        .map_err(|e| e.to_string())?
        .get()
        .map_err(|e| e.to_string())?;
    let start = start_of_today();
    // [startOfToday, endOfTomorrow+1day) ≈ 3 days
    let duration = TimeSpan {
        Duration: 3 * 24 * 3600 * 10_000_000,
    };
    let opts = FindAppointmentsOptions::new().map_err(|e| e.to_string())?;
    let list = store
        .FindAppointmentsAsyncWithOptions(start, duration, &opts)
        .map_err(|e| e.to_string())?
        .get()
        .map_err(|e| e.to_string())?;
    let mut events = Vec::new();
    let n = list.Size().unwrap_or(0);
    for i in 0..n {
        let Ok(appt) = list.GetAt(i) else {
            continue;
        };
        let start_dt = appt.StartTime().ok();
        let duration = appt.Duration().ok();
        let start = start_dt
            .and_then(|d| d.UniversalTime.checked_div(10_000_000))
            .map(|t| t - 11_644_473_600)
            .unwrap_or(0);
        let dur_secs = duration
            .map(|d| d.Duration / 10_000_000)
            .unwrap_or(0);
        let subject = appt.Subject().map(|s| s.to_string()).unwrap_or_default();
        let location = appt.Location().map(|s| s.to_string()).unwrap_or_default();
        let details = appt.Details().map(|s| s.to_string()).unwrap_or_default();
        let uri = appt
            .Uri()
            .ok()
            .and_then(|u| u.AbsoluteUri().ok())
            .map(|s| s.to_string())
            .unwrap_or_default();
        let all_day = appt.AllDay().unwrap_or(false);
        let declined = appointment_is_declined(&appt);
        let fields = [uri.as_str(), location.as_str(), details.as_str()];
        let link = detect_link(&fields);
        let id = format!("{subject}:{start}");
        events.push(MeetingEvent {
            id: id.clone(),
            title: subject,
            start,
            end: start + dur_secs,
            is_all_day: all_day,
            is_declined: declined,
            calendar_id: String::new(),
            calendar_name: String::new(),
            calendar_item_id: id,
            link,
        });
    }
    let _ = Interface::vtable(&store);
    Ok(events)
}

fn appointment_is_declined(appt: &windows::ApplicationModel::Appointments::Appointment) -> bool {
    use windows::ApplicationModel::Appointments::AppointmentParticipantResponse;
    let Ok(invitees) = appt.Invitees() else {
        return false;
    };
    let Ok(iter) = invitees.First() else {
        return false;
    };
    while iter.HasCurrent().unwrap_or(false) {
        if let Ok(inv) = iter.Current() {
            if inv.Response().ok() == Some(AppointmentParticipantResponse::Declined) {
                return true;
            }
        }
        let _ = iter.MoveNext();
    }
    false
}

fn start_of_today() -> windows::Foundation::DateTime {
    let now = unix_now();
    // local midnight: use civil date from local_naive_unix if available
    let local = crate::platform::clock::local_naive_unix();
    let secs = now - (local % 86_400);
    windows::Foundation::DateTime {
        UniversalTime: (secs + 11_644_473_600) * 10_000_000,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn store_starts_empty() {
        let s = CalendarStore::new();
        assert!(s.events().is_empty());
        assert!(s.joined().is_empty());
    }
}
