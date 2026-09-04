use crate::ai::request::{AiMessage, Role};
use crate::ai::Uuid;

pub const DEFAULT_TEXT_BUDGET: usize = 100_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChatRole {
    User,
    Assistant,
}

impl ChatRole {
    pub fn as_str(self) -> &'static str {
        match self {
            ChatRole::User => "user",
            ChatRole::Assistant => "assistant",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "user" => Some(ChatRole::User),
            "assistant" => Some(ChatRole::Assistant),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChatState {
    Streaming,
    Complete,
    Failed,
}

impl ChatState {
    pub fn as_str(self) -> &'static str {
        match self {
            ChatState::Streaming => "streaming",
            ChatState::Complete => "complete",
            ChatState::Failed => "failed",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "streaming" => Some(ChatState::Streaming),
            "complete" => Some(ChatState::Complete),
            "failed" => Some(ChatState::Failed),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChatMessage {
    pub id: Uuid,
    pub role: ChatRole,
    pub text: String,
    pub state: ChatState,
    pub sent_at: i64,
}

impl ChatMessage {
    pub fn user(text: impl Into<String>, now: i64) -> Self {
        Self {
            id: Uuid::generate(),
            role: ChatRole::User,
            text: text.into(),
            state: ChatState::Complete,
            sent_at: now,
        }
    }

    pub fn assistant_streaming(now: i64) -> Self {
        Self {
            id: Uuid::generate(),
            role: ChatRole::Assistant,
            text: String::new(),
            state: ChatState::Streaming,
            sent_at: now,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChatConversation {
    pub id: Uuid,
    pub title: String,
    pub preview: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub message_count: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChatSession {
    pub id: Uuid,
    pub created_at: i64,
    pub updated_at: i64,
    pub messages: Vec<ChatMessage>,
}

impl ChatSession {
    pub fn new(now: i64) -> Self {
        Self {
            id: Uuid::generate(),
            created_at: now,
            updated_at: now,
            messages: Vec::new(),
        }
    }

    pub fn title(&self) -> String {
        self.messages
            .iter()
            .find(|m| m.role == ChatRole::User)
            .map(|m| summary(&m.text, 72))
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "New Chat".into())
    }

    pub fn preview(&self) -> String {
        self.messages
            .iter()
            .rev()
            .find(|m| !m.text.is_empty())
            .map(|m| summary(&m.text, 120))
            .unwrap_or_default()
    }

    pub fn summary(&self) -> ChatConversation {
        ChatConversation {
            id: self.id.clone(),
            title: self.title(),
            preview: self.preview(),
            created_at: self.created_at,
            updated_at: self.updated_at,
            message_count: self.messages.len(),
        }
    }

    pub fn append(&mut self, message: ChatMessage) {
        self.updated_at = self.updated_at.max(message.sent_at);
        self.messages.push(message);
    }

    pub fn replace_last(&mut self, message: ChatMessage, now: i64) {
        if let Some(last) = self.messages.last_mut() {
            *last = message;
            self.updated_at = self.updated_at.max(now);
        }
    }

    pub fn request_messages(&self, text_budget: usize) -> Vec<AiMessage> {
        let ready: Vec<AiMessage> = self
            .messages
            .iter()
            .filter(|m| m.role == ChatRole::User || m.state == ChatState::Complete)
            .map(|m| AiMessage {
                role: if m.role == ChatRole::User {
                    Role::User
                } else {
                    Role::Assistant
                },
                text: m.text.clone(),
            })
            .collect();
        bounded_context(&ready, text_budget)
    }
}

/// Newest user turn is sent whole. Older text walks newest-first into `text_budget`.
pub fn bounded_context(messages: &[AiMessage], text_budget: usize) -> Vec<AiMessage> {
    let Some(newest) = messages
        .iter()
        .rposition(|m| m.role == Role::User)
    else {
        return messages.to_vec();
    };
    let mut remaining = text_budget as i64;
    let mut tail = Vec::new();
    for message in &messages[newest + 1..] {
        remaining -= message.text.len() as i64;
        tail.push(AiMessage {
            role: message.role,
            text: message.text.clone(),
        });
    }
    let mut head = Vec::new();
    for message in messages[..newest].iter().rev() {
        remaining -= message.text.len() as i64;
        if remaining < 0 {
            break;
        }
        head.push(AiMessage {
            role: message.role,
            text: message.text.clone(),
        });
    }
    while head.last().map(|m| m.role) == Some(Role::Assistant) {
        head.pop();
    }
    let prompt = &messages[newest];
    head.reverse();
    head.push(AiMessage {
        role: prompt.role,
        text: prompt.text.clone(),
    });
    head.extend(tail);
    head
}

fn summary(text: &str, limit: usize) -> String {
    let collapsed: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
    collapsed.chars().take(limit).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn newest_user_message_is_never_truncated() {
        let old = AiMessage::user("a".repeat(80_000));
        let mid = AiMessage::assistant("b".repeat(80_000));
        let newest = AiMessage::user("just typed");
        let bounded = bounded_context(&[old, mid, newest], DEFAULT_TEXT_BUDGET);
        assert_eq!(bounded.last().map(|m| m.text.as_str()), Some("just typed"));
        assert!(bounded.iter().map(|m| m.text.len()).sum::<usize>() <= DEFAULT_TEXT_BUDGET + 80_000);
    }
}
