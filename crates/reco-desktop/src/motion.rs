//! A panel's slide: a value eased from one size to another over a fixed time.
//! No Makepad types.

/// How long a panel takes to slide open or closed.
pub const PANEL_SECS: f64 = 0.2;

/// Ease-out cubic: quick to start, settling at the end.
pub fn ease_out(t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t).powi(3)
}

/// A value moving from `from` to `to`, starting at `start` (seconds).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Motion {
    pub from: f64,
    pub to: f64,
    pub start: f64,
    pub duration: f64,
}

impl Motion {
    /// A panel's slide from `from` to `to`, starting `now`.
    pub fn panel(from: f64, to: f64, now: f64) -> Self {
        Self {
            from,
            to,
            start: now,
            duration: PANEL_SECS,
        }
    }

    /// The value at `now`: `from` until it starts, `to` once it's over.
    pub fn value(&self, now: f64) -> f64 {
        if self.done(now) {
            return self.to;
        }
        let t = (now - self.start) / self.duration;
        self.from + (self.to - self.from) * ease_out(t)
    }

    /// Whether the motion is over at `now`.
    pub fn done(&self, now: f64) -> bool {
        now >= self.start + self.duration
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn easing_runs_from_zero_to_one_and_settles() {
        assert_eq!(ease_out(0.0), 0.0);
        assert_eq!(ease_out(1.0), 1.0);
        assert!(ease_out(0.5) > 0.5, "most of the way by half time");
        assert!(ease_out(0.9) > 0.99 - 0.01, "nearly there near the end");
        assert_eq!(ease_out(-1.0), 0.0, "before the start");
        assert_eq!(ease_out(2.0), 1.0, "after the end");
    }

    #[test]
    fn a_panel_slides_in_its_time() {
        let closing = Motion::panel(290.0, 0.0, 10.0);
        assert_eq!(closing.value(10.0), 290.0);
        assert!(!closing.done(10.0));
        let mid = closing.value(10.0 + PANEL_SECS / 2.0);
        assert!(
            mid > 0.0 && mid < 145.0,
            "past halfway at half time ({mid})"
        );
        assert_eq!(closing.value(10.0 + PANEL_SECS), 0.0);
        assert!(closing.done(10.0 + PANEL_SECS));
        assert_eq!(closing.value(99.0), 0.0, "and stays there");
    }

    #[test]
    fn opening_runs_the_other_way() {
        let opening = Motion::panel(0.0, 310.0, 0.0);
        let early = opening.value(0.05);
        let later = opening.value(0.1);
        assert!(0.0 < early && early < later && later < 310.0);
        assert_eq!(opening.value(PANEL_SECS), 310.0);
    }

    #[test]
    fn a_clock_from_before_the_start_holds_the_start() {
        let m = Motion::panel(100.0, 0.0, 5.0);
        assert_eq!(m.value(4.0), 100.0);
        assert!(!m.done(4.0));
    }
}
