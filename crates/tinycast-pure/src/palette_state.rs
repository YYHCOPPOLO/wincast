use crate::palette_mode::PaletteMode;

#[derive(Clone, Debug)]
pub struct PaletteState {
    pub mode: PaletteMode,
    pub query: String,
    pub selection: usize,
    pub focus_token: u128,
    pub is_composing: bool,
}

impl PaletteState {
    pub fn new() -> Self {
        Self {
            mode: PaletteMode::Launcher,
            query: String::new(),
            selection: 0,
            focus_token: 0,
            is_composing: false,
        }
    }

    /// Reset query/selection, set mode, bump focus_token.
    pub fn prepare(&mut self, mode: PaletteMode) {
        self.mode = mode;
        self.query.clear();
        self.selection = 0;
        self.focus_token = self.focus_token.wrapping_add(1);
        self.is_composing = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prepare_clears_query_and_bumps_focus() {
        let mut s = PaletteState::new();
        let t0 = s.focus_token;
        s.query = "abc".into();
        s.selection = 5;
        s.prepare(PaletteMode::Clipboard);
        assert!(s.query.is_empty());
        assert_eq!(s.selection, 0);
        assert_eq!(s.mode, PaletteMode::Clipboard);
        assert_ne!(s.focus_token, t0);
    }
}
