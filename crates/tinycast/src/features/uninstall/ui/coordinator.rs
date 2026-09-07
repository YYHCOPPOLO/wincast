use tinycast_pure::i18n::{chrome, Chrome, UiLang};
use tinycast_pure::uninstall::{UninstallCandidate, UninstallSelection};

use crate::surfaces::dialog::{self, ConfirmPrompt};

pub const CONFIRM_TITLE: &str = "Move to Recycle Bin?";
pub const CONFIRM_ACTION: &str = "Uninstall";

pub fn confirm(name: &str, count: usize, lang: UiLang) -> bool {
    dialog::confirm(&ConfirmPrompt {
        title: tinycast_pure::i18n::uninstall_confirm_title(lang).into(),
        message: tinycast_pure::i18n::uninstall_confirm_message(name, count, lang),
        accept: tinycast_pure::i18n::uninstall_label(lang).into(),
        cancel: chrome(Chrome::Cancel, lang).into(),
    })
}

pub fn primary_label() -> &'static str {
    "Uninstall"
}

pub fn default_selection(candidates: &[UninstallCandidate]) -> UninstallSelection {
    let removable: Vec<&str> = candidates
        .iter()
        .filter(|c| !c.locked)
        .map(|c| c.id.as_str())
        .collect();
    let mut sel = UninstallSelection::new();
    for id in &removable {
        sel.toggle(id);
    }
    sel.intersect_removable(&removable);
    sel
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locked_rows_are_not_prechecked() {
        let candidates = vec![UninstallCandidate {
            id: "locked".into(),
            name: "x".into(),
            path: "C:/Windows/x".into(),
            locked: true,
            is_bundle: false,
            size: None,
        }];
        let sel = default_selection(&candidates);
        assert!(!sel.contains("locked"));
    }
}
