//! Capture previous-app selection first, then hide the palette.

use std::sync::mpsc::{self, Receiver, Sender};

use tinycast_pure::ai::{AiEvent, AiMessage, AiRequest, ModelSelection, QuickAction, Role};
use windows::Win32::Foundation::HWND;

use crate::features::ai::service::factory::ProviderFactory;
use crate::features::ai::service::provider::{AiProvider, HttpAiProvider};
use crate::features::snippets::service::injector;
use crate::platform::messages::WM_QA;
use windows::Win32::UI::WindowsAndMessaging::PostMessageW;

pub struct QuickActionCoordinator {
    events_tx: Sender<AiEvent>,
    events_rx: Receiver<AiEvent>,
    provider: Option<HttpAiProvider>,
    running: bool,
    cancelled: bool,
    accepted: bool,
    generation: u64,
    action: Option<QuickAction>,
    target: HWND,
    result: String,
    preview: bool,
}

impl QuickActionCoordinator {
    pub fn new() -> Self {
        let (events_tx, events_rx) = mpsc::channel();
        Self {
            events_tx,
            events_rx,
            provider: None,
            running: false,
            cancelled: false,
            accepted: false,
            generation: 0,
            action: None,
            target: HWND::default(),
            result: String::new(),
            preview: false,
        }
    }

    pub fn is_running(&self) -> bool {
        self.running
    }

    pub fn result(&self) -> &str {
        &self.result
    }

    pub fn action(&self) -> Option<QuickAction> {
        self.action
    }

    pub fn target(&self) -> HWND {
        self.target
    }

    pub fn wants_preview(&self) -> bool {
        self.preview
    }

    pub fn can_apply(&self) -> bool {
        self.accepted && !self.cancelled && !self.result.is_empty() && !self.target.is_invalid()
    }

    /// Read selection from `target` before the caller hides the palette.
    pub fn capture(target: HWND) -> Result<String, String> {
        injector::capture_for_quick_action(target)
    }

    pub fn start(
        &mut self,
        action: QuickAction,
        selection: String,
        target: HWND,
        factory: &ProviderFactory,
        model: Option<&ModelSelection>,
        connections: &[tinycast_pure::ai::AiConnection],
        language: &str,
        preview: bool,
        host: HWND,
    ) -> Result<(), String> {
        if self.running {
            return Err("A Quick Action is already running.".into());
        }
        let Some(model) = model else {
            return Err("Choose a Quick Action model in Settings.".into());
        };
        let mut provider = factory.http_provider(model, connections)?;
        let request = AiRequest {
            instructions: Some(action.instructions(language)),
            messages: vec![AiMessage {
                role: Role::User,
                text: action.message(&selection),
            }],
            max_output_tokens: 2048,
            web_search: false,
        };
        self.generation = self.generation.wrapping_add(1);
        self.running = true;
        self.cancelled = false;
        self.accepted = false;
        self.action = Some(action);
        self.target = target;
        self.result.clear();
        self.preview = preview || action.always_previews();
        provider.stream(request, self.events_tx.clone());
        self.provider = Some(provider);
        if !host.is_invalid() {
            unsafe {
                let _ = PostMessageW(
                    host,
                    WM_QA,
                    windows::Win32::Foundation::WPARAM(0),
                    windows::Win32::Foundation::LPARAM(0),
                );
            }
        }
        Ok(())
    }

    pub fn drain(&mut self) -> bool {
        let mut changed = false;
        while let Ok(event) = self.events_rx.try_recv() {
            changed = true;
            if self.cancelled {
                continue;
            }
            match event {
                AiEvent::Text(delta) => self.result.push_str(&delta),
                AiEvent::Done => {
                    self.running = false;
                    self.accepted = true;
                }
                AiEvent::Error(err) => {
                    self.running = false;
                    self.accepted = false;
                    if self.result.is_empty() {
                        self.result = err;
                    }
                }
                AiEvent::Thinking(_) | AiEvent::Usage => {}
            }
        }
        changed
    }

    pub fn apply(&self) {
        if !self.can_apply() {
            return;
        }
        injector::replace_selection(self.target, &self.result);
    }

    pub fn cancel(&mut self) {
        self.cancelled = true;
        self.running = false;
        self.accepted = false;
        self.generation = self.generation.wrapping_add(1);
        if let Some(provider) = self.provider.as_mut() {
            provider.cancel();
        }
        while self.events_rx.try_recv().is_ok() {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capture_is_exported_for_previous_app() {
        assert!(QuickActionCoordinator::capture(HWND::default()).is_err());
    }

    #[test]
    fn cancel_blocks_apply_and_ignores_later_events() {
        let mut qa = QuickActionCoordinator::new();
        qa.target = HWND(1 as *mut core::ffi::c_void);
        qa.result = "rewritten".into();
        qa.accepted = true;
        assert!(qa.can_apply());
        qa.cancel();
        assert!(!qa.can_apply());
        assert!(!qa.is_running());
        let _ = qa.events_tx.send(AiEvent::Text("late".into()));
        let _ = qa.events_tx.send(AiEvent::Done);
        qa.drain();
        assert!(!qa.can_apply());
        assert_eq!(qa.result(), "rewritten");
    }
}
