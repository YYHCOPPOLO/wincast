//! Golden images for the design-system primitives at 100 / 125 / 150 %.
//!
//! Each case paints offscreen through `test_render::with_offscreen_dpi` and is compared
//! with `crates/tinycast/tests/golden/<case>@<scale>.png`, allowing small antialiasing
//! drift. Regenerate with `scripts/regen-goldens.sh` (Wine) or
//! `scripts/regen-goldens.ps1` (Windows), which set `TINYCAST_UPDATE_GOLDENS=1`.
//! Cases are text-free on purpose: Wine has no Segoe UI Variable, so text goldens would
//! only be meaningful when regenerated on Windows (planned with the P1 palette pass).
//! The edge fade is not a golden either: Wine's d2d1 ignores layer opacity brushes, so a
//! Wine golden would freeze the *unfaded* output. Add it once goldens come from Windows.

use std::path::PathBuf;

use tinycast_pure::palette_placement::DipRect;
use tinycast_pure::theme;
use windows::Win32::Graphics::Direct2D::ID2D1RenderTarget;

use super::glass::{self, ControlState};
use super::test_render::with_offscreen_dpi;

const SCALES: [(u32, &str); 3] = [(96, "100"), (120, "125"), (144, "150")];
/// A pixel counts as different when any channel moves more than this.
const CHANNEL_TOLERANCE: u8 = 16;
/// At most this share of pixels may differ.
const PIXEL_TOLERANCE: f64 = 0.02;

type Painter = fn(&ID2D1RenderTarget, u8) -> windows::core::Result<()>;

struct Case {
    name: &'static str,
    w: f32,
    h: f32,
    paint: Painter,
}

fn rect(x: f32, y: f32, w: f32, h: f32) -> DipRect {
    DipRect { x, y, w, h }
}

fn cases() -> Vec<Case> {
    vec![
        Case {
            name: "panel_scrim",
            w: 120.0,
            h: 80.0,
            paint: |t, a| super::panel::paint_panel_scrim(t, 120.0, 80.0, a),
        },
        Case {
            name: "dialog_surface",
            w: 120.0,
            h: 80.0,
            paint: |t, a| {
                super::panel::paint_scrim(t, 120.0, 80.0, theme::radius::DIALOG_SURFACE, a)
            },
        },
        Case {
            name: "glass_panel",
            w: 120.0,
            h: 80.0,
            paint: |t, a| {
                glass::fill_glass_panel(
                    t,
                    rect(4.0, 4.0, 112.0, 72.0),
                    theme::radius::MENU_PANEL,
                    a,
                )
            },
        },
        Case {
            name: "glass_pills",
            w: 120.0,
            h: 40.0,
            paint: |t, a| {
                let h = theme::size::BAR_BUTTON_HEIGHT;
                glass::fill_glass_pill(t, rect(4.0, 6.0, 34.0, h), a, ControlState::Rest)?;
                glass::fill_glass_pill(t, rect(43.0, 6.0, 34.0, h), a, ControlState::Hover)?;
                glass::fill_glass_pill(t, rect(82.0, 6.0, 34.0, h), a, ControlState::Pressed)
            },
        },
        Case {
            name: "hairline_rows",
            w: 120.0,
            h: 80.0,
            paint: |t, a| {
                super::panel::paint_panel_scrim(t, 120.0, 80.0, a)?;
                super::row::paint_row_fill(t, 120.0, 4.0, true, false, a)?;
                glass::hairline(t, 10.0, 110.0, 46.0, a)
            },
        },
    ]
}

fn golden_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("golden")
}

/// Premultiplied BGRA (D2D) → straight RGBA (PNG).
fn to_rgba(bgra: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(bgra.len());
    for px in bgra.chunks_exact(4) {
        let a = px[3];
        let un = |c: u8| -> u8 {
            if a == 0 {
                0
            } else {
                ((c as u32 * 255 + a as u32 / 2) / a as u32).min(255) as u8
            }
        };
        out.extend_from_slice(&[un(px[2]), un(px[1]), un(px[0]), a]);
    }
    out
}

fn write_png(path: &std::path::Path, w: usize, h: usize, rgba: &[u8]) {
    let file = std::fs::File::create(path).expect("create golden");
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), w as u32, h as u32);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.set_compression(png::Compression::Best);
    let mut writer = encoder.write_header().expect("png header");
    writer.write_image_data(rgba).expect("png data");
}

fn read_png(path: &std::path::Path) -> Option<(usize, usize, Vec<u8>)> {
    let file = std::fs::File::open(path).ok()?;
    let mut reader = png::Decoder::new(file).read_info().ok()?;
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).ok()?;
    if info.color_type != png::ColorType::Rgba || info.bit_depth != png::BitDepth::Eight {
        return None;
    }
    buf.truncate(info.buffer_size());
    Some((info.width as usize, info.height as usize, buf))
}

fn differing_share(a: &[u8], b: &[u8]) -> f64 {
    let pixels = a.len() / 4;
    let off = a
        .chunks_exact(4)
        .zip(b.chunks_exact(4))
        .filter(|(p, q)| {
            p.iter()
                .zip(*q)
                .any(|(x, y)| x.abs_diff(*y) > CHANNEL_TOLERANCE)
        })
        .count();
    off as f64 / pixels.max(1) as f64
}

#[test]
fn design_system_goldens() {
    let update = std::env::var("TINYCAST_UPDATE_GOLDENS").is_ok_and(|v| v == "1");
    let dir = golden_dir();
    if update {
        std::fs::create_dir_all(&dir).expect("golden dir");
    }
    let mut failures = Vec::new();
    for case in cases() {
        for appearance in [0u8, 1] {
            let theme_name = if appearance == 0 { "dark" } else { "light" };
            for (dpi, pct) in SCALES {
                let (w, h, bgra) =
                    with_offscreen_dpi(case.w, case.h, dpi, |t| (case.paint)(t, appearance))
                        .unwrap_or_else(|err| panic!("{} @{pct}: {err:?}", case.name));
                assert_eq!(w, (case.w * dpi as f32 / 96.0).ceil() as usize);
                let rgba = to_rgba(&bgra);
                assert!(
                    rgba.chunks_exact(4).any(|p| p[3] != 0),
                    "{} {theme_name} @{pct} painted nothing",
                    case.name
                );
                let path = dir.join(format!("{}_{theme_name}@{pct}.png", case.name));
                if update {
                    write_png(&path, w, h, &rgba);
                    continue;
                }
                match read_png(&path) {
                    None => failures.push(format!("missing {}", path.display())),
                    Some((gw, gh, _)) if (gw, gh) != (w, h) => {
                        failures.push(format!("{}: size {gw}x{gh} != {w}x{h}", path.display()))
                    }
                    Some((_, _, golden)) => {
                        let share = differing_share(&golden, &rgba);
                        if share > PIXEL_TOLERANCE {
                            failures.push(format!(
                                "{}: {:.1}% of pixels differ",
                                path.display(),
                                share * 100.0
                            ));
                        }
                    }
                }
            }
        }
    }
    assert!(
        failures.is_empty(),
        "golden mismatches (regenerate with scripts/regen-goldens.sh):\n{}",
        failures.join("\n")
    );
}

#[test]
fn unpremultiply_restores_straight_alpha() {
    assert_eq!(to_rgba(&[0, 0, 0, 0]), vec![0, 0, 0, 0]);
    assert_eq!(to_rgba(&[0, 64, 128, 128]), vec![255, 128, 0, 128]);
}
