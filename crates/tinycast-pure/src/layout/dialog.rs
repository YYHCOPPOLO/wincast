use crate::palette_placement::DipRect;
use crate::theme;

pub fn frame() -> (f32, f32) {
    (theme::size::DIALOG_WIDTH, 120.0)
}

pub fn icon_rect() -> DipRect {
    DipRect {
        x: theme::spacing::XXL,
        y: theme::spacing::XXL,
        w: theme::size::DIALOG_ICON,
        h: theme::size::DIALOG_ICON,
    }
}

pub fn cancel_is_leading() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use crate::layout::{dialog, hud, notes};

    #[test]
    fn dialog_and_hud_tokens() {
        assert_eq!(dialog::frame().0, 420.0);
        assert_eq!(dialog::icon_rect().w, 32.0);
        assert!(dialog::cancel_is_leading());
        assert_eq!(hud::volume_size(), (200.0, 100.0));
        assert_eq!(hud::message_max_width(), 420.0);
        assert_eq!(notes::titlebar_height(), 52.0);
    }
}
