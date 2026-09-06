//! AI Chat / History palette chrome. Transcript paint lives in `design_system::chat`.

use tinycast_pure::ai::{ChatConversation, ChatMessage, ChatRole};
use tinycast_pure::palette_mode::PaletteMode;
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
    PaletteMode::Ai.placeholder()
}

pub fn history_placeholder() -> &'static str {
    PaletteMode::AiHistory.placeholder()
}

pub fn paint_chat(
    messages: &[ChatMessage],
    notice: Option<&str>,
    thinking: bool,
) -> Vec<PaintItem> {
    let _ = (messages, notice, thinking);
    Vec::new()
}

pub fn transcript_content_height(
    messages: &[ChatMessage],
    notice: Option<&str>,
    panel_w: f32,
) -> f32 {
    tinycast_pure::layout::list::chat_transcript_height(
        messages
            .iter()
            .map(|m| (m.role == ChatRole::User, m.text.as_str())),
        notice,
        panel_w,
    )
}

pub fn assistant_plain(text: &str) -> String {
    display_assistant(text)
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
    fn chat_placeholder_is_ask_anything() {
        assert_eq!(chat_placeholder(), "Ask anything…");
        assert_eq!(history_placeholder(), "Search chats…");
    }

    #[test]
    fn paint_chat_does_not_emit_you_alias_rows() {
        let user = ChatMessage::user("hello", 1);
        let items = paint_chat(&[user], None, false);
        assert!(items.is_empty());
    }

    #[test]
    fn transcript_height_includes_bottom_pad() {
        let empty = transcript_content_height(&[], None, 750.0);
        assert!(
            (empty
                - (tinycast_pure::layout::list::chat_pad_top()
                    + tinycast_pure::layout::list::chat_pad_bottom()))
            .abs()
                < 0.01
        );
        let user = ChatMessage::user("hello", 1);
        assert!(transcript_content_height(&[user], None, 750.0) > empty);
    }

    #[test]
    fn user_stays_literal_assistant_drops_fences() {
        let user = ChatMessage::user("**keep stars**", 1);
        assert_eq!(user.text, "**keep stars**");
        let rendered = assistant_plain("```\ncode\n```\n**bold**");
        assert!(rendered.contains("code"));
        assert!(rendered.contains("bold"));
        assert!(!rendered.contains('`'));
    }
}
