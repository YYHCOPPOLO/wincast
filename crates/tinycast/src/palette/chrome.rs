//! Palette menu open and close, played by `Theme.MenuMotion` the way upstream
//! `Palette/MenuPanel.swift` does: scale 0.94 → 1.003 → 1 while alpha rises on the
//! expansion curve, then on dismissal scale −0.04 and alpha → 0 on the exit curve,
//! anchored at the corner the menu hangs from.

use std::time::Instant;

use tinycast_pure::motion::{MenuReveal, Visual};
use tinycast_pure::palette_menu::{MenuItem, OpenMenu};

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

#[derive(Default)]
pub struct MenuMotion {
    reveal: MenuReveal,
    visual: Option<Visual>,
    snapshot: Option<OwnedMenu>,
}

impl MenuMotion {
    pub fn sync(&mut self, live: Option<OwnedMenu>) -> bool {
        self.sync_at(live, Instant::now())
    }

    pub fn sync_at(&mut self, live: Option<OwnedMenu>, now: Instant) -> bool {
        if let Some(menu) = live {
            let switched = self
                .snapshot
                .as_ref()
                .is_some_and(|have| have.kind != menu.kind);
            self.snapshot = Some(menu);
            if switched {
                self.reveal.restart(now);
            } else {
                self.reveal.open(now);
            }
        } else {
            self.reveal.close(now);
        }
        let visual = self.reveal.sample(now);
        if self.reveal.is_hidden() {
            self.snapshot = None;
            self.visual = None;
        } else {
            self.visual = Some(visual);
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
        let visual = self.visual?;
        if visual.alpha <= 0.001 {
            return None;
        }
        Some((menu, visual.alpha, visual.scale))
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn menu(kind: OpenMenu) -> OwnedMenu {
        OwnedMenu {
            header: String::new(),
            items: Vec::new(),
            selection: 0,
            kind,
            appearance: 0,
        }
    }

    #[test]
    fn menu_opens_overshoots_and_closes_to_nothing() {
        let t0 = Instant::now();
        let mut motion = MenuMotion::default();
        assert!(motion.sync_at(Some(menu(OpenMenu::Actions)), t0));
        assert!(motion.needs_frame());
        motion.sync_at(
            Some(menu(OpenMenu::Actions)),
            t0 + Duration::from_millis(100),
        );
        let (_, _, scale) = motion.visual().expect("visible");
        assert!(scale > 1.0, "overshoot at end of expansion: {scale}");
        motion.sync_at(
            Some(menu(OpenMenu::Actions)),
            t0 + Duration::from_millis(400),
        );
        let (_, alpha, scale) = motion.visual().expect("visible");
        assert!((alpha - 1.0).abs() < 1e-3 && (scale - 1.0).abs() < 1e-3);
        assert!(!motion.needs_frame());

        let t1 = t0 + Duration::from_millis(500);
        assert!(motion.sync_at(None, t1));
        assert!(motion.needs_frame());
        assert!(!motion.sync_at(None, t1 + Duration::from_millis(400)));
        assert!(motion.visual().is_none());
    }
}
