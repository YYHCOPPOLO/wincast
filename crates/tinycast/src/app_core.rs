use tinycast_pure::palette_mode::PaletteMode;
use tinycast_pure::palette_state::PaletteState;

pub struct AppCore {
    pub palette: tinycast_pure::palette_state::PaletteState,
    pub palette_visible: bool,
}

impl AppCore {
    pub fn new() -> Self {
        Self {
            palette: PaletteState::new(),
            palette_visible: false,
        }
    }

    pub fn start(&mut self) {
        self.palette = PaletteState::new();
        self.palette_visible = false;
    }

    pub fn toggle_palette(&mut self) {
        if self.palette_visible {
            self.hide_palette();
        } else {
            self.palette.prepare(PaletteMode::Launcher);
            self.palette_visible = true;
        }
    }

    pub fn hide_palette(&mut self) {
        self.palette_visible = false;
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
}
