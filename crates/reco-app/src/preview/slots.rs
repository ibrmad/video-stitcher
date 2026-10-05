//! Slot bookkeeping for the zero-copy ring. Makepad and the worker use
//! different Metal queues with no fence between them, so a slot is written
//! only when the UI has handed it back, and the UI hands a slot back only
//! [`RETIRE_BEATS`] display beats after it stopped showing it (frames still
//! in flight may sample it until then).

/// Textures in the ring.
pub const RING_SLOTS: usize = 6;
/// Display beats a slot waits after being replaced before it is reused.
pub const RETIRE_BEATS: u32 = 3;

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

/// The UI's view: the slot on screen, and slots counting down their beats.
#[derive(Clone, Debug, Default)]
pub struct Retirement {
    shown: Option<usize>,
    retiring: Vec<(usize, u32)>,
}

impl Retirement {
    /// Show `slot`; the slot shown before starts retiring.
    pub fn show(&mut self, slot: usize) {
        if let Some(old) = self.shown.replace(slot)
            && old != slot
        {
            self.retiring.push((old, RETIRE_BEATS));
        }
    }

    /// A display beat passed: slots retired long enough to hand back.
    pub fn beat(&mut self) -> Vec<usize> {
        for (_, beats) in &mut self.retiring {
            *beats = beats.saturating_sub(1);
        }
        let done: Vec<usize> = self
            .retiring
            .iter()
            .filter(|(_, b)| *b == 0)
            .map(|(s, _)| *s)
            .collect();
        self.retiring.retain(|(_, b)| *b > 0);
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
        let mut r = Retirement::default();
        r.show(0);
        r.show(1);
        assert_eq!(r.shown(), Some(1));
        assert!(r.beat().is_empty());
        assert!(r.beat().is_empty());
        assert_eq!(r.beat(), vec![0]);
        assert!(r.is_idle());
    }

    #[test]
    fn showing_the_same_slot_again_retires_nothing() {
        let mut r = Retirement::default();
        r.show(2);
        r.show(2);
        assert!(r.is_idle());
    }

    #[test]
    fn clear_forgets_everything() {
        let mut r = Retirement::default();
        r.show(0);
        r.show(1);
        r.clear();
        assert_eq!(r.shown(), None);
        assert!(r.is_idle());
        assert!(r.beat().is_empty());
    }
}
