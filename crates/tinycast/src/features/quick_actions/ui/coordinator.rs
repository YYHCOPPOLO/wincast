//! Capture previous-app selection first, then hide the palette.

use std::sync::mpsc::{self, Receiver, Sender};

use tinycast_pure::ai::{AiEvent, AiMessage, AiRequest, ModelSelection, QuickAction, Role};
use windows::Win32::Foundation::HWND;

use crate::features::ai::service::factory::ProviderFactory;
use crate::features::ai::service::provider::AiProvider;
use crate::features::snippets::service::injector;
use crate::platform::messages::WM_QA;
use windows::Win32::UI::WindowsAndMessaging::PostMessageW;

pub struct QuickActionCoordinator {
    events_tx: Sender<AiEvent>,
    events_rx: Receiver<AiEvent>,
    running: bool,
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
            running: false,
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

    /// Read selection from `target` before the caller hides the palette.
    pub fn capture(target: HWND) -> Option<String> {
        injector::capture_selection(target, true)
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
        self.running = true;
        self.action = Some(action);
        self.target = target;
        self.result.clear();
        self.preview = preview || action.always_previews();
        provider.stream(request, self.events_tx.clone());
        let _ = provider;
        if !host.is_invalid() {
            unsafe {
                let _ = PostMessageW(host, WM_QA, windows::Win32::Foundation::WPARAM(0), windows::Win32::Foundation::LPARAM(0));
            }
        }
        Ok(())
    }

    pub fn drain(&mut self) -> bool {
        let mut changed = false;
        while let Ok(event) = self.events_rx.try_recv() {
            changed = true;
            match event {
                AiEvent::Text(delta) => self.result.push_str(&delta),
                AiEvent::Done => self.running = false,
                AiEvent::Error(err) => {
                    self.running = false;
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
        if self.result.is_empty() || self.target.is_invalid() {
            return;
        }
        injector::inject_into(self.target, &self.result, None);
    }

    pub fn cancel(&mut self) {
        self.running = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capture_is_exported_for_previous_app() {
        assert!(QuickActionCoordinator::capture(HWND::default()).is_none());
    }
}
