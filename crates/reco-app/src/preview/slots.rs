//! Slot bookkeeping for the zero-copy ring. Makepad and the worker use
//! different Metal queues with no fence between them, so a slot is written
//! only when the UI has handed it back, and the UI hands a slot back only
//! [`RETIRE_BEATS`] display beats and at least [`RETIRE_MIN`] after it
//! stopped showing it (frames still in flight may sample it until then).

use std::time::{Duration, Instant};

/// Textures in the ring.
pub const RING_SLOTS: usize = 6;
/// Display beats a slot waits after being replaced before it is reused.
pub const RETIRE_BEATS: u32 = 3;
/// The least time a replaced slot waits too: beats keep coming while Makepad
/// skips paints with three frames already in flight, and 50 ms covers three
/// 60 Hz frames.
pub const RETIRE_MIN: Duration = Duration::from_millis(50);

/// The worker's view of the ring: which slots it may render into.
#[derive(Clone, Debug)]
pub struct SlotRing {
    free: Vec<bool>,
}

impl SlotRing {
    /// A ring of `len` free slots.
    pub fn new(len: usize) -> Self {
        Self {
            free: vec![true; len],
        }
    }

    /// Take a free slot to render into.
    pub fn acquire(&mut self) -> Option<usize> {
        let slot = self.free.iter().position(|f| *f)?;
        self.free[slot] = false;
        Some(slot)
    }

    /// The UI handed `slot` back.
    pub fn release(&mut self, slot: usize) {
        if let Some(f) = self.free.get_mut(slot) {
            *f = true;
        }
    }

    /// Slots free to render into.
    pub fn free_count(&self) -> usize {
        self.free.iter().filter(|f| **f).count()
    }
}

/// The UI's view: the slot on screen, and slots counting down their beats
/// (each with the time it was replaced).
#[derive(Clone, Debug, Default)]
pub struct Retirement {
    shown: Option<usize>,
    retiring: Vec<(usize, u32, Instant)>,
}

impl Retirement {
    /// Show `slot` from `now`; the slot shown before starts retiring.
    pub fn show(&mut self, slot: usize, now: Instant) {
        if let Some(old) = self.shown.replace(slot)
            && old != slot
        {
            self.retiring.push((old, RETIRE_BEATS, now));
        }
    }

    /// A display beat passed at `now`: slots retired long enough, in beats
    /// and in time, to hand back.
    pub fn beat(&mut self, now: Instant) -> Vec<usize> {
        for (_, beats, _) in &mut self.retiring {
            *beats = beats.saturating_sub(1);
        }
        let ready = |&(_, beats, since): &(usize, u32, Instant)| {
            beats == 0 && now.saturating_duration_since(since) >= RETIRE_MIN
        };
        let done: Vec<usize> = self
            .retiring
            .iter()
            .filter(|r| ready(r))
            .map(|r| r.0)
            .collect();
        self.retiring.retain(|r| !ready(r));
        done
    }

    /// The slot on screen.
    pub fn shown(&self) -> Option<usize> {
        self.shown
    }

    /// Whether nothing is retiring (no more beats needed).
    pub fn is_idle(&self) -> bool {
        self.retiring.is_empty()
    }

    /// A new ring replaced the old one: forget every slot.
    pub fn clear(&mut self) {
        self.shown = None;
        self.retiring.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ring_hands_out_each_slot_once() {
        let mut ring = SlotRing::new(3);
        let mut got = vec![
            ring.acquire().unwrap(),
            ring.acquire().unwrap(),
            ring.acquire().unwrap(),
        ];
        got.sort();
        assert_eq!(got, vec![0, 1, 2]);
        assert_eq!(ring.acquire(), None);
        ring.release(1);
        assert_eq!(ring.free_count(), 1);
        assert_eq!(ring.acquire(), Some(1));
    }

    #[test]
    fn releasing_an_unknown_slot_is_ignored() {
        let mut ring = SlotRing::new(2);
        ring.release(7);
        assert_eq!(ring.free_count(), 2);
    }

    #[test]
    fn a_replaced_slot_retires_after_its_beats() {
        let t0 = Instant::now();
        let late = t0 + RETIRE_MIN;
        let mut r = Retirement::default();
        r.show(0, t0);
        r.show(1, t0);
        assert_eq!(r.shown(), Some(1));
        assert!(r.beat(late).is_empty());
        assert!(r.beat(late).is_empty());
        assert_eq!(r.beat(late), vec![0]);
        assert!(r.is_idle());
    }

    #[test]
    fn a_slot_also_waits_its_minimum_time() {
        let t0 = Instant::now();
        let mut r = Retirement::default();
        r.show(0, t0);
        r.show(1, t0);
        for _ in 0..RETIRE_BEATS {
            assert!(r.beat(t0 + Duration::from_millis(5)).is_empty());
        }
        assert!(r.beat(t0 + Duration::from_millis(10)).is_empty());
        assert_eq!(r.beat(t0 + RETIRE_MIN), vec![0]);
    }

    #[test]
    fn showing_the_same_slot_again_retires_nothing() {
        let t0 = Instant::now();
        let mut r = Retirement::default();
        r.show(2, t0);
        r.show(2, t0);
        assert!(r.is_idle());
    }

    #[test]
    fn clear_forgets_everything() {
        let t0 = Instant::now();
        let mut r = Retirement::default();
        r.show(0, t0);
        r.show(1, t0);
        r.clear();
        assert_eq!(r.shown(), None);
        assert!(r.is_idle());
        assert!(r.beat(t0 + RETIRE_MIN).is_empty());
    }
}
