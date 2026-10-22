//! Tinycast's Slint interface.
//!
//! Slint is used under the Royalty-free 2.0 terms. The About page discloses
//! that use. Files in this repository stay MIT.

pub mod bridge;
mod squircle;

pub use bridge::apply_appearance;
pub use squircle::squircle_commands;

slint::include_modules!();
