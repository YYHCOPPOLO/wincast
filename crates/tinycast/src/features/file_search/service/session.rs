use std::sync::Mutex;
use std::time::{Duration, Instant};

use tinycast_pure::file_search::{FileSearchHit, FileSearchPolicy};
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_MULTITHREADED};
use windows::Win32::UI::WindowsAndMessaging::PostMessageW;

use super::search;
use crate::platform::messages::WM_FILE_SEARCH;

pub const DEBOUNCE_MS: u64 = 120;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum State {
    Idle,
    Searching,
    Ready,
    Failed,
}

#[derive(Clone)]
struct Pending {
    query: String,
    revision: u64,
    earliest: Instant,
    policy: FileSearchPolicy,
}

struct WorkerBox {
    pending: Option<Pending>,
    running: bool,
}

static WORKER: Mutex<WorkerBox> = Mutex::new(WorkerBox {
    pending: None,
    running: false,
});

static DONE: Mutex<Option<(u64, String, Result<Vec<FileSearchHit>, ()>)>> = Mutex::new(None);

pub struct FileSearchSession {
    results: Vec<FileSearchHit>,
    state: State,
    query: String,
    revision: u64,
    policy: FileSearchPolicy,
    host: HWND,
}

impl FileSearchSession {
    pub const DEBOUNCE_MS: u64 = DEBOUNCE_MS;

    pub fn new(home: String, scopes: &[String], ignore: &[String], host: HWND) -> Self {
        Self {
            results: Vec::new(),
            state: State::Idle,
            query: String::new(),
            revision: 0,
            policy: FileSearchPolicy::new(scopes, ignore, &home),
            host,
        }
    }

    pub fn from_settings(scopes: &[String], ignore: &[String], host: HWND) -> Self {
        let home = std::env::var("USERPROFILE").unwrap_or_default();
        Self::new(home, scopes, ignore, host)
    }

    pub fn set_host(&mut self, host: HWND) {
        self.host = host;
    }

    pub fn state(&self) -> State {
        self.state
    }

    pub fn results(&self) -> &[FileSearchHit] {
        &self.results
    }

    pub fn apply(&mut self, scopes: &[String], ignore: &[String]) {
        let home = self.policy.home.clone();
        let policy = FileSearchPolicy::new(scopes, ignore, &home);
        if policy == self.policy {
            return;
        }
        self.policy = policy;
        self.cancel();
    }

    pub fn search(&mut self, raw: &str) {
        let query = raw.trim().to_string();
        if query.is_empty() {
            self.cancel();
            return;
        }
        if query == self.query && self.state != State::Failed {
            return;
        }
        self.revision = self.revision.wrapping_add(1);
        self.query = query.clone();
        self.state = State::Searching;
        let pending = Pending {
            query,
            revision: self.revision,
            earliest: Instant::now() + Duration::from_millis(DEBOUNCE_MS),
            policy: self.policy.clone(),
        };
        queue_work(pending, self.host);
    }

    pub fn cancel(&mut self) {
        self.revision = self.revision.wrapping_add(1);
        self.query.clear();
        self.results.clear();
        self.state = State::Idle;
        if let Ok(mut g) = WORKER.lock() {
            g.pending = None;
        }
    }

    pub fn take_done(&mut self) {
        let Some((revision, query, result)) = DONE.lock().ok().and_then(|mut g| g.take()) else {
            return;
        };
        self.accept(revision, query, result);
    }

    fn accept(&mut self, revision: u64, query: String, result: Result<Vec<FileSearchHit>, ()>) {
        if revision != self.revision || query != self.query {
            return;
        }
        match result {
            Ok(hits) => {
                self.results = hits;
                self.state = State::Ready;
            }
            Err(()) => {
                self.results.clear();
                self.state = State::Failed;
            }
        }
    }
}

fn queue_work(pending: Pending, host: HWND) {
    let mut start = false;
    if let Ok(mut g) = WORKER.lock() {
        g.pending = Some(pending);
        if !g.running {
            g.running = true;
            start = true;
        }
    }
    if start {
        let bits = host.0 as isize;
        let _ = std::thread::Builder::new()
            .name("tinycast-file-search".into())
            .spawn(move || {
                let host = HWND(bits as *mut core::ffi::c_void);
                run_worker(host);
            });
    }
}

fn run_worker(host: HWND) {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
    }
    loop {
        let request = {
            let Ok(mut g) = WORKER.lock() else {
                unsafe {
                    CoUninitialize();
                }
                return;
            };
            match g.pending.take() {
                Some(p) => p,
                None => {
                    g.running = false;
                    unsafe {
                        CoUninitialize();
                    }
                    return;
                }
            }
        };
        let wait = request.earliest.saturating_duration_since(Instant::now());
        if !wait.is_zero() {
            std::thread::sleep(wait);
        }
        {
            let Ok(g) = WORKER.lock() else {
                unsafe {
                    CoUninitialize();
                }
                return;
            };
            if g.pending
                .as_ref()
                .is_some_and(|p| p.revision != request.revision)
            {
                continue;
            }
        }
        let result = search::search(&request.query, &request.policy).map_err(|_| ());
        if let Ok(mut g) = DONE.lock() {
            *g = Some((request.revision, request.query, result));
        }
        unsafe {
            let _ = PostMessageW(host, WM_FILE_SEARCH, WPARAM(0), LPARAM(0));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::Win32::Foundation::HWND;

    #[test]
    fn debounce_is_120_ms() {
        assert_eq!(FileSearchSession::DEBOUNCE_MS, 120);
        assert_eq!(DEBOUNCE_MS, 120);
    }

    #[test]
    fn empty_query_does_no_work() {
        let mut s =
            FileSearchSession::new("C:/Users/test".into(), &["~".into()], &[], HWND::default());
        s.search("   ");
        assert_eq!(s.state(), State::Idle);
        assert!(s.results().is_empty());
        s.search("annual report");
        assert_eq!(s.state(), State::Searching);
        s.cancel();
        assert_eq!(s.state(), State::Idle);
    }

    #[test]
    fn superseded_result_is_dropped() {
        let mut s =
            FileSearchSession::new("C:/Users/test".into(), &["~".into()], &[], HWND::default());
        s.search("one");
        let old = s.revision;
        s.search("two");
        s.accept(
            old,
            "one".into(),
            Ok(vec![FileSearchHit::from_path(
                "C:/a.txt",
                false,
                "C:/Users/test",
            )]),
        );
        assert!(s.results().is_empty());
        assert_eq!(s.state(), State::Searching);
    }
}
