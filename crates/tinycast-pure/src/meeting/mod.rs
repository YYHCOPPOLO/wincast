pub mod link;
pub mod window;

pub use link::{detect_link, MeetingLink, Provider};
pub use window::{AutoJoinPolicy, MeetingEvent, MenuBarSummary, UpcomingWindow};
