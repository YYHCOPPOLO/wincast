//! Settings → Backup: export, import, Raycast import.

use tinycast_pure::theme;

const ROW_H: f32 = 44.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BackupHit {
    Export,
    Import,
    Raycast,
}

pub fn content_height() -> f32 {
    24.0 + ROW_H * 3.0 + theme::spacing::XL * 2.0
}

fn row_y(i: usize) -> f32 {
    24.0 + i as f32 * (ROW_H + theme::spacing::XL)
}

pub fn hit(_x: f32, y: f32, scroll: f32) -> Option<BackupHit> {
    let y = y + scroll;
    let hits = [BackupHit::Export, BackupHit::Import, BackupHit::Raycast];
    for (i, hit) in hits.into_iter().enumerate() {
        let top = row_y(i);
        if y >= top && y < top + ROW_H {
            return Some(hit);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn export_is_first_row() {
        assert_eq!(hit(20.0, 30.0, 0.0), Some(BackupHit::Export));
    }
}
