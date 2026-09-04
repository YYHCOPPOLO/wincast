//! AI Chat / History palette rows. Assistant text is shown as markdown-ish lines; user is literal.

use tinycast_pure::ai::{ChatConversation, ChatMessage, ChatRole, ChatState};
use tinycast_pure::palette_placement::DipRect;
use tinycast_pure::theme;

use crate::features::launcher::ui::list::PaintItem;

pub const MODEL_BUTTON_WIDTH: f32 = 168.0;

pub fn model_trailing_width() -> f32 {
    theme::spacing::MD + MODEL_BUTTON_WIDTH
}

pub fn model_button_rect(panel_w: f32) -> DipRect {
    let h = theme::size::BAR_BUTTON_HEIGHT;
    DipRect {
        x: panel_w - theme::spacing::XXL - MODEL_BUTTON_WIDTH,
        y: theme::size::HEADER_PADDING + (theme::size::HEADER_HEIGHT - h) / 2.0,
        w: MODEL_BUTTON_WIDTH,
        h,
    }
}

pub fn chat_placeholder() -> &'static str {
    "Message"
}

pub fn history_placeholder() -> &'static str {
    "Search chats"
}

pub fn paint_chat(
    messages: &[ChatMessage],
    notice: Option<&str>,
    thinking: bool,
) -> Vec<PaintItem> {
    let mut items = Vec::new();
    for (i, message) in messages.iter().enumerate() {
        let title = match message.role {
            ChatRole::User => message.text.clone(),
            ChatRole::Assistant => display_assistant(&message.text),
        };
        let trailing = match message.state {
            ChatState::Streaming if thinking => "Thinking".into(),
            ChatState::Streaming => String::new(),
            ChatState::Failed => "Failed".into(),
            ChatState::Complete => String::new(),
        };
        items.push(PaintItem::Row {
            title,
            alias: Some(if message.role == ChatRole::User {
                "You".into()
            } else {
                "Tinycast".into()
            }),
            trailing,
            keycap: None,
            icon_source: None,
            selected: i + 1 == messages.len(),
        });
    }
    if let Some(notice) = notice.filter(|s| !s.is_empty()) {
        items.push(PaintItem::Row {
            title: notice.to_string(),
            alias: None,
            trailing: String::new(),
            keycap: None,
            icon_source: None,
            selected: false,
        });
    }
    items
}

pub fn paint_history(rows: &[ChatConversation], selection: usize) -> Vec<PaintItem> {
    rows.iter()
        .enumerate()
        .map(|(i, row)| PaintItem::Row {
            title: row.title.clone(),
            alias: None,
            trailing: row.preview.clone(),
            keycap: None,
            icon_source: None,
            selected: i == selection,
        })
        .collect()
}

fn display_assistant(text: &str) -> String {
    let mut out = String::new();
    let mut in_fence = false;
    for line in text.lines() {
        if line.trim_start().starts_with("```") {
            in_fence = !in_fence;
            continue;
        }
        let rendered = if in_fence {
            line.to_string()
        } else {
            strip_inline(line)
        };
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str(&rendered);
    }
    if out.is_empty() {
        text.to_string()
    } else {
        out
    }
}

fn strip_inline(line: &str) -> String {
    let trimmed = line.trim_start_matches('#').trim_start();
    trimmed.replace("**", "").replace('`', "")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_stays_literal_assistant_drops_fences() {
        let user = ChatMessage::user("**keep stars**", 1);
        let mut assistant = tinycast_pure::ai::ChatMessage::assistant_streaming(1);
        assistant.state = ChatState::Complete;
        assistant.text = "```\ncode\n```\n**bold**".into();
        let items = paint_chat(&[user, assistant], None, false);
        match &items[0] {
            PaintItem::Row { title, .. } => assert_eq!(title, "**keep stars**"),
            _ => panic!("row"),
        }
        match &items[1] {
            PaintItem::Row { title, .. } => {
                assert!(title.contains("code"));
                assert!(title.contains("bold"));
                assert!(!title.contains('`'));
            }
            _ => panic!("row"),
        }
    }
}
