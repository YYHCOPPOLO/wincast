//! Shared surface primitives, the Direct2D counterparts of upstream
//! `DesignSystem/Glass/*` and `DesignSystem/Scrolling/*` (abue-ammar/tinycast@da2c4e44):
//!
//! - [`fill_glass_panel`]: a floating surface (menu, popover, dialog) — tinted fill plus
//!   a hairline rim. Upstream's `glassFrost` overlay was removed on main, so there is no
//!   frost pass any more.
//! - [`hairline`]: the 1-DIP separator.
//! - [`fill_glass_pill`]: a capsule control at rest / hover / pressed
//!   (`Theme.Colors.controlHover` / `controlPressed`).
//! - [`push_edge_fade`] / [`pop_edge_fade`]: an opacity mask that dissolves content into
//!   the top and bottom bands (`EdgeDissolve.swift`, `OverflowFade.swift`).
//!
//! Every surface built in later phases should paint through these instead of
//! hand-picking alphas so the tokens stay the single source.

use std::mem::ManuallyDrop;

use tinycast_pure::palette_placement::DipRect;
use tinycast_pure::theme;
use windows::Foundation::Numerics::Matrix3x2;
use windows::Win32::Graphics::Direct2D::Common::{D2D1_GRADIENT_STOP, D2D_POINT_2F, D2D_RECT_F};
use windows::Win32::Graphics::Direct2D::{
    ID2D1Brush, ID2D1Layer, ID2D1RenderTarget, D2D1_ANTIALIAS_MODE_PER_PRIMITIVE,
    D2D1_EXTEND_MODE_CLAMP, D2D1_GAMMA_2_2, D2D1_LAYER_OPTIONS_NONE, D2D1_LAYER_PARAMETERS,
    D2D1_LINEAR_GRADIENT_BRUSH_PROPERTIES, D2D1_ROUNDED_RECT,
};

use super::appearance;
use super::squircle::{fill_squircle, stroke_squircle};

/// How a glass control is being interacted with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ControlState {
    Rest,
    Hover,
    Pressed,
}

/// Opaque-enough tint for a floating glass surface when no live blur sits behind it.
/// Matches what the palette menu painted before (dark 0.08 gray, light 0.98 gray).
pub fn glass_tint_rgba(appearance: u8) -> (f32, f32, f32, f32) {
    if appearance == 0 {
        (0.08, 0.08, 0.08, 0.94)
    } else {
        (0.98, 0.98, 0.98, 0.96)
    }
}

pub fn rim_rgba(appearance: u8) -> (f32, f32, f32, f32) {
    theme::colors::ramp_rgba(
        appearance,
        theme::colors::BORDER_DARK_ALPHA,
        theme::colors::BORDER_LIGHT_ALPHA,
    )
}

pub fn separator_rgba(appearance: u8) -> (f32, f32, f32, f32) {
    theme::colors::ramp_rgba(
        appearance,
        theme::colors::SEPARATOR_DARK_ALPHA,
        theme::colors::SEPARATOR_LIGHT_ALPHA,
    )
}

pub fn control_rgba(appearance: u8, state: ControlState) -> (f32, f32, f32, f32) {
    use theme::colors::*;
    match state {
        ControlState::Rest => ramp_rgba(
            appearance,
            CONTROL_SURFACE_DARK_ALPHA,
            CONTROL_SURFACE_LIGHT_ALPHA,
        ),
        ControlState::Hover => ramp_rgba(
            appearance,
            CONTROL_HOVER_DARK_ALPHA,
            CONTROL_HOVER_LIGHT_ALPHA,
        ),
        ControlState::Pressed => ramp_rgba(
            appearance,
            CONTROL_PRESSED_DARK_ALPHA,
            CONTROL_PRESSED_LIGHT_ALPHA,
        ),
    }
}

/// Tinted squircle plus a hairline rim inset by half a stroke so it stays crisp.
pub fn fill_glass_panel(
    target: &ID2D1RenderTarget,
    rect: DipRect,
    radius: f32,
    appearance: u8,
) -> windows::core::Result<()> {
    fill_squircle(target, rect, radius, glass_tint_rgba(appearance))?;
    stroke_glass_rim(target, rect, radius, appearance)
}

pub fn stroke_glass_rim(
    target: &ID2D1RenderTarget,
    rect: DipRect,
    radius: f32,
    appearance: u8,
) -> windows::core::Result<()> {
    let half = theme::size::HAIRLINE * 0.5;
    let inset = DipRect {
        x: rect.x + half,
        y: rect.y + half,
        w: (rect.w - theme::size::HAIRLINE).max(0.0),
        h: (rect.h - theme::size::HAIRLINE).max(0.0),
    };
    stroke_squircle(
        target,
        inset,
        (radius - half).max(0.0),
        rim_rgba(appearance),
        theme::size::HAIRLINE,
    )
}

/// Horizontal 1-DIP separator from `x0` to `x1` whose top edge is `y`.
pub fn hairline(
    target: &ID2D1RenderTarget,
    x0: f32,
    x1: f32,
    y: f32,
    appearance: u8,
) -> windows::core::Result<()> {
    let brush = unsafe {
        target.CreateSolidColorBrush(&appearance::color(separator_rgba(appearance)), None)?
    };
    let rect = D2D_RECT_F {
        left: x0,
        top: y,
        right: x1,
        bottom: y + theme::size::HAIRLINE,
    };
    unsafe { target.FillRectangle(&rect, &brush) };
    Ok(())
}

/// Capsule control fill (Liquid Glass button / pill).
pub fn fill_glass_pill(
    target: &ID2D1RenderTarget,
    rect: DipRect,
    appearance: u8,
    state: ControlState,
) -> windows::core::Result<()> {
    let brush = unsafe {
        target.CreateSolidColorBrush(&appearance::color(control_rgba(appearance, state)), None)?
    };
    let radius = rect.h.min(rect.w) * 0.5;
    let rounded = D2D1_ROUNDED_RECT {
        rect: D2D_RECT_F {
            left: rect.x,
            top: rect.y,
            right: rect.x + rect.w,
            bottom: rect.y + rect.h,
        },
        radiusX: radius,
        radiusY: radius,
    };
    unsafe { target.FillRoundedRectangle(&rounded, &brush) };
    Ok(())
}

/// Gradient stops (offset, opacity) for an edge fade over a region `height` tall.
/// The top band fades from `top_floor` to 1, the bottom band from 1 to `bottom_floor`.
/// A zero band is skipped. Offsets are clamped so overlapping bands stay ordered.
pub fn edge_fade_stops(
    height: f32,
    top_band: f32,
    top_floor: f32,
    bottom_band: f32,
    bottom_floor: f32,
) -> Vec<(f32, f32)> {
    let h = height.max(1.0);
    let top = (top_band / h).clamp(0.0, 0.5);
    let bottom = (1.0 - bottom_band / h).clamp(0.5, 1.0);
    let mut stops = Vec::with_capacity(4);
    stops.push((0.0, if top > 0.0 { top_floor } else { 1.0 }));
    if top > 0.0 {
        stops.push((top, 1.0));
    }
    if bottom < 1.0 {
        stops.push((bottom, 1.0));
    }
    stops.push((1.0, if bottom < 1.0 { bottom_floor } else { 1.0 }));
    stops
}

/// Opens a layer whose opacity follows [`edge_fade_stops`] over `rect`. Every paint until
/// [`pop_edge_fade`] is dissolved toward the edges. Use the bands from `theme::fade`.
pub fn push_edge_fade(
    target: &ID2D1RenderTarget,
    rect: DipRect,
    top_band: f32,
    top_floor: f32,
    bottom_band: f32,
    bottom_floor: f32,
) -> windows::core::Result<()> {
    let stops: Vec<D2D1_GRADIENT_STOP> =
        edge_fade_stops(rect.h, top_band, top_floor, bottom_band, bottom_floor)
            .into_iter()
            .map(|(position, a)| D2D1_GRADIENT_STOP {
                position,
                color: appearance::color((0.0, 0.0, 0.0, a)),
            })
            .collect();
    unsafe {
        let collection =
            target.CreateGradientStopCollection(&stops, D2D1_GAMMA_2_2, D2D1_EXTEND_MODE_CLAMP)?;
        let brush = target.CreateLinearGradientBrush(
            &D2D1_LINEAR_GRADIENT_BRUSH_PROPERTIES {
                startPoint: D2D_POINT_2F {
                    x: rect.x,
                    y: rect.y,
                },
                endPoint: D2D_POINT_2F {
                    x: rect.x,
                    y: rect.y + rect.h,
                },
            },
            None,
            &collection,
        )?;
        let brush: ID2D1Brush = windows::core::Interface::cast(&brush)?;
        let mut params = D2D1_LAYER_PARAMETERS {
            contentBounds: D2D_RECT_F {
                left: rect.x,
                top: rect.y,
                right: rect.x + rect.w,
                bottom: rect.y + rect.h,
            },
            geometricMask: ManuallyDrop::new(None),
            maskAntialiasMode: D2D1_ANTIALIAS_MODE_PER_PRIMITIVE,
            maskTransform: Matrix3x2::identity(),
            opacity: 1.0,
            opacityBrush: ManuallyDrop::new(Some(brush)),
            layerOptions: D2D1_LAYER_OPTIONS_NONE,
        };
        target.PushLayer(&params, None::<&ID2D1Layer>);
        ManuallyDrop::drop(&mut params.opacityBrush);
    }
    Ok(())
}

pub fn pop_edge_fade(target: &ID2D1RenderTarget) {
    unsafe { target.PopLayer() };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn control_states_step_up_the_ramp() {
        for appearance in [0u8, 1] {
            let rest = control_rgba(appearance, ControlState::Rest).3;
            let hover = control_rgba(appearance, ControlState::Hover).3;
            let pressed = control_rgba(appearance, ControlState::Pressed).3;
            assert!(rest < hover && hover < pressed);
        }
        assert_eq!(control_rgba(0, ControlState::Hover).3, 0.16);
        assert_eq!(control_rgba(1, ControlState::Pressed).3, 0.20);
    }

    #[test]
    fn edge_fade_stops_cover_both_bands() {
        let stops = edge_fade_stops(400.0, 40.0, 0.15, 80.0, 0.25);
        assert_eq!(
            stops,
            vec![(0.0, 0.15), (0.1, 1.0), (0.8, 1.0), (1.0, 0.25)]
        );
        let none = edge_fade_stops(400.0, 0.0, 0.15, 0.0, 0.25);
        assert_eq!(none, vec![(0.0, 1.0), (1.0, 1.0)]);
    }

    #[test]
    fn edge_fade_bands_never_cross() {
        let stops = edge_fade_stops(60.0, 100.0, 0.0, 100.0, 0.0);
        assert!(stops.windows(2).all(|w| w[0].0 <= w[1].0));
    }
}
