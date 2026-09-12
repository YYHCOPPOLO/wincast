//! Keyboard focus model for custom-drawn Settings. Shared with hit-testing ids.

use tinycast_pure::palette_placement::DipRect;
use tinycast_pure::settings_tab::SettingsTab;
use tinycast_pure::theme;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FocusRole {
    Tab,
    Toggle,
    Button,
    Text,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FocusItem {
    pub tab: Option<SettingsTab>,
    pub name: String,
    pub role: FocusRole,
    pub enabled: bool,
    pub checked: Option<bool>,
    pub secret: bool,
    pub rect: DipRect,
    pub edit_id: Option<usize>,
    pub scrolls: bool,
}

impl FocusItem {
    pub fn focusable(&self) -> bool {
        self.enabled && !self.secret
    }
}

pub fn sidebar_items(selected: SettingsTab, lang: tinycast_pure::i18n::UiLang) -> Vec<FocusItem> {
    let mut y = theme::spacing::XL;
    let mut items = Vec::new();
    for (i, section) in tinycast_pure::settings_tab::SettingsSection::all()
        .into_iter()
        .enumerate()
    {
        if i > 0 {
            y += theme::spacing::SECTION_SPACING;
        }
        y += 22.0;
        for &tab in section.tabs() {
            items.push(FocusItem {
                tab: Some(tab),
                name: tinycast_pure::i18n::settings_tab_title(tab, lang).to_string(),
                role: FocusRole::Tab,
                enabled: true,
                checked: Some(tab == selected),
                secret: false,
                rect: DipRect {
                    x: theme::spacing::SM,
                    y,
                    w: theme::size::SETTINGS_SIDEBAR - theme::spacing::SM * 2.0,
                    h: 28.0,
                },
                edit_id: None,
                scrolls: false,
            });
            y += 28.0;
        }
    }
    items
}

pub fn content_item(
    name: impl Into<String>,
    role: FocusRole,
    enabled: bool,
    checked: Option<bool>,
    secret: bool,
    detail: DipRect,
    edit_id: Option<usize>,
) -> FocusItem {
    FocusItem {
        tab: None,
        name: name.into(),
        role,
        enabled,
        checked,
        secret,
        rect: DipRect {
            x: detail.x + theme::size::SETTINGS_SIDEBAR,
            y: detail.y,
            w: detail.w,
            h: detail.h,
        },
        edit_id,
        scrolls: true,
    }
}

pub fn traverse(items: &[FocusItem], current: Option<usize>, back: bool) -> Option<usize> {
    let enabled: Vec<usize> = items
        .iter()
        .enumerate()
        .filter(|(_, item)| item.focusable())
        .map(|(i, _)| i)
        .collect();
    if enabled.is_empty() {
        return None;
    }
    let pos = current.and_then(|c| enabled.iter().position(|&i| i == c));
    let next = match (pos, back) {
        (None, false) => {
            if let Some(c) = current {
                walk_from(items, c, back).unwrap_or(enabled[0])
            } else {
                enabled[0]
            }
        }
        (None, true) => {
            if let Some(c) = current {
                walk_from(items, c, back).unwrap_or(*enabled.last().unwrap())
            } else {
                *enabled.last().unwrap()
            }
        }
        (Some(0), true) => *enabled.last().unwrap(),
        (Some(p), true) => enabled[p - 1],
        (Some(p), false) => enabled[(p + 1) % enabled.len()],
    };
    Some(next)
}

fn walk_from(items: &[FocusItem], current: usize, back: bool) -> Option<usize> {
    let n = items.len();
    if n == 0 {
        return None;
    }
    for step in 1..=n {
        let i = if back {
            (current + n - step) % n
        } else {
            (current + step) % n
        };
        if items[i].focusable() {
            return Some(i);
        }
    }
    None
}

pub fn acc_value(item: &FocusItem) -> Option<String> {
    if item.secret {
        return None;
    }
    item.checked
        .map(|on| if on { "on".into() } else { "off".into() })
}

pub fn acc_role_id(role: FocusRole) -> i32 {
    match role {
        FocusRole::Tab => 0x25,    // ROLE_SYSTEM_PAGETAB
        FocusRole::Toggle => 0x2C, // ROLE_SYSTEM_CHECKBUTTON
        FocusRole::Button => 0x2B, // ROLE_SYSTEM_PUSHBUTTON
        FocusRole::Text => 0x2A,   // ROLE_SYSTEM_TEXT
    }
}

pub fn acc_state(item: &FocusItem, focused: bool) -> i32 {
    let mut state = 0x0010_0000; // STATE_SYSTEM_FOCUSABLE
    if focused {
        state |= 0x4; // FOCUSED
    }
    if !item.enabled {
        state |= 0x1; // UNAVAILABLE
    }
    if item.checked == Some(true) {
        state |= 0x10; // CHECKED
        if item.role == FocusRole::Tab {
            state |= 0x2; // SELECTED
        }
    }
    if item.secret {
        state |= 0x20000; // PROTECTED
    }
    state
}

pub fn default_action(role: FocusRole) -> &'static str {
    match role {
        FocusRole::Toggle => "Toggle",
        FocusRole::Text => "Edit",
        _ => "Press",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tab_wraps_forward_and_back() {
        let items = sidebar_items(SettingsTab::General, tinycast_pure::i18n::UiLang::En);
        assert!(items.len() > 4);
        let first = traverse(&items, None, false).unwrap();
        assert_eq!(items[first].tab, Some(SettingsTab::General));
        let last = traverse(&items, None, true).unwrap();
        assert_eq!(items[last].tab, Some(SettingsTab::About));
        let after_last = traverse(&items, Some(last), false).unwrap();
        assert_eq!(after_last, first);
        let before_first = traverse(&items, Some(first), true).unwrap();
        assert_eq!(before_first, last);
    }

    #[test]
    fn disabled_and_secret_are_skipped() {
        let items = vec![
            FocusItem {
                tab: None,
                name: "On".into(),
                role: FocusRole::Toggle,
                enabled: true,
                checked: Some(true),
                secret: false,
                rect: DipRect {
                    x: 0.0,
                    y: 0.0,
                    w: 10.0,
                    h: 10.0,
                },
                edit_id: None,
                scrolls: true,
            },
            FocusItem {
                tab: None,
                name: "API key".into(),
                role: FocusRole::Text,
                enabled: true,
                checked: None,
                secret: true,
                rect: DipRect {
                    x: 0.0,
                    y: 20.0,
                    w: 10.0,
                    h: 10.0,
                },
                edit_id: Some(205),
                scrolls: true,
            },
            FocusItem {
                tab: None,
                name: "Off".into(),
                role: FocusRole::Toggle,
                enabled: false,
                checked: Some(false),
                secret: false,
                rect: DipRect {
                    x: 0.0,
                    y: 40.0,
                    w: 10.0,
                    h: 10.0,
                },
                edit_id: None,
                scrolls: true,
            },
        ];
        assert_eq!(traverse(&items, Some(0), false), Some(0));
        assert_eq!(traverse(&items, Some(1), false), Some(0));
        assert_eq!(acc_value(&items[1]), None);
        assert_eq!(acc_value(&items[0]).as_deref(), Some("on"));
        assert_eq!(acc_role_id(FocusRole::Button), 0x2B);
    }

    #[test]
    fn content_item_sits_in_the_detail_column() {
        let item = content_item(
            "Enable",
            FocusRole::Toggle,
            true,
            Some(true),
            false,
            DipRect {
                x: 32.0,
                y: 40.0,
                w: 200.0,
                h: 52.0,
            },
            None,
        );
        assert_eq!(item.rect.x, 32.0 + theme::size::SETTINGS_SIDEBAR);
        assert!(item.scrolls);
        assert!(item.tab.is_none());
    }
}
