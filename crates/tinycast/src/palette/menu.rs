use tinycast_pure::palette_menu::MenuItem;
use tinycast_pure::palette_menu::{
    action_group_rects, actions_menu_frame, menu_header_rect, menu_row_rect, ACTIONS_SHORTCUT,
};
use tinycast_pure::palette_placement::DipRect;
use tinycast_pure::theme;
use windows::Win32::Graphics::Direct2D::Common::{D2D1_COLOR_F, D2D_POINT_2F, D2D_RECT_F};
use windows::Win32::Graphics::Direct2D::{
    ID2D1RenderTarget, D2D1_DRAW_TEXT_OPTIONS_CLIP, D2D1_ELLIPSE, D2D1_ROUNDED_RECT,
};
use windows::Win32::Graphics::DirectWrite::{
    IDWriteFactory, IDWriteTextFormat, DWRITE_MEASURING_MODE_NATURAL, DWRITE_TEXT_METRICS,
};

use crate::features::launcher::ui::list::ListFonts;

pub struct FooterPaint<'a> {
    pub show_action_group: bool,
    pub primary_label: &'a str,
}

pub struct MenuPaint<'a> {
    pub header: &'a str,
    pub items: &'a [MenuItem],
    pub selection: usize,
}

pub fn paint_footer(
    target: &ID2D1RenderTarget,
    dwrite: &IDWriteFactory,
    fonts: &ListFonts,
    width: f32,
    height: f32,
    footer: &FooterPaint<'_>,
) -> windows::core::Result<()> {
    if height < theme::size::COMPACT_HEIGHT + theme::size::BOTTOM_BAR_HEIGHT {
        return Ok(());
    }
    let bar_h = theme::size::BOTTOM_BAR_HEIGHT;
    let bar_y = height - bar_h;
    let chrome = color(1.0, 1.0, 1.0, theme::colors::SELECTION_DARK_ALPHA);
    let brush = unsafe { target.CreateSolidColorBrush(&chrome, None)? };
    unsafe {
        target.DrawLine(
            D2D_POINT_2F { x: 0.0, y: bar_y },
            D2D_POINT_2F { x: width, y: bar_y },
            &brush,
            theme::size::HAIRLINE,
            None,
        );
    }

    let inset = theme::spacing::XXL;
    let menu_d = theme::size::MENU_BUTTON;
    let menu_r = menu_d / 2.0;
    let ellipse = D2D1_ELLIPSE {
        point: D2D_POINT_2F {
            x: inset + menu_r,
            y: bar_y + bar_h / 2.0,
        },
        radiusX: menu_r,
        radiusY: menu_r,
    };
    unsafe {
        target.FillEllipse(&ellipse, &brush);
    }

    if !footer.show_action_group {
        return Ok(());
    }
    let Some(group) = action_group_rects(width, height) else {
        return Ok(());
    };
    fill_round(target, group.capsule, group.capsule.h / 2.0, chrome)?;

    let label = color(1.0, 1.0, 1.0, 0.92);
    let muted = color(1.0, 1.0, 1.0, 0.60);
    let pad = theme::spacing::MD;
    let primary = group.primary;
    let mut right = primary.x + primary.w - pad;
    right -= paint_keycap(target, dwrite, fonts, "↵", right, primary.y, primary.h)?;
    draw_text(
        target,
        &fonts.header,
        label,
        D2D_RECT_F {
            left: primary.x + pad,
            top: primary.y,
            right: (right - theme::spacing::SM).max(primary.x + pad + 8.0),
            bottom: primary.y + primary.h,
        },
        footer.primary_label,
    )?;

    let actions = group.actions;
    right = actions.x + actions.w - pad;
    for token in ACTIONS_SHORTCUT.split('+').rev() {
        right -= paint_keycap(target, dwrite, fonts, token, right, actions.y, actions.h)?;
        right -= theme::spacing::XXS;
    }
    draw_text(
        target,
        &fonts.header,
        muted,
        D2D_RECT_F {
            left: actions.x + pad,
            top: actions.y,
            right: (right - theme::spacing::XS).max(actions.x + pad + 8.0),
            bottom: actions.y + actions.h,
        },
        "Actions",
    )?;
    Ok(())
}

pub fn paint_menu(
    target: &ID2D1RenderTarget,
    dwrite: &IDWriteFactory,
    fonts: &ListFonts,
    panel_w: f32,
    panel_h: f32,
    menu: &MenuPaint<'_>,
) -> windows::core::Result<()> {
    if menu.items.is_empty() {
        return Ok(());
    }
    let has_header = !menu.header.is_empty();
    let frame = actions_menu_frame(panel_w, panel_h, menu.items.len(), has_header);
    let glass = color(0.08, 0.08, 0.08, 0.94);
    fill_round(target, frame, theme::radius::MENU_PANEL, glass)?;
    let frost = color(1.0, 1.0, 1.0, 0.05);
    fill_round(target, frame, theme::radius::MENU_PANEL, frost)?;
    let border = unsafe { target.CreateSolidColorBrush(&color(1.0, 1.0, 1.0, 0.20), None)? };
    unsafe {
        target.DrawRoundedRectangle(
            &D2D1_ROUNDED_RECT {
                rect: rect_of(frame),
                radiusX: theme::radius::MENU_PANEL,
                radiusY: theme::radius::MENU_PANEL,
            },
            &border,
            theme::size::HAIRLINE,
            None,
        );
    }

    if has_header {
        let header = menu_header_rect(frame);
        draw_text(
            target,
            &fonts.header,
            color(1.0, 1.0, 1.0, 0.45),
            rect_of(header),
            menu.header,
        )?;
    }

    for (index, item) in menu.items.iter().enumerate() {
        let row = menu_row_rect(frame, has_header, index);
        if index == menu.selection {
            fill_round(
                target,
                row,
                theme::radius::MENU_ROW,
                color(1.0, 1.0, 1.0, theme::colors::SELECTION_DARK_ALPHA),
            )?;
        }
        let ink = if item.is_destructive() {
            color(1.0, 0.32, 0.32, 0.95)
        } else {
            color(1.0, 1.0, 1.0, 0.92)
        };
        let pad = theme::spacing::MD;
        let mut right = row.x + row.w - pad;
        if let Some(shortcut) = item.shortcut {
            for token in shortcut.split('+').rev() {
                right -= paint_keycap(target, dwrite, fonts, token, right, row.y, row.h)?;
                right -= theme::spacing::XXS;
            }
        }
        let icon = theme::size::MENU_ICON;
        let icon_rect = D2D_RECT_F {
            left: row.x + pad,
            top: row.y + (row.h - icon) / 2.0,
            right: row.x + pad + icon,
            bottom: row.y + (row.h + icon) / 2.0,
        };
        fill_round(
            target,
            DipRect {
                x: icon_rect.left,
                y: icon_rect.top,
                w: icon,
                h: icon,
            },
            theme::radius::MENU,
            color(1.0, 1.0, 1.0, 0.08),
        )?;
        let text_left = icon_rect.right + theme::spacing::SM;
        draw_text(
            target,
            &fonts.title,
            ink,
            D2D_RECT_F {
                left: text_left,
                top: row.y,
                right: (right - theme::spacing::SM).max(text_left + 8.0),
                bottom: row.y + row.h,
            },
            item.label,
        )?;
    }
    Ok(())
}

fn paint_keycap(
    target: &ID2D1RenderTarget,
    dwrite: &IDWriteFactory,
    fonts: &ListFonts,
    text: &str,
    right: f32,
    row_y: f32,
    row_h: f32,
) -> windows::core::Result<f32> {
    let cap_h = theme::size::KEY_CAP;
    let text_w =
        text_width(dwrite, &fonts.keycap, text, 80.0, cap_h).max(theme::size::COMPACT_KEY_CAP);
    let cap_w = text_w + theme::spacing::SM * 2.0;
    let cap_x = right - cap_w;
    let cap_y = row_y + (row_h - cap_h) / 2.0;
    fill_round(
        target,
        DipRect {
            x: cap_x,
            y: cap_y,
            w: cap_w,
            h: cap_h,
        },
        theme::radius::KEY_CAP,
        color(1.0, 1.0, 1.0, 0.12),
    )?;
    draw_text(
        target,
        &fonts.keycap,
        color(1.0, 1.0, 1.0, 0.70),
        D2D_RECT_F {
            left: cap_x,
            top: cap_y,
            right: cap_x + cap_w,
            bottom: cap_y + cap_h,
        },
        text,
    )?;
    Ok(cap_w)
}

fn fill_round(
    target: &ID2D1RenderTarget,
    r: DipRect,
    radius: f32,
    c: D2D1_COLOR_F,
) -> windows::core::Result<()> {
    let brush = unsafe { target.CreateSolidColorBrush(&c, None)? };
    unsafe {
        target.FillRoundedRectangle(
            &D2D1_ROUNDED_RECT {
                rect: rect_of(r),
                radiusX: radius,
                radiusY: radius,
            },
            &brush,
        );
    }
    Ok(())
}

fn draw_text(
    target: &ID2D1RenderTarget,
    format: &IDWriteTextFormat,
    c: D2D1_COLOR_F,
    rect: D2D_RECT_F,
    text: &str,
) -> windows::core::Result<()> {
    let brush = unsafe { target.CreateSolidColorBrush(&c, None)? };
    let wide: Vec<u16> = text.encode_utf16().collect();
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

fn text_width(
    dwrite: &IDWriteFactory,
    format: &IDWriteTextFormat,
    text: &str,
    max_w: f32,
    h: f32,
) -> f32 {
    let wide: Vec<u16> = text.encode_utf16().collect();
    unsafe {
        let Ok(layout) = dwrite.CreateTextLayout(&wide, format, max_w, h) else {
            return 0.0;
        };
        let mut metrics = DWRITE_TEXT_METRICS::default();
        if layout.GetMetrics(&mut metrics).is_err() {
            return 0.0;
        }
        metrics.width
    }
}

fn rect_of(r: DipRect) -> D2D_RECT_F {
    D2D_RECT_F {
        left: r.x,
        top: r.y,
        right: r.x + r.w,
        bottom: r.y + r.h,
    }
}

fn color(r: f32, g: f32, b: f32, a: f32) -> D2D1_COLOR_F {
    D2D1_COLOR_F { r, g, b, a }
}
