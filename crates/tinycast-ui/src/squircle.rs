//! Same corner sampling as `design_system::squircle`: `k = 0.46 * r`.
//!
//! Slint's public path value is the SVG command string on `Path.commands`.
//! `PathElement` is only reachable through the private generated-code API.

/// SVG commands for a filled squircle in path coordinates.
pub fn squircle_commands(
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    radius: f32,
) -> slint::SharedString {
    let left = x;
    let top = y;
    let right = x + width;
    let bottom = y + height;
    let r = radius.min(width * 0.5).min(height * 0.5).max(0.0);
    let k = 0.46 * r;
    let mut cmd = String::new();
    push(&mut cmd, "M", &[(left + r, top)]);
    push(&mut cmd, " L", &[(right - r, top)]);
    push(
        &mut cmd,
        " C",
        &[(right - r + k, top), (right, top + r - k), (right, top + r)],
    );
    push(&mut cmd, " L", &[(right, bottom - r)]);
    push(
        &mut cmd,
        " C",
        &[
            (right, bottom - r + k),
            (right - r + k, bottom),
            (right - r, bottom),
        ],
    );
    push(&mut cmd, " L", &[(left + r, bottom)]);
    push(
        &mut cmd,
        " C",
        &[
            (left + r - k, bottom),
            (left, bottom - r + k),
            (left, bottom - r),
        ],
    );
    push(&mut cmd, " L", &[(left, top + r)]);
    push(
        &mut cmd,
        " C",
        &[(left, top + r - k), (left + r - k, top), (left + r, top)],
    );
    cmd.push_str(" Z");
    slint::SharedString::from(cmd)
}

fn push(cmd: &mut String, op: &str, points: &[(f32, f32)]) {
    cmd.push_str(op);
    for (x, y) in points {
        cmd.push(' ');
        cmd.push_str(&format_coord(*x));
        cmd.push(' ');
        cmd.push_str(&format_coord(*y));
    }
}

fn format_coord(value: f32) -> String {
    let scaled = (value * 1000.0).round() / 1000.0;
    let text = format!("{scaled:.3}");
    text.trim_end_matches('0').trim_end_matches('.').to_string()
}

#[cfg(test)]
mod tests {
    use super::{format_coord, squircle_commands};

    #[test]
    fn radius_26_matches_direct2d_handles() {
        let commands = squircle_commands(0.0, 0.0, 220.0, 120.0, 26.0);
        let r = 26.0;
        let k = 0.46 * r;
        let right = 220.0;
        let bottom = 120.0;
        let top_right = format!(
            "C {} {} {} {} {} {}",
            format_coord(right - r + k),
            format_coord(0.0),
            format_coord(right),
            format_coord(r - k),
            format_coord(right),
            format_coord(r),
        );
        let bottom_left = format!(
            "C {} {} {} {} {} {}",
            format_coord(r - k),
            format_coord(bottom),
            format_coord(0.0),
            format_coord(bottom - r + k),
            format_coord(0.0),
            format_coord(bottom - r),
        );
        let text = commands.as_str();
        assert!(text.starts_with("M "), "{text}");
        assert!(text.contains(&top_right), "{text}");
        assert!(text.contains(&bottom_left), "{text}");
        assert!(text.ends_with(" Z"), "{text}");
    }

    #[test]
    fn radius_clamps_to_half_the_shorter_side() {
        let commands = squircle_commands(0.0, 0.0, 10.0, 40.0, 1000.0);
        let r = 5.0;
        let k = 0.46 * r;
        let expected = format!("M {} {}", format_coord(r), format_coord(0.0),);
        assert!(commands.as_str().starts_with(&expected), "{commands}");
        assert!(
            commands.contains(&format_coord(k)) || commands.contains(&format_coord(10.0 - r + k)),
            "{commands} k={k}"
        );
    }
}
