//! Continue/Cancel dialogs. The caller registers the accepted side with the
//! modal gate and returns; the Slint card runs it only when the user accepts.
//! Escape, a click outside and a second dialog never execute anything.

use std::cell::Cell;
use std::rc::Rc;

use tinycast_pure::i18n::{chrome, Chrome, UiLang};

use crate::ui_host::modal::{self, DialogRequest};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfirmPrompt {
    pub title: String,
    pub message: String,
    pub accept: String,
    pub cancel: String,
    /// Dangerous action: red ink and the default focus starts on Cancel.
    pub destructive: bool,
}

/// Claims the one dialog slot. The modal gate owns it, so a confirm that is
/// already registered blocks a second one.
pub fn begin() -> bool {
    modal::begin()
}

pub fn end() {
    modal::end();
}

pub fn is_up() -> bool {
    modal::is_up()
}

/// Continue / Cancel. Returns whether the card opened; `on_accept` runs later,
/// from the event loop, and only if the user accepted.
pub fn confirm(prompt: &ConfirmPrompt, on_accept: impl FnOnce() + 'static) -> bool {
    open(prompt, "info.circle", None, Box::new(on_accept))
}

/// Single Continue button for failure reports; accepting changes nothing.
pub fn alert(title: &str, message: &str, lang: UiLang) -> bool {
    let prompt = ConfirmPrompt {
        title: title.to_string(),
        message: message.to_string(),
        accept: tinycast_pure::i18n::onboarding_continue(lang).into(),
        cancel: String::new(),
        destructive: false,
    };
    open(&prompt, "info.circle", None, Box::new(|| {}))
}

/// Set Volume slider: left/right walk the 5% grid, Enter accepts, Escape
/// cancels. The level the user left behind goes to `on_accept`.
pub fn pick_volume(current: f32, lang: UiLang, on_accept: impl FnOnce(f32) + 'static) -> bool {
    let level = Rc::new(Cell::new(tinycast_pure::volume::clamped(current)));
    let accepted = Rc::clone(&level);
    let prompt = ConfirmPrompt {
        title: tinycast_pure::i18n::dialog_set_volume(lang).into(),
        message: tinycast_pure::i18n::dialog_set_volume_message(lang).into(),
        accept: tinycast_pure::i18n::dialog_set_volume(lang).into(),
        cancel: chrome(Chrome::Cancel, lang).into(),
        destructive: false,
    };
    open(
        &prompt,
        "speaker.wave.2",
        Some(level),
        Box::new(move || on_accept(accepted.get())),
    )
}

fn open(
    prompt: &ConfirmPrompt,
    symbol: &str,
    volume: Option<Rc<Cell<f32>>>,
    on_accept: Box<dyn FnOnce()>,
) -> bool {
    let request = DialogRequest {
        heading: prompt.title.clone(),
        message: prompt.message.clone(),
        accept: prompt.accept.clone(),
        cancel: prompt.cancel.clone(),
        symbol: symbol.to_string(),
        destructive: prompt.destructive,
        show_cancel: !prompt.cancel.is_empty(),
        // Confirmations are the compact card; form dialogs arrive with the
        // editors in phase 8.
        form: false,
        volume,
    };
    modal::open(request, on_accept)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn second_dialog_is_refused_while_one_is_up() {
        assert!(begin());
        assert!(is_up());
        assert!(!begin());
        end();
        assert!(!is_up());
        assert!(begin());
        end();
    }

    #[test]
    fn dialog_slot_is_exclusive_and_per_thread() {
        assert!(begin());
        assert!(is_up());
        assert!(!begin(), "a second dialog must not stack");
        std::thread::spawn(|| assert!(!is_up(), "other threads see their own slot"))
            .join()
            .unwrap();
        end();
        assert!(!is_up());
    }

    #[test]
    fn dialog_width_is_420() {
        assert_eq!(tinycast_pure::layout::dialog::frame().0, 420.0);
        assert!(tinycast_pure::layout::dialog::cancel_is_leading());
    }

    #[test]
    fn dialog_paint_is_not_gdi_marker() {
        let src = include_str!("dialog.rs");
        let impl_src = src.split("mod tests").next().unwrap_or(src);
        let marker = ["Text", "OutW"].concat();
        assert!(
            !impl_src.contains(&marker),
            "dialog must not paint with GDI {marker}"
        );
    }
}
