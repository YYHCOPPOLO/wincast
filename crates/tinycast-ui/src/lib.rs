//! Tinycast's Slint interface.
//!
//! Slint is used under the Royalty-free 2.0 terms. The About page discloses
//! that use. Files in this repository stay MIT.

pub mod bridge;

pub use bridge::apply_appearance;

slint::include_modules!();
