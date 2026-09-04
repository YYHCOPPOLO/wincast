//! Chat actions. Views render state; mutations go through here.

use std::sync::mpsc::{self, Receiver, Sender};

use tinycast_pure::ai::{
    compose_instructions, should_resume, AiConnection, AiEvent, AiRequest, ChatMessage,
    ChatSession, ChatState, ModelSelection, OpenPolicy, Uuid, DEFAULT_TEXT_BUDGET,
};
use tinycast_pure::palette_menu::MenuItem;

use crate::features::ai::service::factory::ProviderFactory;
use crate::features::ai::service::history::ChatHistoryStore;
use crate::features::ai::service::provider::{AiProvider, HttpAiProvider};
use crate::platform::clock::unix_now;

pub const ID_AI_NEW: &str = "ai-new";
pub const ID_AI_HISTORY: &str = "ai-history";
pub const ID_AI_SETTINGS: &str = "ai-settings";
pub const ID_AI_STOP: &str = "ai-stop";
pub const ID_AI_COPY: &str = "ai-copy";

pub const MODEL_SLOT_IDS: [&str; 24] = [
    "ai-m0", "ai-m1", "ai-m2", "ai-m3", "ai-m4", "ai-m5", "ai-m6", "ai-m7", "ai-m8", "ai-m9",
    "ai-m10", "ai-m11", "ai-m12", "ai-m13", "ai-m14", "ai-m15", "ai-m16", "ai-m17", "ai-m18",
    "ai-m19", "ai-m20", "ai-m21", "ai-m22", "ai-m23",
];

pub struct AiChatCoordinator {
    pub session: ChatSession,
    pub draft: String,
    pub notice: Option<String>,
    pub thinking: bool,
    streaming: bool,
    events_tx: Sender<AiEvent>,
    events_rx: Receiver<AiEvent>,
    history: ChatHistoryStore,
    provider: Option<HttpAiProvider>,
}

impl AiChatCoordinator {
    pub fn new() -> Self {
        let (events_tx, events_rx) = mpsc::channel();
        Self {
            session: ChatSession::new(unix_now()),
            draft: String::new(),
            notice: None,
            thinking: false,
            streaming: false,
            events_tx,
            events_rx,
            history: ChatHistoryStore::in_roaming(),
            provider: None,
        }
    }

    pub fn is_streaming(&self) -> bool {
        self.streaming
    }

    pub fn history(&self) -> &ChatHistoryStore {
        &self.history
    }

    pub fn apply_enabled(&mut self, enabled: bool, retention_days: i64) {
        if !enabled {
            self.cancel();
            self.session = ChatSession::new(unix_now());
            self.notice = None;
            self.thinking = false;
            self.history.close();
            return;
        }
        self.history.load();
        self.apply_retention(true, retention_days);
    }

    pub fn apply_retention(&mut self, enabled: bool, retention_days: i64) {
        if !enabled || retention_days < 0 {
            return;
        }
        let cutoff = unix_now() - retention_days * 86_400;
        self.history.prune_before(cutoff);
    }

    pub fn apply_open_policy(&mut self, policy: OpenPolicy) {
        if self.streaming {
            return;
        }
        let resident = !self.session.messages.is_empty();
        let last = if resident {
            self.session.updated_at
        } else {
            self.history
                .conversations()
                .first()
                .map(|c| c.updated_at)
                .unwrap_or(0)
        };
        if should_resume(policy, last, unix_now()) {
            if !resident {
                let id = self.history.conversations().first().map(|c| c.id.clone());
                if let Some(id) = id {
                    self.open(&id);
                }
            }
        } else if resident {
            self.start_new();
        }
    }

    pub fn start_new(&mut self) {
        self.cancel();
        self.session = ChatSession::new(unix_now());
        self.notice = None;
        self.thinking = false;
        self.draft.clear();
    }

    pub fn open(&mut self, id: &Uuid) -> bool {
        self.cancel();
        let Some(session) = self.history.session(id) else {
            return false;
        };
        self.session = session;
        self.notice = None;
        self.thinking = false;
        true
    }

    pub fn delete(&mut self, id: &Uuid) {
        if self.session.id.as_str() == id.as_str() {
            self.start_new();
        }
        self.history.delete(id);
    }

    pub fn send(
        &mut self,
        input: &str,
        factory: &ProviderFactory,
        selection: Option<&ModelSelection>,
        connections: &[AiConnection],
        instructions: Option<&str>,
        prompt_enabled: bool,
        web_search: bool,
    ) -> bool {
        let text = input.trim();
        if text.is_empty() || self.streaming {
            return false;
        }
        self.notice = None;
        let now = unix_now();
        self.session.append(ChatMessage::user(text, now));
        let Some(selection) = selection else {
            self.notice = Some("Choose a default AI model in Settings.".into());
            return false;
        };
        let provider = match factory.http_provider(selection, connections) {
            Ok(provider) => provider,
            Err(err) => {
                self.notice = Some(err);
                return false;
            }
        };
        let request = AiRequest {
            instructions: compose_instructions(instructions, prompt_enabled),
            messages: self.session.request_messages(DEFAULT_TEXT_BUDGET),
            max_output_tokens: 4096,
            web_search,
        };
        self.session.append(ChatMessage::assistant_streaming(now));
        self.streaming = true;
        self.thinking = false;
        self.history.save(&self.session);
        let mut provider = provider;
        provider.stream(request, self.events_tx.clone());
        self.provider = Some(provider);
        true
    }

    pub fn cancel(&mut self) {
        if let Some(provider) = self.provider.as_mut() {
            provider.cancel();
        }
        if !self.streaming {
            return;
        }
        self.finish_last(ChatState::Failed, "Cancelled");
    }

    pub fn drain_events(&mut self) -> bool {
        let mut changed = false;
        while let Ok(event) = self.events_rx.try_recv() {
            changed = true;
            self.receive(event);
        }
        changed
    }

    pub fn last_assistant_text(&self) -> Option<String> {
        self.session
            .messages
            .iter()
            .rev()
            .find(|m| m.role == tinycast_pure::ai::ChatRole::Assistant && !m.text.is_empty())
            .map(|m| m.text.clone())
    }

    pub fn actions(&self) -> Vec<MenuItem> {
        let mut items = vec![
            MenuItem {
                id: ID_AI_NEW,
                label: "New Chat".into(),
                shortcut: Some("Ctrl+N"),
            },
            MenuItem {
                id: ID_AI_HISTORY,
                label: "Chat History".into(),
                shortcut: None,
            },
            MenuItem {
                id: ID_AI_SETTINGS,
                label: "AI Settings".into(),
                shortcut: None,
            },
        ];
        if self.streaming {
            items.push(MenuItem {
                id: ID_AI_STOP,
                label: "Stop Response".into(),
                shortcut: Some("↵"),
            });
        }
        if self.last_assistant_text().is_some() {
            items.push(MenuItem {
                id: ID_AI_COPY,
                label: "Copy Last Response".into(),
                shortcut: None,
            });
        }
        items
    }

    fn receive(&mut self, event: AiEvent) {
        match event {
            AiEvent::Text(delta) => {
                self.thinking = false;
                if let Some(last) = self.session.messages.last_mut() {
                    if last.state == ChatState::Streaming {
                        last.text.push_str(&delta);
                    }
                }
            }
            AiEvent::Thinking(on) => self.thinking = on,
            AiEvent::Usage => {}
            AiEvent::Done => self.finish_last(ChatState::Complete, ""),
            AiEvent::Error(err) => self.finish_last(ChatState::Failed, &err),
        }
    }

    fn finish_last(&mut self, state: ChatState, fallback: &str) {
        self.streaming = false;
        self.thinking = false;
        if let Some(last) = self.session.messages.last_mut() {
            if last.state == ChatState::Streaming {
                if last.text.is_empty() && !fallback.is_empty() {
                    last.text = fallback.to_string();
                }
                last.state = state;
            }
        }
        if state == ChatState::Failed && !fallback.is_empty() {
            self.notice = Some(fallback.to_string());
        }
        self.history.save(&self.session);
    }
}

pub fn open_policy(opens_to: i64, after_minutes: i64) -> OpenPolicy {
    if opens_to != 0 {
        OpenPolicy::New
    } else if after_minutes < 0 {
        OpenPolicy::Recent {
            after_minutes: u32::MAX,
        }
    } else {
        OpenPolicy::Recent {
            after_minutes: after_minutes.max(0) as u32,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ctrl_k_owns_new_history_settings() {
        let chat = AiChatCoordinator::new();
        let actions = chat.actions();
        let labels: Vec<_> = actions.iter().map(|i| i.label.as_str()).collect();
        assert!(labels.contains(&"New Chat"));
        assert!(labels.contains(&"Chat History"));
        assert!(labels.contains(&"AI Settings"));
        assert!(!labels.contains(&"Stop Response"));
    }
}
