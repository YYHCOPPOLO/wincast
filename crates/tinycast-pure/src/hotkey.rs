#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, serde::Serialize, serde::Deserialize)]
pub struct Modifiers {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub win: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, serde::Serialize, serde::Deserialize)]
pub struct KeyShortcut {
    pub vk: u16,
    pub modifiers: Modifiers,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, serde::Serialize, serde::Deserialize)]
pub enum DoubleTapModifier {
    Control,
    Option,
    Shift,
    Command,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum HotKeyBinding {
    Combo(KeyShortcut),
    DoubleTap(DoubleTapModifier),
}

pub fn default_toggle_palette() -> HotKeyBinding {
    HotKeyBinding::Combo(KeyShortcut {
        vk: 0x20, // VK_SPACE
        modifiers: Modifiers {
            ctrl: false,
            alt: true,
            shift: false,
            win: false,
        },
    })
}

impl Modifiers {
    pub fn none() -> Self {
        Self {
            ctrl: false,
            alt: false,
            shift: false,
            win: false,
        }
    }

    pub fn is_bare(self) -> bool {
        !self.ctrl && !self.alt && !self.shift && !self.win
    }

    /// ⌘⌥⌃ equivalents: Win / Alt / Ctrl. Shift alone is not enough.
    pub fn has_commanding(self) -> bool {
        self.ctrl || self.alt || self.win
    }
}

/// VK_SHIFT/CONTROL/MENU and left/right / Win variants.
pub fn is_modifier_vk(vk: u16) -> bool {
    matches!(vk, 0x10 | 0x11 | 0x12 | 0x5B | 0x5C | 0xA0..=0xA5)
}

pub fn is_function_vk(vk: u16) -> bool {
    (0x70..=0x87).contains(&vk)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CaptureOutcome {
    Ignore,
    Cancel,
    Clear,
    Commit(HotKeyBinding),
}

/// Local WM_KEYDOWN while global hotkeys are paused. Combo-only stub (no double-tap).
pub fn capture_keydown(vk: u16, modifiers: Modifiers) -> CaptureOutcome {
    if is_modifier_vk(vk) {
        return CaptureOutcome::Ignore;
    }
    if modifiers.is_bare() && vk == 0x1B {
        return CaptureOutcome::Cancel;
    }
    if modifiers.is_bare() && (vk == 0x08 || vk == 0x2E) {
        return CaptureOutcome::Clear;
    }
    if modifiers.has_commanding() || is_function_vk(vk) {
        return CaptureOutcome::Commit(HotKeyBinding::Combo(KeyShortcut { vk, modifiers }));
    }
    CaptureOutcome::Ignore
}

impl HotKeyBinding {
    pub fn label(&self) -> String {
        match self {
            HotKeyBinding::Combo(shortcut) => {
                let mut parts = Vec::new();
                if shortcut.modifiers.ctrl {
                    parts.push("Ctrl");
                }
                if shortcut.modifiers.alt {
                    parts.push("Alt");
                }
                if shortcut.modifiers.shift {
                    parts.push("Shift");
                }
                if shortcut.modifiers.win {
                    parts.push("Win");
                }
                parts.push(vk_label(shortcut.vk));
                parts.join("+")
            }
            HotKeyBinding::DoubleTap(m) => {
                let name = match m {
                    DoubleTapModifier::Control => "Ctrl",
                    DoubleTapModifier::Option => "Alt",
                    DoubleTapModifier::Shift => "Shift",
                    DoubleTapModifier::Command => "Win",
                };
                format!("Double {name}")
            }
        }
    }
}

fn vk_label(vk: u16) -> &'static str {
    match vk {
        0x08 => "Backspace",
        0x09 => "Tab",
        0x0D => "Enter",
        0x1B => "Esc",
        0x20 => "Space",
        0x25 => "Left",
        0x26 => "Up",
        0x27 => "Right",
        0x28 => "Down",
        0x2E => "Del",
        0x30 => "0",
        0x31 => "1",
        0x32 => "2",
        0x33 => "3",
        0x34 => "4",
        0x35 => "5",
        0x36 => "6",
        0x37 => "7",
        0x38 => "8",
        0x39 => "9",
        0x41 => "A",
        0x42 => "B",
        0x43 => "C",
        0x44 => "D",
        0x45 => "E",
        0x46 => "F",
        0x47 => "G",
        0x48 => "H",
        0x49 => "I",
        0x4A => "J",
        0x4B => "K",
        0x4C => "L",
        0x4D => "M",
        0x4E => "N",
        0x4F => "O",
        0x50 => "P",
        0x51 => "Q",
        0x52 => "R",
        0x53 => "S",
        0x54 => "T",
        0x55 => "U",
        0x56 => "V",
        0x57 => "W",
        0x58 => "X",
        0x59 => "Y",
        0x5A => "Z",
        0x70 => "F1",
        0x71 => "F2",
        0x72 => "F3",
        0x73 => "F4",
        0x74 => "F5",
        0x75 => "F6",
        0x76 => "F7",
        0x77 => "F8",
        0x78 => "F9",
        0x79 => "F10",
        0x7A => "F11",
        0x7B => "F12",
        _ => "Key",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_summon_is_alt_space() {
        match default_toggle_palette() {
            HotKeyBinding::Combo(k) => {
                assert_eq!(k.vk, 0x20);
                assert!(k.modifiers.alt && !k.modifiers.ctrl && !k.modifiers.win);
            }
            _ => panic!("expected combo"),
        }
    }

    #[test]
    fn default_toggle_palette_has_only_alt() {
        match default_toggle_palette() {
            HotKeyBinding::Combo(k) => {
                assert!(!k.modifiers.shift);
            }
            _ => panic!("expected combo"),
        }
    }

    #[test]
    fn capture_combo_requires_commanding_modifier() {
        assert_eq!(
            capture_keydown(0x43, Modifiers::none()),
            CaptureOutcome::Ignore
        );
        let alt = Modifiers {
            ctrl: false,
            alt: true,
            shift: false,
            win: false,
        };
        match capture_keydown(0x43, alt) {
            CaptureOutcome::Commit(HotKeyBinding::Combo(k)) => {
                assert_eq!(k.vk, 0x43);
                assert!(k.modifiers.alt);
            }
            other => panic!("{other:?}"),
        }
        assert_eq!(
            capture_keydown(0x1B, Modifiers::none()),
            CaptureOutcome::Cancel
        );
        assert_eq!(
            capture_keydown(0x2E, Modifiers::none()),
            CaptureOutcome::Clear
        );
        match capture_keydown(0x70, Modifiers::none()) {
            CaptureOutcome::Commit(HotKeyBinding::Combo(k)) => assert_eq!(k.vk, 0x70),
            other => panic!("{other:?}"),
        }
        assert_eq!(
            capture_keydown(0x11, Modifiers::none()),
            CaptureOutcome::Ignore
        );
    }
}
