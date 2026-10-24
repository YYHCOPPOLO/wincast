//! Settings → Calendar: enable (consent), auto-join, join window, camera preview.

use tinycast_pure::theme;

use crate::design_system::settings as ds;

const ROW_H: f32 = 52.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CalendarHit {
    Enable,
    AutoJoin,
    Camera,
    JoinWindow,
}

pub fn section_header() -> &'static str {
    "Calendar"
}

pub fn content_height() -> f32 {
    ds::form_origin() + ROW_H * 4.0 + ds::CARD_PAD + theme::spacing::SECTION_SPACING
}

fn row_y(i: usize) -> f32 {
    ds::form_origin() + i as f32 * ROW_H
}

pub fn hit(_x: f32, y: f32, scroll: f32) -> Option<CalendarHit> {
    let y = y + scroll;
    let hits = [
        CalendarHit::Enable,
        CalendarHit::AutoJoin,
        CalendarHit::Camera,
        CalendarHit::JoinWindow,
    ];
    for (i, hit) in hits.into_iter().enumerate() {
        let top = row_y(i);
        if y >= top && y < top + ROW_H {
            return Some(hit);
        }
    }
    None
}

pub fn join_window_title(minutes: i64) -> &'static str {
    match minutes {
        1 => "1 minute",
        2 => "2 minutes",
        10 => "10 minutes",
        15 => "15 minutes",
        _ => "5 minutes",
    }
}

pub fn cycle_join_window(minutes: i64) -> i64 {
    match minutes {
        1 => 2,
        2 => 5,
        5 => 10,
        10 => 15,
        _ => 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enable_is_first_row() {
        assert_eq!(
            hit(20.0, ds::form_origin() + 4.0, 0.0),
            Some(CalendarHit::Enable)
        );
        assert_eq!(cycle_join_window(5), 10);
        assert_eq!(hit(20.0, row_y(1) + 4.0, 0.0), Some(CalendarHit::AutoJoin));
        assert!((row_y(1) - row_y(0) - ROW_H).abs() < 0.001);
    }
}
