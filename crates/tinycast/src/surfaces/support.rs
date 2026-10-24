//! The Support/About surfaces are Slint windows; their host lives in
//! `ui_host`. Every `showSupport` route lands here so the reminder anchor
//! moves once.
//!
//! `SupportWindow`/`AboutWindow` keep the API the Direct2D windows had
//! (`create`, `show`, `set_locale`) so `platform::run` and `AppCore` do not
//! change. The single Polar checkout stays the one Support action.

pub use crate::ui_host::support::{AboutWindow, SupportWindow};

/// The one Support checkout (the settings About page and the Support window
/// both open it).
pub const CHECKOUT: &str =
    "https://buy.polar.sh/polar_cl_NDVFC20DKQpLcNawsh97QzbARBXD3WNn8v35R0mbJmT";

/// Client size and the heart tile (`theme::size::SUPPORT_WINDOW`/`_ICON`).
pub const WIDTH: f32 = tinycast_pure::theme::size::SUPPORT_WINDOW.0;
pub const HEIGHT: f32 = tinycast_pure::theme::size::SUPPORT_WINDOW.1;
pub const ICON: f32 = tinycast_pure::theme::size::SUPPORT_ICON;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checkout_is_the_one_polar_link() {
        assert!(CHECKOUT.contains("polar.sh"));
    }

    #[test]
    fn support_width_is_460() {
        assert_eq!(crate::surfaces::support::WIDTH, 460.0);
        assert_eq!(crate::surfaces::support::HEIGHT, 360.0);
        assert_eq!(crate::surfaces::support::ICON, 76.0);
    }

    #[test]
    fn support_has_no_textout() {
        let src = include_str!("support.rs");
        let impl_src = src.split("mod tests").next().unwrap_or(src);
        let marker = ["Text", "OutW"].concat();
        assert!(!impl_src.contains(&marker));
    }
}
