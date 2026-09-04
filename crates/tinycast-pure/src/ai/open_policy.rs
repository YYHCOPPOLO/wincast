//! Whether summoning AI Chat resumes the last transcript.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OpenPolicy {
    Recent { after_minutes: u32 },
    New,
}

pub fn should_resume(policy: OpenPolicy, last_activity: i64, now: i64) -> bool {
    match policy {
        OpenPolicy::New => false,
        OpenPolicy::Recent { after_minutes } => {
            if last_activity <= 0 || now < last_activity {
                return false;
            }
            if after_minutes == u32::MAX {
                return true;
            }
            now - last_activity < (after_minutes as i64) * 60
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_never_resumes() {
        assert!(!should_resume(OpenPolicy::New, 1, 2));
    }

    #[test]
    fn recent_resumes_inside_idle_window() {
        assert!(should_resume(
            OpenPolicy::Recent { after_minutes: 10 },
            1_000,
            1_000 + 9 * 60
        ));
        assert!(!should_resume(
            OpenPolicy::Recent { after_minutes: 10 },
            1_000,
            1_000 + 10 * 60
        ));
        assert!(!should_resume(
            OpenPolicy::Recent { after_minutes: 10 },
            0,
            1_000
        ));
        assert!(should_resume(
            OpenPolicy::Recent {
                after_minutes: u32::MAX
            },
            1,
            1_000_000
        ));
    }
}
