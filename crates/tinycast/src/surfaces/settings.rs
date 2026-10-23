//! The settings surface is a Slint window; its host lives in `ui_host`.
//!
//! `SettingsWindow` keeps the API the Direct2D window had (`create`, `show`,
//! `invalidate`, `set_locale`, `on_tab_changed`, `commit_pending`) so
//! `platform::run`, the tray and `AppCore` do not change.
pub use crate::ui_host::settings::SettingsWindow;
