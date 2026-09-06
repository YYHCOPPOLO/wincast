use std::sync::Mutex;

use tinycast_pure::palette_menu::MenuItem;
use tinycast_pure::palette_menu::{
    action_group_rects_measured, menu_button_rect, menu_frame, menu_header_rect, menu_line_rects,
    menu_row_rect, ActionGroupRects, OpenMenu, ACTIONS_SHORTCUT,
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

use crate::design_system::Fonts;
use crate::features::launcher::ui::list::ListFonts;

static LAST_ACTION_GROUP: Mutex<Option<ActionGroupRects>> = Mutex::new(None);

pub fn last_action_group_rects() -> Option<ActionGroupRects> {
    LAST_ACTION_GROUP.lock().ok().and_then(|g| *g)
}

fn store_action_group(group: Option<ActionGroupRects>) {
    if let Ok(mut slot) = LAST_ACTION_GROUP.lock() {
        *slot = group;
    }
}

pub struct FooterPaint<'a> {
    pub show_action_group: bool,
    pub primary_label: &'a str,
    pub primary_destructive: bool,
}

pub struct MenuPaint<'a> {
    pub header: &'a str,
    pub items: &'a [MenuItem],
    pub selection: usize,
    pub kind: OpenMenu,
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
        store_action_group(None);
        return Ok(());
    }
    let appearance = 0u8;
    let surface = crate::design_system::appearance::color(theme::colors::ramp_rgba(
        appearance,
        theme::colors::CONTROL_SURFACE_DARK_ALPHA,
        theme::colors::CONTROL_SURFACE_LIGHT_ALPHA,
    ));
    let brush = unsafe { target.CreateSolidColorBrush(&surface, None)? };

    let circle = menu_button_rect(height);
    let menu_r = circle.w / 2.0;
    let ellipse = D2D1_ELLIPSE {
        point: D2D_POINT_2F {
            x: circle.x + menu_r,
            y: circle.y + circle.h / 2.0,
        },
        radiusX: menu_r,
        radiusY: menu_r,
    };
    unsafe {
        target.FillEllipse(&ellipse, &brush);
    }
    let ink = crate::design_system::appearance::color(theme::colors::ramp_rgba(
        appearance,
        theme::colors::TEXT_SECONDARY_ALPHA,
        theme::colors::TEXT_SECONDARY_ALPHA,
    ));
    let (top, bot) = menu_line_rects(circle);
    fill_round(target, top, top.h / 2.0, ink)?;
    fill_round(target, bot, bot.h / 2.0, ink)?;

    if !footer.show_action_group {
        store_action_group(None);
        return Ok(());
    }
    let ds = Fonts::new(dwrite)?;
    let action_caps: Vec<&str> = ACTIONS_SHORTCUT.split('+').collect();
    let primary_w = bar_button_width(&ds, footer.primary_label, &["↵"]);
    let actions_w = bar_button_width(&ds, "Actions", &action_caps);
    let Some(group) = action_group_rects_measured(width, height, primary_w, actions_w) else {
        store_action_group(None);
        return Ok(());
    };
    store_action_group(Some(group));
    crate::design_system::fill_squircle(
        target,
        group.capsule,
        group.capsule.h / 2.0,
        theme::colors::ramp_rgba(
            appearance,
            theme::colors::CONTROL_SURFACE_DARK_ALPHA,
            theme::colors::CONTROL_SURFACE_LIGHT_ALPHA,
        ),
    )?;
    if footer.primary_destructive {
        let danger = color(0.86, 0.22, 0.22, 0.95);
        fill_round(target, group.primary, group.primary.h / 2.0, danger)?;
    }

    let label = color(1.0, 1.0, 1.0, theme::colors::TEXT_PRIMARY_ALPHA);
    let muted = color(1.0, 1.0, 1.0, theme::colors::TEXT_SECONDARY_ALPHA);
    let pad = theme::spacing::MD;
    let primary = group.primary;
    let mut right = primary.x + primary.w - pad;
    right -= crate::design_system::paint_keycap(
        target,
        &ds,
        "↵",
        right,
        primary.y,
        primary.h,
        true,
        appearance,
    )?;
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
    for token in action_caps.iter().rev() {
        right -= crate::design_system::paint_keycap(
            target,
            &ds,
            token,
            right,
            actions.y,
            actions.h,
            true,
            appearance,
        )?;
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

fn bar_button_width(fonts: &Fonts, label: &str, caps: &[&str]) -> f32 {
    let pad = theme::spacing::MD;
    let mut w = pad + text_width(&fonts.dwrite, &fonts.bar, label, 240.0, theme::size::BAR_BUTTON_HEIGHT);
    if !caps.is_empty() {
        w += theme::spacing::SM;
    }
    for (i, cap) in caps.iter().enumerate() {
        if i > 0 {
            w += theme::spacing::XXS;
        }
        w += keycap_width(fonts, cap);
    }
    w + pad
}

fn keycap_width(fonts: &Fonts, text: &str) -> f32 {
    let pad = theme::spacing::XS;
    let text_w = text_width(&fonts.dwrite, &fonts.keycap, text, 80.0, theme::size::KEY_CAP);
    (text_w + pad * 2.0).max(theme::size::KEY_CAP)
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
    let frame = menu_frame(menu.kind, panel_w, panel_h, menu.items.len(), has_header);
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
            &item.label,
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

#[cfg(test)]
mod tests {
    use super::*;
    use windows::core::w;
    use windows::Win32::Graphics::DirectWrite::{
        DWriteCreateFactory, DWRITE_FACTORY_TYPE_SHARED, DWRITE_FONT_STRETCH_NORMAL,
        DWRITE_FONT_STYLE_NORMAL, DWRITE_FONT_WEIGHT_REGULAR, DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
        DWRITE_TEXT_ALIGNMENT_LEADING, DWRITE_WORD_WRAPPING_NO_WRAP,
    };

    fn render_footer_scene() -> (usize, Vec<u8>) {
        let (w, _h, bits) = crate::design_system::test_render::with_offscreen(750, 475, |target| {
            unsafe {
                target.Clear(Some(&D2D1_COLOR_F {
                    r: 0.0,
                    g: 0.0,
                    b: 0.0,
                    a: 0.0,
                }));
            }
            let dwrite = unsafe { DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)? };
            let fonts = test_list_fonts(&dwrite)?;
            paint_footer(
                target,
                &dwrite,
                &fonts,
                750.0,
                475.0,
                &FooterPaint {
                    show_action_group: true,
                    primary_label: "Open Application",
                    primary_destructive: false,
                },
            )
        })
        .expect("footer scene");
        (w, bits)
    }

    fn test_list_fonts(dwrite: &IDWriteFactory) -> windows::core::Result<ListFonts> {
        let format = unsafe {
            dwrite.CreateTextFormat(
                w!("Segoe UI"),
                None,
                DWRITE_FONT_WEIGHT_REGULAR,
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                11.0,
                w!("en-US"),
            )?
        };
        unsafe {
            format.SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP)?;
            format.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER)?;
            format.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_LEADING)?;
        }
        Ok(ListFonts {
            title: format.clone(),
            trailing: format.clone(),
            header: format.clone(),
            chip: format.clone(),
            keycap: format.clone(),
            calc_result: format.clone(),
            calc_badge: format.clone(),
            emoji: format,
        })
    }

    #[test]
    fn footer_has_no_full_width_hairline() {
        let (w, bits) = render_footer_scene();
        let y = 475 - 52;
        let mut longest = 0usize;
        let mut run = 0usize;
        for x in 0..w {
            let a = bits[(y * w + x) * 4 + 3];
            if a > 8 {
                run += 1;
                if run > longest {
                    longest = run;
                }
            } else {
                run = 0;
            }
        }
        assert!(
            longest < 200,
            "hairline would make most of y={y} opaque, longest={longest} w={w}"
        );
    }
}
