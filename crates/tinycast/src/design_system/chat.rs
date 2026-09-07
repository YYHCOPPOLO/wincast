use tinycast_pure::ai::{ChatMessage, ChatRole, ChatState};
use tinycast_pure::layout::list as chat_layout;
use tinycast_pure::theme;
use windows::core::w;
use windows::Win32::Graphics::Direct2D::Common::D2D_RECT_F;
use windows::Win32::Graphics::Direct2D::{
    ID2D1RenderTarget, D2D1_ANTIALIAS_MODE_PER_PRIMITIVE, D2D1_DRAW_TEXT_OPTIONS_CLIP,
};
use windows::Win32::Graphics::DirectWrite::{
    IDWriteFactory, IDWriteTextFormat, DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_NORMAL,
    DWRITE_FONT_WEIGHT_REGULAR, DWRITE_MEASURING_MODE_NATURAL, DWRITE_PARAGRAPH_ALIGNMENT_NEAR,
    DWRITE_TEXT_ALIGNMENT_LEADING, DWRITE_TEXT_ALIGNMENT_TRAILING, DWRITE_TEXT_METRICS,
    DWRITE_WORD_WRAPPING_WRAP,
};

use super::appearance;
use super::fill_squircle;
use super::Fonts;

pub struct ChatPaint<'a> {
    pub messages: &'a [ChatMessage],
    pub notice: Option<&'a str>,
    pub scroll: f32,
}

pub fn paint_transcript(
    target: &ID2D1RenderTarget,
    fonts: &Fonts,
    chat: ChatPaint<'_>,
    panel_w: f32,
    panel_h: f32,
    appearance: u8,
) -> windows::core::Result<()> {
    let origin = chat_layout::content_top();
    let clip_top = chat_layout::paint_clip_top();
    let bottom = panel_h.max(origin);
    let clip = D2D_RECT_F {
        left: 0.0,
        top: clip_top,
        right: panel_w,
        bottom,
    };
    unsafe {
        target.PushAxisAlignedClip(&clip, D2D1_ANTIALIAS_MODE_PER_PRIMITIVE);
    }
    let wrap_lead = wrap_format(&fonts.dwrite, false)?;
    let wrap_trail = wrap_format(&fonts.dwrite, true)?;
    let mut y = origin + chat_layout::chat_pad_top() - chat.scroll;
    let gap = chat_layout::chat_message_gap();
    for message in chat.messages {
        let h = paint_message(
            target,
            fonts,
            &wrap_lead,
            &wrap_trail,
            message,
            y,
            panel_w,
            appearance,
        )?;
        y += h + gap;
    }
    if let Some(notice) = chat.notice.filter(|s| !s.is_empty()) {
        let secondary = theme::colors::ramp_rgba(
            appearance,
            theme::colors::TEXT_SECONDARY_ALPHA,
            theme::colors::TEXT_SECONDARY_ALPHA,
        );
        let rect = chat_layout::chat_assistant_rect(panel_w, y);
        draw_wrapped(
            target,
            &wrap_lead,
            notice,
            rect.x,
            y,
            rect.w,
            48.0,
            secondary,
        )?;
    }
    unsafe {
        target.PopAxisAlignedClip();
    }
    Ok(())
}

fn paint_message(
    target: &ID2D1RenderTarget,
    fonts: &Fonts,
    wrap_lead: &IDWriteTextFormat,
    wrap_trail: &IDWriteTextFormat,
    message: &ChatMessage,
    y: f32,
    panel_w: f32,
    appearance: u8,
) -> windows::core::Result<f32> {
    let failed = message.state == ChatState::Failed;
    let ink = if failed {
        (0.86, 0.22, 0.22, 1.0)
    } else {
        theme::colors::ramp_rgba(
            appearance,
            theme::colors::TEXT_PRIMARY_ALPHA,
            theme::colors::TEXT_PRIMARY_ALPHA,
        )
    };
    let text = if message.role == ChatRole::Assistant {
        assistant_plain(&message.text)
    } else {
        message.text.clone()
    };
    let display = if text.is_empty() && message.state == ChatState::Streaming {
        "Thinking"
    } else {
        text.as_str()
    };

    if message.role == ChatRole::User {
        let pad_x = theme::spacing::XL;
        let pad_y = theme::spacing::MD;
        let max_inner = (panel_w - chat_layout::chat_pad_x() * 2.0 - pad_x * 2.0).max(40.0);
        let (tw, th) = measure(&fonts.dwrite, wrap_trail, display, max_inner);
        let bubble_w = (tw + pad_x * 2.0).clamp(36.0, max_inner + pad_x * 2.0);
        let bubble_h = (th + pad_y * 2.0).max(theme::size::BAR_BUTTON_HEIGHT);
        let mut bubble = chat_layout::chat_user_bubble(panel_w, y, bubble_w);
        bubble.h = bubble_h;
        fill_squircle(
            target,
            bubble,
            theme::radius::ROW,
            theme::colors::ramp_rgba(
                appearance,
                theme::colors::CONTROL_SURFACE_DARK_ALPHA,
                theme::colors::CONTROL_SURFACE_LIGHT_ALPHA,
            ),
        )?;
        draw_wrapped(
            target,
            wrap_trail,
            display,
            bubble.x + pad_x,
            bubble.y + pad_y,
            (bubble.w - pad_x * 2.0).max(8.0),
            (bubble.h - pad_y * 2.0).max(8.0),
            ink,
        )?;
        Ok(bubble_h)
    } else {
        let pad_x = theme::spacing::SM;
        let pad_y = theme::spacing::MD;
        let mut rect = chat_layout::chat_assistant_rect(panel_w, y);
        let inner_w = (rect.w - pad_x * 2.0).max(40.0);
        let (_tw, th) = measure(&fonts.dwrite, wrap_lead, display, inner_w);
        let h = (th + pad_y * 2.0).max(theme::typography::ROW_TITLE + pad_y * 2.0);
        rect.h = h;
        draw_wrapped(
            target,
            wrap_lead,
            display,
            rect.x + pad_x,
            rect.y + pad_y,
            inner_w,
            (h - pad_y * 2.0).max(8.0),
            ink,
        )?;
        Ok(h)
    }
}

fn assistant_plain(text: &str) -> String {
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
            let trimmed = line.trim_start_matches('#').trim_start();
            trimmed.replace("**", "").replace('`', "")
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

fn wrap_format(dwrite: &IDWriteFactory, trailing: bool) -> windows::core::Result<IDWriteTextFormat> {
    let format = unsafe {
        dwrite.CreateTextFormat(
            w!("Microsoft YaHei UI"),
            None,
            DWRITE_FONT_WEIGHT_REGULAR,
            DWRITE_FONT_STYLE_NORMAL,
            DWRITE_FONT_STRETCH_NORMAL,
            theme::typography::ROW_TITLE,
            w!("en-US"),
        )?
    };
    unsafe {
        format.SetWordWrapping(DWRITE_WORD_WRAPPING_WRAP)?;
        format.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_NEAR)?;
        format.SetTextAlignment(if trailing {
            DWRITE_TEXT_ALIGNMENT_TRAILING
        } else {
            DWRITE_TEXT_ALIGNMENT_LEADING
        })?;
    }
    Ok(format)
}

fn measure(dwrite: &IDWriteFactory, format: &IDWriteTextFormat, text: &str, max_w: f32) -> (f32, f32) {
    if text.is_empty() {
        return (0.0, theme::typography::ROW_TITLE);
    }
    let wide: Vec<u16> = text.encode_utf16().collect();
    unsafe {
        let Ok(layout) = dwrite.CreateTextLayout(&wide, format, max_w.max(1.0), 4000.0) else {
            return (max_w, theme::typography::ROW_TITLE);
        };
        let mut metrics = DWRITE_TEXT_METRICS::default();
        if layout.GetMetrics(&mut metrics).is_err() {
            return (max_w, theme::typography::ROW_TITLE);
        }
        (metrics.width, metrics.height.max(theme::typography::ROW_TITLE))
    }
}

fn draw_wrapped(
    target: &ID2D1RenderTarget,
    format: &IDWriteTextFormat,
    text: &str,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    rgba: (f32, f32, f32, f32),
) -> windows::core::Result<()> {
    let brush = unsafe { target.CreateSolidColorBrush(&appearance::color(rgba), None)? };
    let wide: Vec<u16> = text.encode_utf16().collect();
    let rect = D2D_RECT_F {
        left: x,
        top: y,
        right: x + w,
        bottom: y + h,
    };
    unsafe {
        target.DrawText(
            &wide,
            format,
            &rect,
            &brush,
            D2D1_DRAW_TEXT_OPTIONS_CLIP,
            DWRITE_MEASURING_MODE_NATURAL,
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tinycast_pure::ai::ChatMessage;

    #[test]
    fn user_bubble_layout_leaves_leading_spacer() {
        let r = chat_layout::chat_user_bubble(750.0, 80.0, 200.0);
        assert!(r.x > 20.0);
    }

    #[test]
    fn chat_paint_holds_messages() {
        let msgs = [ChatMessage::user("hi", 1)];
        let chat = ChatPaint {
            messages: &msgs,
            notice: None,
            scroll: 0.0,
        };
        assert_eq!(chat.messages.len(), 1);
    }
}
