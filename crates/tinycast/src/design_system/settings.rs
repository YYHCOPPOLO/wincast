use tinycast_pure::palette_placement::DipRect;
use tinycast_pure::theme;

pub const ROW_H: f32 = theme::size::SETTINGS_ROW_HEIGHT;
pub const TOGGLE_W: f32 = 40.0;
pub const TOGGLE_H: f32 = 22.0;
pub const HEADER_H: f32 = 22.0;
pub const CARD_INSET: f32 = theme::spacing::XXL;
pub const CARD_PAD: f32 = theme::spacing::XL;

pub fn footer_block_h(lines: u32) -> f32 {
    (18.0 * lines as f32).max(18.0)
}

/// Symmetric content inset: card origin plus inner pad.
pub fn content_pad() -> f32 {
    CARD_INSET + CARD_PAD
}

pub fn toggle_rect(width: f32, row_y: f32, pad_x: f32) -> DipRect {
    DipRect {
        x: width - pad_x - TOGGLE_W,
        y: row_y + (ROW_H - TOGGLE_H) / 2.0,
        w: TOGGLE_W,
        h: TOGGLE_H,
    }
}

pub fn approx_label_width(text: &str) -> f32 {
    let mut w = 0.0f32;
    for c in text.chars() {
        w += if (c as u32) > 0x2E80 { 13.0 } else { 7.5 };
    }
    (w + theme::spacing::XS).max(theme::spacing::XL)
}

pub struct GroupedSection {
    pub header: Option<&'static str>,
    pub footer: Option<&'static str>,
    pub footer_h: f32,
    pub y: f32,
    pub width: f32,
    pub body_h: f32,
}

impl GroupedSection {
    pub fn card_rect(&self) -> DipRect {
        let header_h = if self.header.is_some() {
            HEADER_H + theme::spacing::SECTION_HEADER_BOTTOM
        } else {
            0.0
        };
        DipRect {
            x: CARD_INSET,
            y: self.y + header_h,
            w: (self.width - CARD_INSET * 2.0).max(0.0),
            h: self.body_h,
        }
    }

    pub fn next_y(&self) -> f32 {
        let card = self.card_rect();
        let footer_h = if self.footer.is_some() {
            theme::spacing::SM + self.footer_h.max(footer_block_h(1))
        } else {
            0.0
        };
        card.y + card.h + footer_h + theme::spacing::SECTION_SPACING
    }
}

#[derive(Clone, Copy, Debug)]
pub enum RowTrailing<'a> {
    None,
    Toggle(bool),
    Label(&'a str),
}

pub fn trailing_width(trailing: RowTrailing<'_>, width: f32, pad_x: f32) -> f32 {
    let max_trail = ((width - pad_x * 2.0) * 0.5).max(48.0);
    match trailing {
        RowTrailing::Toggle(_) => TOGGLE_W + theme::spacing::SM,
        RowTrailing::Label(label) => approx_label_width(label).clamp(40.0, max_trail),
        RowTrailing::None => 0.0,
    }
}

pub fn form_origin() -> f32 {
    CARD_INSET + HEADER_H + theme::spacing::SECTION_HEADER_BOTTOM + CARD_PAD
}

/// Content-space y after a FeatureSwitchSection card (scroll = 0).
pub fn switch_section_next_y(launcher_row: bool) -> f32 {
    let rows = if launcher_row { 2 } else { 1 };
    GroupedSection {
        header: Some(""),
        footer: None,
        footer_h: 0.0,
        y: CARD_INSET,
        width: 420.0,
        body_h: CARD_PAD * 2.0 + ROW_H * rows as f32,
    }
    .next_y()
}

pub fn feature_switch_section(
    width: f32,
    y: f32,
    header: &'static str,
    launcher_row: bool,
) -> (GroupedSection, DipRect, Option<DipRect>) {
    let rows = if launcher_row { 2 } else { 1 };
    let section = GroupedSection {
        header: Some(header),
        footer: None,
        footer_h: 0.0,
        y,
        width,
        body_h: CARD_PAD * 2.0 + ROW_H * rows as f32,
    };
    let card = section.card_rect();
    let enable = DipRect {
        x: card.x,
        y: card.y + CARD_PAD,
        w: card.w,
        h: ROW_H,
    };
    let show = if launcher_row {
        Some(DipRect {
            x: card.x,
            y: enable.y + ROW_H,
            w: card.w,
            h: ROW_H,
        })
    } else {
        None
    };
    (section, enable, show)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn feature_switch_rows_are_stacked_row_h() {
        let (_, enable, show) = feature_switch_section(420.0, CARD_INSET, "Notes", true);
        let show = show.expect("launcher row");
        assert!((show.y - enable.y - ROW_H).abs() < 0.001);
        assert!((show.y - enable.y - (ROW_H + theme::spacing::XL)).abs() > 1.0);
        assert!((enable.y - form_origin()).abs() < 0.001);
    }

    #[test]
    fn footer_height_grows_with_wrapped_copy() {
        assert!(footer_block_h(3) > 36.0);
    }

    #[test]
    fn toggle_stays_inside_card_content_inset() {
        for width in [theme::size::SETTINGS_DETAIL_MINIMUM, 645.0] {
            let (section, enable, show) = feature_switch_section(width, CARD_INSET, "Notes", true);
            let card = section.card_rect();
            let pad = content_pad();
            let on = toggle_rect(width, enable.y, pad);
            assert!(
                on.x + 0.01 >= card.x + CARD_PAD,
                "toggle x {} inside card {}+{}",
                on.x,
                card.x,
                CARD_PAD
            );
            assert!(
                on.x + on.w <= card.x + card.w - CARD_PAD + 0.01,
                "toggle right {} vs card inner {}",
                on.x + on.w,
                card.x + card.w - CARD_PAD
            );
            let show = show.expect("launcher row");
            let off = toggle_rect(width, show.y, pad);
            assert!(off.x + off.w <= card.x + card.w - CARD_PAD + 0.01);
            let leaked = width - theme::spacing::XL - TOGGLE_W;
            assert!(
                leaked + TOGGLE_W > card.x + card.w - CARD_PAD,
                "legacy XL pad must overflow the card so the new inset is a real fix"
            );
        }
    }

    #[test]
    fn long_trailing_label_does_not_eat_the_title_column() {
        let width = theme::size::SETTINGS_DETAIL_MINIMUM;
        let pad = content_pad();
        let label = "空闲 0 秒后返回（0 = 从不）。";
        let trail = trailing_width(RowTrailing::Label(label), width, pad);
        let text_right = width - pad - trail;
        assert!(text_right - pad >= 80.0, "title column {text_right}");
        assert!(trail <= (width - pad * 2.0) * 0.5 + 0.01);
        assert!(approx_label_width("Never") < approx_label_width(label));
    }
}
