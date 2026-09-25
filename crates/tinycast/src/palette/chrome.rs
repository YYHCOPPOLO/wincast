//! Palette menu open and close. `RootPaletteView` scales from 0.96 and fades
//! with `easeOut(duration: 0.14)`, anchored at the corner the menu hangs from.

use tinycast_pure::motion::{lerp, Ease, Reveal, MENU_SCALE};
use tinycast_pure::palette_menu::{MenuItem, OpenMenu};
use tinycast_pure::theme;

use super::menu::MenuPaint;

#[derive(Clone)]
pub struct OwnedMenu {
    pub header: String,
    pub items: Vec<MenuItem>,
    pub selection: usize,
    pub kind: OpenMenu,
    pub appearance: u8,
}

impl OwnedMenu {
    pub fn capture(menu: &MenuPaint<'_>) -> Self {
        Self {
            header: menu.header.to_string(),
            items: menu
                .items
                .iter()
                .map(|item| MenuItem {
                    id: item.id,
                    label: item.label.clone(),
                    shortcut: item.shortcut,
                })
                .collect(),
            selection: menu.selection,
            kind: menu.kind,
            appearance: menu.appearance,
        }
    }
}

pub struct MenuMotion {
    reveal: Reveal,
    snapshot: Option<OwnedMenu>,
}

impl Default for MenuMotion {
    fn default() -> Self {
        Self {
            reveal: Reveal::default(),
            snapshot: None,
        }
    }
}

impl MenuMotion {
    pub fn sync(&mut self, live: Option<OwnedMenu>) -> bool {
        if let Some(menu) = live {
            let switched = self
                .snapshot
                .as_ref()
                .is_some_and(|have| have.kind != menu.kind);
            self.snapshot = Some(menu);
            if switched {
                self.reveal = Reveal::at(0.0);
            }
            let _ = self.reveal.go(1.0, theme::duration::MENU_SECS, Ease::Out);
        } else {
            let _ = self.reveal.go(0.0, theme::duration::MENU_SECS, Ease::Out);
        }
        let value = self.reveal.sample_now();
        if value <= 0.001 && !self.reveal.is_running() {
            self.snapshot = None;
        }
        self.snapshot.is_some()
    }

    pub fn needs_frame(&self) -> bool {
        self.reveal.is_running()
    }

    pub fn presentation(&self) -> Option<(OwnedMenu, f32, f32)> {
        let (menu, opacity, scale) = self.visual()?;
        Some((menu.clone(), opacity, scale))
    }

    pub fn visual(&self) -> Option<(&OwnedMenu, f32, f32)> {
        let menu = self.snapshot.as_ref()?;
        let t = self.reveal.value;
        if t <= 0.001 {
            return None;
        }
        Some((menu, t, lerp(MENU_SCALE, 1.0, t)))
    }
}

/// The overlay corner `menuTransition` scales around.
pub fn menu_anchor(kind: OpenMenu, panel_w: f32, panel_h: f32) -> (f32, f32) {
    match kind {
        OpenMenu::ClipboardFilter | OpenMenu::AiModel => (panel_w, 0.0),
        OpenMenu::AppMenu => (0.0, panel_h),
        OpenMenu::Actions | OpenMenu::None => (panel_w, panel_h),
    }
}
