//! Optional ChatGPT subscription via a PATH `codex` binary. Never bundled.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::Child;

pub const INSTALL_DOCS: &str = "https://developers.openai.com/codex/cli";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CodexPhase {
    Idle,
    Connected,
    Unavailable,
    Failed,
}

pub struct ChatGptManager {
    child: Option<Child>,
    phase: CodexPhase,
    home: PathBuf,
}

impl ChatGptManager {
    pub fn new() -> Self {
        Self {
            child: None,
            phase: if which_codex().is_some() {
                CodexPhase::Idle
            } else {
                CodexPhase::Unavailable
            },
            home: crate::platform::paths::roaming_dir()
                .join("ChatGPTSubscription")
                .join("CodexHome"),
        }
    }

    pub fn phase(&self) -> CodexPhase {
        self.phase
    }

    pub fn home(&self) -> &Path {
        &self.home
    }

    pub fn refresh(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        self.phase = if which_codex().is_some() {
            CodexPhase::Idle
        } else {
            CodexPhase::Unavailable
        };
    }

    /// JSON-RPC turns are not implemented; never report Connected or spawn a disconnected server.
    pub fn connect(&mut self) -> Result<(), String> {
        self.stop();
        if which_codex().is_none() {
            self.phase = CodexPhase::Unavailable;
            return Err("Codex CLI is not on PATH.".into());
        }
        self.phase = CodexPhase::Idle;
        Err("ChatGPT chat is not available yet. Choose an API connection.".into())
    }

    pub fn stop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        self.refresh();
    }
}

impl Drop for ChatGptManager {
    fn drop(&mut self) {
        self.stop();
    }
}

pub fn which_codex() -> Option<PathBuf> {
    which_codex_in(std::env::var_os("PATH")?.as_os_str())
}

pub fn which_codex_in(path: &OsStr) -> Option<PathBuf> {
    for dir in std::env::split_paths(path) {
        for name in ["codex.exe", "codex"] {
            let candidate = dir.join(name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

fn codex_args() -> &'static [&'static str] {
    &[
        "-c",
        "cli_auth_credentials_store=\"file\"",
        "-c",
        "check_for_update_on_startup=false",
        "-c",
        "features.apps=false",
        "-c",
        "features.plugins=false",
        "-c",
        "features.shell_tool=false",
        "-c",
        "features.browser_use=false",
        "-c",
        "features.computer_use=false",
        "app-server",
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_path_is_unavailable() {
        assert!(which_codex_in(OsStr::new("")).is_none());
    }

    #[test]
    fn finds_codex_on_injected_path() {
        let dir = std::env::temp_dir().join(format!(
            "tinycast-codex-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let exe = dir.join("codex.exe");
        std::fs::write(&exe, b"fake").unwrap();
        assert_eq!(which_codex_in(dir.as_os_str()), Some(exe));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn install_docs_are_the_openai_cli_page() {
        assert_eq!(INSTALL_DOCS, "https://developers.openai.com/codex/cli");
        assert!(codex_args().contains(&"app-server"));
        assert!(codex_args().iter().any(|a| a.contains("shell_tool=false")));
    }

    #[test]
    fn connect_does_not_mark_connected_without_a_stream() {
        let mut mgr = ChatGptManager {
            child: None,
            phase: CodexPhase::Idle,
            home: std::env::temp_dir().join("tinycast-codex-home-test"),
        };
        let err = mgr.connect().unwrap_err();
        assert!(!err.to_lowercase().contains("sk-"));
        assert_ne!(mgr.phase(), CodexPhase::Connected);
        assert!(mgr.child.is_none());
    }
}
