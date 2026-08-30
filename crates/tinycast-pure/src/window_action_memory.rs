use std::collections::HashMap;
use std::hash::Hash;

use crate::palette_placement::DipRect;
use crate::window_command::WindowCommandId;
use crate::window_layout::is_tile_command;

const TOLERANCE: f32 = 2.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Record {
    pub restore: DipRect,
    pub applied: DipRect,
    pub command: WindowCommandId,
    pub step: i32,
    pub screen_id: i32,
    pub at_ms: i64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Decision {
    pub step: i32,
    pub restore: DipRect,
    pub can_restore: bool,
    pub last_tile: Option<WindowCommandId>,
}

pub struct WindowActionMemory<K: Hash + Eq> {
    pub capacity: usize,
    pub cycle_timeout_ms: Option<i64>,
    records: HashMap<K, Record>,
    order: Vec<K>,
}

impl<K: Hash + Eq + Clone> WindowActionMemory<K> {
    pub fn new() -> Self {
        Self {
            capacity: 64,
            cycle_timeout_ms: None,
            records: HashMap::new(),
            order: Vec::new(),
        }
    }

    pub fn count(&self) -> usize {
        self.records.len()
    }

    pub fn record(&self, key: &K) -> Option<Record> {
        self.records.get(key).copied()
    }

    pub fn decide(
        &self,
        key: &K,
        command: WindowCommandId,
        current: DipRect,
        screen_id: i32,
        cycle_enabled: bool,
        now_ms: i64,
    ) -> Decision {
        let Some(record) = self.records.get(key) else {
            return Decision {
                step: 0,
                restore: current,
                can_restore: false,
                last_tile: None,
            };
        };
        if !approx(current, record.applied) {
            return Decision {
                step: 0,
                restore: current,
                can_restore: true,
                last_tile: None,
            };
        }
        let last_tile = is_tile_command(record.command).then_some(record.command);
        let cycles = command.cycles_on_repeat();
        let expired = self
            .cycle_timeout_ms
            .map(|timeout| now_ms - record.at_ms > timeout)
            .unwrap_or(false);
        let continues = cycle_enabled
            && cycles
            && command == record.command
            && screen_id == record.screen_id
            && !expired;
        Decision {
            step: if continues { (record.step + 1) % 3 } else { 0 },
            restore: record.restore,
            can_restore: true,
            last_tile,
        }
    }

    pub fn commit(
        &mut self,
        key: K,
        command: WindowCommandId,
        decision: Decision,
        applied: DipRect,
        screen_id: i32,
        now_ms: i64,
    ) {
        self.records.insert(
            key.clone(),
            Record {
                restore: decision.restore,
                applied,
                command,
                step: decision.step,
                screen_id,
                at_ms: now_ms,
            },
        );
        self.touch(key);
    }

    pub fn forget_cycle(&mut self, key: &K) {
        if let Some(record) = self.records.get_mut(key) {
            record.step = 0;
        }
    }

    pub fn forget(&mut self, key: &K) {
        if self.records.remove(key).is_some() {
            self.order.retain(|k| k != key);
        }
    }

    fn touch(&mut self, key: K) {
        self.order.retain(|k| k != &key);
        self.order.push(key);
        while self.order.len() > self.capacity {
            if let Some(oldest) = self.order.first().cloned() {
                self.order.remove(0);
                self.records.remove(&oldest);
            } else {
                break;
            }
        }
    }
}

impl<K: Hash + Eq + Clone> Default for WindowActionMemory<K> {
    fn default() -> Self {
        Self::new()
    }
}

fn approx(a: DipRect, b: DipRect) -> bool {
    (a.x - b.x).abs() <= TOLERANCE
        && (a.y - b.y).abs() <= TOLERANCE
        && (a.w - b.w).abs() <= TOLERANCE
        && (a.h - b.h).abs() <= TOLERANCE
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::window_command::WindowCommandId;

    fn rect() -> DipRect {
        DipRect {
            x: 10.0,
            y: 10.0,
            w: 400.0,
            h: 300.0,
        }
    }

    #[test]
    fn first_sight_is_step_zero_and_cannot_restore() {
        let mem = WindowActionMemory::<u64>::new();
        let d = mem.decide(&1, WindowCommandId::LeftHalf, rect(), 0, true, 0);
        assert_eq!(d.step, 0);
        assert!(!d.can_restore);
    }

    #[test]
    fn drift_over_two_points_resets_and_refreshes_restore() {
        let mut mem = WindowActionMemory::<u64>::new();
        let start = rect();
        let d0 = mem.decide(&1, WindowCommandId::LeftHalf, start, 0, true, 0);
        mem.commit(1, WindowCommandId::LeftHalf, d0, start, 0, 0);
        let moved = DipRect {
            x: start.x + 5.0,
            y: start.y,
            w: start.w,
            h: start.h,
        };
        let d = mem.decide(&1, WindowCommandId::LeftHalf, moved, 0, true, 10);
        assert_eq!(d.step, 0);
        assert_eq!(d.restore, moved);
        assert!(d.can_restore);
    }

    #[test]
    fn same_half_cycles_when_enabled() {
        let mut mem = WindowActionMemory::<u64>::new();
        let start = rect();
        let d0 = mem.decide(&1, WindowCommandId::LeftHalf, start, 0, true, 0);
        mem.commit(1, WindowCommandId::LeftHalf, d0, start, 0, 0);
        let d1 = mem.decide(&1, WindowCommandId::LeftHalf, start, 0, true, 1);
        assert_eq!(d1.step, 1);
        assert_eq!(d1.restore, start);
    }

    #[test]
    fn restore_is_single_level() {
        let mut mem = WindowActionMemory::<u64>::new();
        let start = rect();
        let d0 = mem.decide(&1, WindowCommandId::LeftHalf, start, 0, true, 0);
        mem.commit(1, WindowCommandId::LeftHalf, d0, start, 0, 0);
        let mid = DipRect {
            x: 0.0,
            y: 0.0,
            w: 500.0,
            h: 800.0,
        };
        let d1 = mem.decide(&1, WindowCommandId::Maximize, start, 0, true, 1);
        mem.commit(1, WindowCommandId::Maximize, d1, mid, 0, 1);
        let d2 = mem.decide(&1, WindowCommandId::Restore, mid, 0, true, 2);
        assert_eq!(d2.restore, start);
        assert!(d2.can_restore);
    }
}
