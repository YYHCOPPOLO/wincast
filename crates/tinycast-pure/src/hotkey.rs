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
}
