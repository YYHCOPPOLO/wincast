//! Settings → AI. No Apple Intelligence row.

use tinycast_pure::ai::{validate_url, AiConnection, EndpointError, ModelSelection, ProviderKind};
use tinycast_pure::theme;
use windows::Win32::Graphics::Direct2D::Common::D2D_RECT_F;
use windows::Win32::Graphics::Direct2D::{ID2D1RenderTarget, D2D1_DRAW_TEXT_OPTIONS_CLIP};
use windows::Win32::Graphics::DirectWrite::DWRITE_MEASURING_MODE_NATURAL;

use crate::design_system::settings as ds;
use crate::features::ai::service::chatgpt::CodexPhase;
use crate::features::launcher::settings::items::{Formats, Rect};

const ROW_H: f32 = 52.0;
const FIXED_ROWS: usize = 8;
const EDITOR_ROW_H: f32 = 36.0;
const EDITOR_ROWS: usize = 4;
const EDITOR_FIELD_H: f32 = 22.0;
const REMOVE_HIT_W: f32 = 72.0;

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
    EDITOR_ROWS as f32 * EDITOR_ROW_H + theme::spacing::MD + 18.0
}

pub fn content_height(connection_count: usize, editing: Option<usize>) -> f32 {
    let mut h = row_y(FIXED_ROWS);
    for i in 0..connection_count {
        h += ROW_H + theme::spacing::XL;
        if editing == Some(i) {
            h += editor_height() + theme::spacing::SM;
        }
    }
    h
}

fn row_y(i: usize) -> f32 {
    if i == 0 {
        ds::form_origin()
    } else {
        ds::switch_section_next_y(false) + (i - 1) as f32 * (ROW_H + theme::spacing::XL)
    }
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
            let remove = remove_rect(width, top);
            if x >= remove.x && x < remove.x + remove.w {
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

pub fn remove_rect(width: f32, row_y: f32) -> tinycast_pure::palette_placement::DipRect {
    let pad = ds::content_pad();
    tinycast_pure::palette_placement::DipRect {
        x: width - pad - REMOVE_HIT_W,
        y: row_y,
        w: REMOVE_HIT_W,
        h: ROW_H,
    }
}

pub fn redact_ai_error(err: &str) -> String {
    let lower = err.to_ascii_lowercase();
    if lower.contains("sk-") || lower.contains("api key") || lower.contains("apikey") {
        "Couldn’t save this connection.".into()
    } else {
        err.lines().next().unwrap_or(err).to_string()
    }
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
    let pad = ds::content_pad();
    let field_x = pad + theme::size::FORM_LABEL_WIDTH + theme::spacing::LG;
    let field_w = (width - pad - field_x).max(40.0);
    let field = |row: usize| {
        let y = top + (row as f32) * EDITOR_ROW_H;
        Rect {
            x: field_x,
            y: y + (EDITOR_ROW_H - EDITOR_FIELD_H) / 2.0,
            w: field_w,
            h: EDITOR_FIELD_H,
        }
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
    key_status_lang(saved, tinycast_pure::i18n::UiLang::En)
}

fn key_status_lang(saved: bool, lang: tinycast_pure::i18n::UiLang) -> &'static str {
    if saved {
        tinycast_pure::i18n::ai_saved_on_pc(lang)
    } else {
        tinycast_pure::i18n::ai_not_set(lang)
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

pub fn default_model_title(
    selection: Option<&ModelSelection>,
    connections: &[AiConnection],
) -> String {
    default_model_title_lang(selection, connections, tinycast_pure::i18n::UiLang::En)
}

fn default_model_title_lang(
    selection: Option<&ModelSelection>,
    connections: &[AiConnection],
    lang: tinycast_pure::i18n::UiLang,
) -> String {
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
        None => tinycast_pure::i18n::ai_none(lang).into(),
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
    codex_title_lang(phase, tinycast_pure::i18n::UiLang::En)
}

fn codex_title_lang(phase: CodexPhase, lang: tinycast_pure::i18n::UiLang) -> &'static str {
    match phase {
        CodexPhase::Unavailable => tinycast_pure::i18n::ai_install_codex(lang),
        _ if !can_offer_chatgpt() => tinycast_pure::i18n::ai_not_available(lang),
        CodexPhase::Connected => tinycast_pure::i18n::ai_disconnect_chatgpt(lang),
        CodexPhase::Failed => tinycast_pure::i18n::ai_try_again(lang),
        CodexPhase::Idle => tinycast_pure::i18n::ai_connect(lang),
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
    error: Option<&str>,
    width: f32,
    scroll: f32,
    appearance: u8,
) -> windows::core::Result<()> {
    let lang = formats.lang;
    let (section, _, _) = ds::feature_switch_section(
        width,
        ds::CARD_INSET - scroll,
        tinycast_pure::i18n::pane_section_title(tinycast_pure::settings_tab::SettingsTab::Ai, lang),
        false,
    );
    ds::paint_grouped_section(
        target,
        formats.header,
        formats.caption,
        &section,
        appearance,
    )?;
    ds::paint_form_row(
        target,
        formats.body,
        formats.caption,
        tinycast_pure::i18n::ai_enable_title(lang),
        tinycast_pure::i18n::ai_enable_subtitle(lang),
        row_y(0) - scroll,
        width,
        true,
        appearance,
        ds::RowTrailing::Toggle(enabled),
    )?;
    ds::paint_form_row(
        target,
        formats.body,
        formats.caption,
        tinycast_pure::i18n::ai_web_search_title(lang),
        tinycast_pure::i18n::ai_web_search_subtitle(lang),
        row_y(1) - scroll,
        width,
        enabled,
        appearance,
        ds::RowTrailing::Toggle(web_search),
    )?;
    ds::paint_form_row(
        target,
        formats.body,
        formats.caption,
        tinycast_pure::i18n::ai_system_prompt_title(lang),
        tinycast_pure::i18n::ai_system_prompt_subtitle(lang),
        row_y(2) - scroll,
        width,
        enabled,
        appearance,
        ds::RowTrailing::Toggle(system_prompt),
    )?;
    ds::paint_form_row(
        target,
        formats.body,
        formats.caption,
        tinycast_pure::i18n::ai_opens_to_title(lang),
        "",
        row_y(3) - scroll,
        width,
        enabled,
        appearance,
        ds::RowTrailing::Label(tinycast_pure::i18n::ai_opens_to_value(opens_to != 0, lang)),
    )?;
    ds::paint_form_row(
        target,
        formats.body,
        formats.caption,
        tinycast_pure::i18n::ai_keep_conversations(lang),
        "",
        row_y(4) - scroll,
        width,
        enabled,
        appearance,
        ds::RowTrailing::Label(tinycast_pure::i18n::ai_retention_value(retention, lang)),
    )?;
    let model = default_model_title_lang(default_model, connections, lang);
    ds::paint_form_row(
        target,
        formats.body,
        formats.caption,
        tinycast_pure::i18n::ai_default_model(lang),
        "",
        row_y(5) - scroll,
        width,
        enabled,
        appearance,
        ds::RowTrailing::Label(&model),
    )?;
    ds::paint_form_row(
        target,
        formats.body,
        formats.caption,
        tinycast_pure::i18n::ai_chatgpt_subscription(lang),
        "",
        row_y(6) - scroll,
        width,
        enabled,
        appearance,
        ds::RowTrailing::Label(codex_title_lang(codex, lang)),
    )?;
    ds::paint_form_row(
        target,
        formats.body,
        formats.caption,
        tinycast_pure::i18n::ai_add_connection(lang),
        tinycast_pure::i18n::ai_add_connection_sub(lang),
        row_y(7) - scroll,
        width,
        enabled,
        appearance,
        ds::RowTrailing::None,
    )?;
    let remove = tinycast_pure::i18n::remove_label(lang);
    for (i, conn) in connections.iter().enumerate() {
        let top = connection_row_top(i, editing) - scroll;
        ds::paint_form_row(
            target,
            formats.body,
            formats.caption,
            &conn.title(),
            conn.provider.title(),
            top,
            width,
            true,
            appearance,
            ds::RowTrailing::None,
        )?;
        paint_remove_control(target, formats, remove, remove_rect(width, top), appearance)?;
        if editing == Some(i) {
            let editor_top = top + ROW_H + theme::spacing::SM;
            paint_editor_label(
                target,
                formats,
                tinycast_pure::i18n::ai_provider(lang),
                conn.provider.title(),
                editor_top,
                0,
                width,
                appearance,
            )?;
            paint_editor_label(
                target,
                formats,
                tinycast_pure::i18n::ai_base_url(lang),
                "",
                editor_top,
                1,
                width,
                appearance,
            )?;
            paint_editor_label(
                target,
                formats,
                tinycast_pure::i18n::ai_model_id(lang),
                "",
                editor_top,
                2,
                width,
                appearance,
            )?;
            paint_editor_label(
                target,
                formats,
                tinycast_pure::i18n::ai_api_key(lang),
                "",
                editor_top,
                3,
                width,
                appearance,
            )?;
            let hint = error.unwrap_or_else(|| key_status_lang(key_saved, lang));
            paint_editor_hint(target, formats, hint, editor_top, width, appearance)?;
        }
    }
    Ok(())
}

fn paint_remove_control(
    target: &ID2D1RenderTarget,
    formats: &Formats<'_>,
    label: &str,
    rect: tinycast_pure::palette_placement::DipRect,
    appearance: u8,
) -> windows::core::Result<()> {
    let brush = unsafe { target.CreateSolidColorBrush(&ds::secondary_ink(appearance), None)? };
    let wide: Vec<u16> = label.encode_utf16().collect();
    unsafe {
        target.DrawText(
            &wide,
            formats.caption,
            &D2D_RECT_F {
                left: rect.x,
                top: rect.y,
                right: rect.x + rect.w,
                bottom: rect.y + rect.h,
            },
            &brush,
            D2D1_DRAW_TEXT_OPTIONS_CLIP,
            DWRITE_MEASURING_MODE_NATURAL,
        );
    }
    Ok(())
}

fn editor_label_rect(top: f32, row: usize) -> D2D_RECT_F {
    let pad = ds::content_pad();
    let y = top + row as f32 * EDITOR_ROW_H;
    D2D_RECT_F {
        left: pad,
        top: y,
        right: pad + theme::size::FORM_LABEL_WIDTH,
        bottom: y + EDITOR_ROW_H,
    }
}

fn paint_editor_label(
    target: &ID2D1RenderTarget,
    formats: &Formats<'_>,
    title: &str,
    trailing: &str,
    top: f32,
    row: usize,
    width: f32,
    appearance: u8,
) -> windows::core::Result<()> {
    let brush = unsafe { target.CreateSolidColorBrush(&ds::primary_ink(appearance), None)? };
    let title_w: Vec<u16> = title.encode_utf16().collect();
    unsafe {
        target.DrawText(
            &title_w,
            formats.body,
            &editor_label_rect(top, row),
            &brush,
            D2D1_DRAW_TEXT_OPTIONS_CLIP,
            DWRITE_MEASURING_MODE_NATURAL,
        );
    }
    if !trailing.is_empty() {
        let pad = ds::content_pad();
        let field_x = pad + theme::size::FORM_LABEL_WIDTH + theme::spacing::LG;
        let muted = unsafe { target.CreateSolidColorBrush(&ds::secondary_ink(appearance), None)? };
        let t: Vec<u16> = trailing.encode_utf16().collect();
        let y = top + row as f32 * EDITOR_ROW_H;
        unsafe {
            target.DrawText(
                &t,
                formats.caption,
                &D2D_RECT_F {
                    left: field_x,
                    top: y,
                    right: width - pad,
                    bottom: y + EDITOR_ROW_H,
                },
                &muted,
                D2D1_DRAW_TEXT_OPTIONS_CLIP,
                DWRITE_MEASURING_MODE_NATURAL,
            );
        }
    }
    Ok(())
}

fn paint_editor_hint(
    target: &ID2D1RenderTarget,
    formats: &Formats<'_>,
    hint: &str,
    top: f32,
    width: f32,
    appearance: u8,
) -> windows::core::Result<()> {
    if hint.is_empty() {
        return Ok(());
    }
    let pad = ds::content_pad();
    let y = top + EDITOR_ROWS as f32 * EDITOR_ROW_H;
    let muted = unsafe { target.CreateSolidColorBrush(&ds::secondary_ink(appearance), None)? };
    let t: Vec<u16> = hint.encode_utf16().collect();
    unsafe {
        target.DrawText(
            &t,
            formats.caption,
            &D2D_RECT_F {
                left: pad,
                top: y,
                right: width - pad,
                bottom: y + 18.0,
            },
            &muted,
            D2D1_DRAW_TEXT_OPTIONS_CLIP,
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
        let remove = remove_rect(width, connection_row_top(0, None));
        assert_eq!(
            hit(
                remove.x - 8.0,
                connection_row_top(0, None) + 8.0,
                0.0,
                1,
                width,
                None
            ),
            Some(AiHit::Connection(0))
        );
        assert_eq!(
            hit(
                remove.x + 4.0,
                connection_row_top(0, None) + 8.0,
                0.0,
                1,
                width,
                None
            ),
            Some(AiHit::RemoveConnection(0))
        );
        assert_eq!(
            redact_ai_error("bad sk-secret"),
            "Couldn’t save this connection."
        );
        assert_eq!(redact_ai_error("Invalid URL"), "Invalid URL");
        let top = connection_row_top(0, Some(0));
        assert_eq!(
            hit(
                20.0,
                top + ROW_H + theme::spacing::SM + 4.0,
                0.0,
                1,
                width,
                Some(0)
            ),
            Some(AiHit::CycleProvider)
        );
        assert!(editor_rects(0, Some(0), width).is_some());
    }

    #[test]
    fn editor_fields_clear_the_label_column() {
        let width = theme::size::SETTINGS_DETAIL_MINIMUM;
        let rects = editor_rects(0, Some(0), width).expect("editing");
        let pad = ds::content_pad();
        let label_right = pad + theme::size::FORM_LABEL_WIDTH;
        assert!(rects.url.x >= label_right + theme::spacing::LG - 0.01);
        assert!(rects.model.x >= label_right);
        assert!(rects.key.x >= label_right);
        assert!(rects.url.y + rects.url.h <= rects.model.y + 0.01);
        assert!(rects.model.y + rects.model.h <= rects.key.y + 0.01);
        assert!(rects.key.x + rects.key.w <= width - pad + 0.01);
        let label = editor_label_rect(0.0, 1);
        assert!(label.right <= rects.url.x + 0.01);
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
