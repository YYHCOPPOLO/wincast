use tinycast_pure::palette_mode::PaletteMode;
use tinycast_pure::palette_placement::{default_anchor, frame_for};
use tinycast_pure::palette_state::PaletteState;

use crate::palette::physical;
use crate::palette::PaletteWindow;
use crate::platform::screens::{cursor_target_screen, dip_to_px, dip_to_px_with_dpi};

pub struct AppCore {
    pub palette: PaletteState,
    pub palette_visible: bool,
    pub palette_window: Option<PaletteWindow>,
    expanded: bool,
    anchor: Option<tinycast_pure::palette_placement::PaletteAnchor>,
    anchor_px: Option<physical::Point>,
}

impl AppCore {
    pub fn new() -> Self {
        Self {
            palette: PaletteState::new(),
            palette_visible: false,
            palette_window: None,
            expanded: false,
            anchor: None,
            anchor_px: None,
        }
    }

    pub fn start(&mut self) {
        self.palette = PaletteState::new();
        self.palette_visible = false;
        self.expanded = false;
        self.anchor = None;
        self.anchor_px = None;
        if let Some(window) = &self.palette_window {
            window.hide();
        }
    }

    pub fn toggle_palette(&mut self) {
        if self.palette_visible {
            self.hide_palette();
        } else {
            self.palette.prepare(PaletteMode::Launcher);
            self.palette_visible = true;
            self.expanded = false;
            self.show_palette_window();
        }
    }

    pub fn hide_palette(&mut self) {
        self.palette_visible = false;
        self.expanded = false;
        self.anchor = None;
        self.anchor_px = None;
        if let Some(window) = &self.palette_window {
            window.hide();
        }
    }

    pub fn expand_palette(&mut self) {
        if !self.palette_visible {
            return;
        }
        self.expanded = true;
        let Some(anchor_px) = self.anchor_px else {
            return;
        };
        if let Some(window) = &self.palette_window {
            window.set_expanded(true, anchor_px);
        }
    }

    pub fn append_query_char(&mut self, c: char) {
        if c.is_control() {
            return;
        }
        self.palette.query.push(c);
        if !self.palette.query.is_empty() {
            self.expand_palette();
        }
    }

    pub fn relayout_palette(&mut self) {
        if !self.palette_visible {
            return;
        }
        let Some(anchor) = self.anchor else {
            return;
        };
        let Some(hwnd) = self.palette_window.as_ref().map(|w| w.hwnd) else {
            return;
        };
        let frame = frame_for(anchor, self.expanded);
        let rect = physical::Rect::from_win32(dip_to_px(hwnd, frame));
        self.anchor_px = Some(physical::Point {
            x: rect.x,
            y: rect.y,
        });
        if let Some(window) = &self.palette_window {
            window.set_expanded(self.expanded, self.anchor_px.unwrap());
        }
    }

    fn show_palette_window(&mut self) {
        if self.palette_window.is_none() {
            return;
        }
        let Some((screen, dpi)) = cursor_target_screen() else {
            return;
        };
        let anchor = default_anchor(screen);
        self.anchor = Some(anchor);
        let frame = frame_for(anchor, false);
        let rect = physical::Rect::from_win32(dip_to_px_with_dpi(frame, dpi));
        self.anchor_px = Some(physical::Point {
            x: rect.x,
            y: rect.y,
        });
        if let Some(window) = &self.palette_window {
            window.show_at(rect);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toggle_palette_flips_visible_and_prepares_launcher() {
        let mut c = AppCore::new();
        assert!(!c.palette_visible);
        c.toggle_palette();
        assert!(c.palette_visible);
        assert_eq!(
            c.palette.mode,
            tinycast_pure::palette_mode::PaletteMode::Launcher
        );
        c.toggle_palette();
        assert!(!c.palette_visible);
    }

    #[test]
    fn empty_query_stays_compact_until_query_or_down() {
        let mut c = AppCore::new();
        c.toggle_palette();
        assert!(c.palette.query.is_empty());
        assert!(!c.expanded);
        c.expand_palette();
        assert!(c.expanded);
        c.hide_palette();
        c.toggle_palette();
        assert!(!c.expanded);
        c.append_query_char('a');
        assert_eq!(c.palette.query, "a");
        assert!(c.expanded);
    }
}
