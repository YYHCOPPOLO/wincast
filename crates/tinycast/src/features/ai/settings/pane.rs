//! Settings → AI. No Apple Intelligence row.

use tinycast_pure::ai::{validate_url, AiConnection, EndpointError, ModelSelection, ProviderKind};
use tinycast_pure::theme;
use windows::Win32::Graphics::Direct2D::Common::{D2D1_COLOR_F, D2D_RECT_F};
use windows::Win32::Graphics::Direct2D::{
    ID2D1RenderTarget, D2D1_DRAW_TEXT_OPTIONS_NONE, D2D1_ROUNDED_RECT,
};
use windows::Win32::Graphics::DirectWrite::DWRITE_MEASURING_MODE_NATURAL;

use crate::features::ai::service::chatgpt::CodexPhase;
use crate::design_system::settings as ds;
use crate::features::launcher::settings::items::{Formats, Rect};

const ROW_H: f32 = 52.0;
const TOGGLE_W: f32 = 40.0;
const TOGGLE_H: f32 = 22.0;
const FIXED_ROWS: usize = 8;
const EDITOR_ROW_H: f32 = 40.0;
const EDITOR_ROWS: usize = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AiHit {
    Enable,
    WebSearch,
    SystemPrompt,
    OpensTo,
    Retention,
    DefaultModel,
    Codex,
    AddConnection,
    Connection(usize),
    RemoveConnection(usize),
    CycleProvider,
}

pub fn section_header() -> &'static str {
    "AI"
}

pub fn editor_height() -> f32 {
    EDITOR_ROWS as f32 * EDITOR_ROW_H + theme::spacing::MD
}

pub fn content_height(connection_count: usize, editing: Option<usize>) -> f32 {
    let mut h = ds::form_origin() + FIXED_ROWS as f32 * (ROW_H + theme::spacing::XL);
    for i in 0..connection_count {
        h += ROW_H + theme::spacing::XL;
        if editing == Some(i) {
            h += editor_height() + theme::spacing::SM;
        }
    }
    h
}

fn row_y(i: usize) -> f32 {
    ds::form_origin() + i as f32 * (ROW_H + theme::spacing::XL)
}

pub fn hit(
    x: f32,
    y: f32,
    scroll: f32,
    connection_count: usize,
    width: f32,
    editing: Option<usize>,
) -> Option<AiHit> {
    let y = y + scroll;
    let pad = theme::spacing::XL;
    let hits = [
        AiHit::Enable,
        AiHit::WebSearch,
        AiHit::SystemPrompt,
        AiHit::OpensTo,
        AiHit::Retention,
        AiHit::DefaultModel,
        AiHit::Codex,
        AiHit::AddConnection,
    ];
    for (i, hit) in hits.into_iter().enumerate() {
        let top = row_y(i);
        if y >= top && y < top + ROW_H {
            return Some(hit);
        }
    }
    for i in 0..connection_count {
        let top = connection_row_top(i, editing);
        if y >= top && y < top + ROW_H {
            if x >= width - pad - 72.0 {
                return Some(AiHit::RemoveConnection(i));
            }
            return Some(AiHit::Connection(i));
        }
        if editing == Some(i) {
            let editor_top = top + ROW_H + theme::spacing::SM;
            if y >= editor_top && y < editor_top + EDITOR_ROW_H {
                return Some(AiHit::CycleProvider);
            }
        }
    }
    None
}

pub fn connection_row_top(index: usize, editing: Option<usize>) -> f32 {
    let mut y = row_y(FIXED_ROWS);
    for i in 0..index {
        y += ROW_H + theme::spacing::XL;
        if editing == Some(i) {
            y += editor_height() + theme::spacing::SM;
        }
    }
    y
}

pub struct EditorRects {
    pub url: Rect,
    pub model: Rect,
    pub key: Rect,
}

pub fn editor_rects(index: usize, editing: Option<usize>, width: f32) -> Option<EditorRects> {
    if editing != Some(index) {
        return None;
    }
    let top = connection_row_top(index, editing) + ROW_H + theme::spacing::SM;
    let pad = theme::spacing::XL;
    let field = |row: usize| Rect {
        x: pad,
        y: top + (row as f32) * EDITOR_ROW_H + 16.0,
        w: (width - pad * 2.0).max(40.0),
        h: 22.0,
    };
    Some(EditorRects {
        url: field(1),
        model: field(2),
        key: field(3),
    })
}

pub fn apply_draft(
    connection: &mut AiConnection,
    provider: ProviderKind,
    base_url: &str,
    model: &str,
) -> Result<(), EndpointError> {
    let parsed = validate_url(base_url)?;
    connection.provider = provider;
    connection.base_url = parsed.as_str().trim().to_string();
    let model = model.trim();
    connection.models = if model.is_empty() {
        Vec::new()
    } else {
        vec![model.to_string()]
    };
    Ok(())
}

pub fn key_status(saved: bool) -> &'static str {
    if saved {
        "Saved on this PC"
    } else {
        "Not set"
    }
}

pub fn can_offer_chatgpt() -> bool {
    false
}

pub fn opens_to_title(value: i64) -> &'static str {
    if value == 0 {
        "Recent Conversation"
    } else {
        "A New Conversation"
    }
}

pub fn cycle_opens_to(value: i64) -> i64 {
    if value == 0 {
        1
    } else {
        0
    }
}

pub fn retention_title(days: i64) -> &'static str {
    match days {
        7 => "7 Days",
        90 => "3 Months",
        d if d < 0 => "Forever",
        _ => "30 Days",
    }
}

pub fn cycle_retention(days: i64) -> i64 {
    match days {
        7 => 30,
        30 => 90,
        90 => -1,
        _ => 7,
    }
}

pub fn default_model_title(selection: Option<&ModelSelection>, connections: &[AiConnection]) -> String {
    match selection {
        Some(ModelSelection::Api { connection, model }) => {
            let name = connections
                .iter()
                .find(|c| c.id.as_str() == connection.as_str())
                .map(|c| c.title())
                .unwrap_or_else(|| "API".into());
            format!("{name} · {model}")
        }
        Some(ModelSelection::ChatGpt { model, .. }) => format!("ChatGPT · {model}"),
        None => "None".into(),
    }
}

pub fn cycle_default_model(
    current: Option<ModelSelection>,
    connections: &[AiConnection],
    chatgpt: bool,
) -> Option<ModelSelection> {
    let mut choices = Vec::new();
    if chatgpt && can_offer_chatgpt() {
        choices.push(ModelSelection::ChatGpt {
            model: "gpt-5".into(),
            effort: None,
        });
    }
    for conn in connections {
        for model in &conn.models {
            choices.push(ModelSelection::Api {
                connection: conn.id.clone(),
                model: model.clone(),
            });
        }
    }
    if choices.is_empty() {
        return None;
    }
    let next = match current {
        Some(cur) => choices
            .iter()
            .position(|c| c == &cur)
            .map(|i| (i + 1) % choices.len())
            .unwrap_or(0),
        None => 0,
    };
    Some(choices[next].clone())
}

pub fn cycle_provider(kind: ProviderKind) -> ProviderKind {
    match kind {
        ProviderKind::OpenAi => ProviderKind::Anthropic,
        ProviderKind::Anthropic => ProviderKind::Gemini,
        ProviderKind::Gemini => ProviderKind::OpenRouter,
        ProviderKind::OpenRouter => ProviderKind::OpenAiCompatible,
        ProviderKind::OpenAiCompatible => ProviderKind::OpenAi,
    }
}

pub fn codex_title(phase: CodexPhase) -> &'static str {
    match phase {
        CodexPhase::Unavailable => "Install Codex CLI…",
        _ if !can_offer_chatgpt() => "Not available yet",
        CodexPhase::Connected => "Disconnect ChatGPT",
        CodexPhase::Failed => "Try Again",
        CodexPhase::Idle => "Connect",
    }
}

pub fn paint(
    target: &ID2D1RenderTarget,
    formats: &Formats<'_>,
    enabled: bool,
    web_search: bool,
    system_prompt: bool,
    opens_to: i64,
    retention: i64,
    default_model: Option<&ModelSelection>,
    connections: &[AiConnection],
    codex: CodexPhase,
    editing: Option<usize>,
    key_saved: bool,
    width: f32,
    scroll: f32,
) -> windows::core::Result<()> {
    let (section, _, _) =
        ds::feature_switch_section(width, ds::CARD_INSET - scroll, section_header(), false);
    ds::paint_grouped_section(target, formats.header, formats.caption, &section, 0)?;
    paint_toggle(
        target,
        formats,
        "Enable AI",
        "Chat with the model you choose; nothing is loaded or sent until it is on.",
        enabled,
        row_y(0) - scroll,
        width,
    )?;
    paint_toggle(
        target,
        formats,
        "Web search",
        "Let OpenRouter models search the web.",
        web_search,
        row_y(1) - scroll,
        width,
    )?;
    paint_toggle(
        target,
        formats,
        "System prompt",
        "Send Tinycast’s preamble and your extra instructions.",
        system_prompt,
        row_y(2) - scroll,
        width,
    )?;
    paint_row(
        target,
        formats,
        "Opens to",
        opens_to_title(opens_to),
        row_y(3) - scroll,
        width,
    )?;
    paint_row(
        target,
        formats,
        "Keep conversations",
        retention_title(retention),
        row_y(4) - scroll,
        width,
    )?;
    let model = default_model_title(default_model, connections);
    paint_row(
        target,
        formats,
        "Default model",
        &model,
        row_y(5) - scroll,
        width,
    )?;
    paint_row(
        target,
        formats,
        "ChatGPT subscription",
        codex_title(codex),
        row_y(6) - scroll,
        width,
    )?;
    paint_row(
        target,
        formats,
        "Add API connection",
        "OpenAI, Anthropic, Gemini, OpenRouter, or compatible.",
        row_y(7) - scroll,
        width,
    )?;
    for (i, conn) in connections.iter().enumerate() {
        let top = connection_row_top(i, editing) - scroll;
        paint_row(
            target,
            formats,
            &conn.title(),
            conn.provider.title(),
            top,
            width,
        )?;
        if editing == Some(i) {
            let editor_top = top + ROW_H + theme::spacing::SM;
            paint_row(
                target,
                formats,
                "Provider",
                conn.provider.title(),
                editor_top,
                width,
            )?;
            paint_row(
                target,
                formats,
                "Base URL",
                "",
                editor_top + EDITOR_ROW_H,
                width,
            )?;
            paint_row(
                target,
                formats,
                "Model id",
                "",
                editor_top + EDITOR_ROW_H * 2.0,
                width,
            )?;
            paint_row(
                target,
                formats,
                "API key",
                key_status(key_saved),
                editor_top + EDITOR_ROW_H * 3.0,
                width,
            )?;
        }
    }
    Ok(())
}

fn paint_toggle(
    target: &ID2D1RenderTarget,
    formats: &Formats<'_>,
    title: &str,
    subtitle: &str,
    on: bool,
    y: f32,
    width: f32,
) -> windows::core::Result<()> {
    paint_row(target, formats, title, subtitle, y, width)?;
    let pad = theme::spacing::XL;
    let toggle = D2D1_ROUNDED_RECT {
        rect: D2D_RECT_F {
            left: width - pad - TOGGLE_W,
            top: y + (ROW_H - TOGGLE_H) / 2.0,
            right: width - pad,
            bottom: y + (ROW_H - TOGGLE_H) / 2.0 + TOGGLE_H,
        },
        radiusX: TOGGLE_H / 2.0,
        radiusY: TOGGLE_H / 2.0,
    };
    let fill = if on {
        D2D1_COLOR_F {
            r: 0.2,
            g: 0.55,
            b: 1.0,
            a: 1.0,
        }
    } else {
        D2D1_COLOR_F {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 0.18,
        }
    };
    let brush = unsafe { target.CreateSolidColorBrush(&fill, None)? };
    unsafe { target.FillRoundedRectangle(&toggle, &brush) };
    Ok(())
}

fn paint_row(
    target: &ID2D1RenderTarget,
    formats: &Formats<'_>,
    title: &str,
    subtitle: &str,
    y: f32,
    width: f32,
) -> windows::core::Result<()> {
    let pad = theme::spacing::XL;
    let white = D2D1_COLOR_F {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a: 0.92,
    };
    let muted = D2D1_COLOR_F {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a: 0.55,
    };
    let brush = unsafe { target.CreateSolidColorBrush(&white, None)? };
    let title_w: Vec<u16> = title.encode_utf16().collect();
    unsafe {
        target.DrawText(
            &title_w,
            formats.body,
            &D2D_RECT_F {
                left: pad,
                top: y + 8.0,
                right: width - pad - TOGGLE_W - 8.0,
                bottom: y + 28.0,
            },
            &brush,
            D2D1_DRAW_TEXT_OPTIONS_NONE,
            DWRITE_MEASURING_MODE_NATURAL,
        );
    }
    let muted_brush = unsafe { target.CreateSolidColorBrush(&muted, None)? };
    let sub: Vec<u16> = subtitle.encode_utf16().collect();
    unsafe {
        target.DrawText(
            &sub,
            formats.caption,
            &D2D_RECT_F {
                left: pad,
                top: y + 28.0,
                right: width - pad - TOGGLE_W - 8.0,
                bottom: y + ROW_H - 4.0,
            },
            &muted_brush,
            D2D1_DRAW_TEXT_OPTIONS_NONE,
            DWRITE_MEASURING_MODE_NATURAL,
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pane_has_no_apple_intelligence_copy() {
        assert_ne!(opens_to_title(0), "Apple Intelligence");
        assert!(!retention_title(30).contains("Apple"));
        let conn = AiConnection::new(ProviderKind::OpenAi);
        let title = default_model_title(None, &[conn]);
        assert!(!title.to_lowercase().contains("apple"));
    }

    #[test]
    fn new_connection_has_no_catalog_model() {
        let conn = AiConnection::new(ProviderKind::OpenAiCompatible);
        assert!(conn.models.is_empty());
        assert!(!can_offer_chatgpt());
    }

    #[test]
    fn draft_accepts_loopback_and_rejects_plain_http() {
        let mut conn = AiConnection::new(ProviderKind::OpenAiCompatible);
        apply_draft(
            &mut conn,
            ProviderKind::OpenAiCompatible,
            "http://127.0.0.1:11434/v1",
            "llama3",
        )
        .unwrap();
        assert_eq!(conn.base_url, "http://127.0.0.1:11434/v1");
        assert_eq!(conn.models, vec!["llama3".to_string()]);
        assert!(apply_draft(
            &mut conn,
            ProviderKind::OpenAiCompatible,
            "http://example.com/v1",
            "x",
        )
        .is_err());
    }

    #[test]
    fn connection_click_opens_editor_not_remove() {
        let width = 400.0;
        assert_eq!(
            hit(20.0, connection_row_top(0, None) + 8.0, 0.0, 1, width, None),
            Some(AiHit::Connection(0))
        );
        assert_eq!(
            hit(
                width - 20.0,
                connection_row_top(0, None) + 8.0,
                0.0,
                1,
                width,
                None
            ),
            Some(AiHit::RemoveConnection(0))
        );
        let top = connection_row_top(0, Some(0));
        assert_eq!(
            hit(20.0, top + ROW_H + theme::spacing::SM + 4.0, 0.0, 1, width, Some(0)),
            Some(AiHit::CycleProvider)
        );
        assert!(editor_rects(0, Some(0), width).is_some());
    }

    #[test]
    fn chatgpt_is_not_offered_as_a_model() {
        let conn = AiConnection::new(ProviderKind::OpenAi);
        let mut with_model = conn.clone();
        with_model.models.push("gpt-4.1-mini".into());
        let next = cycle_default_model(None, &[with_model], true);
        assert!(matches!(next, Some(ModelSelection::Api { .. })));
        assert!(!codex_title(CodexPhase::Idle).eq_ignore_ascii_case("connect"));
    }
}
