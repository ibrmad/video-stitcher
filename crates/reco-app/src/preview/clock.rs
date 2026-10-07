//! Drift-free frame timing: the target frame is computed from the
//! wall-clock time since play started, so a late tick is caught up on the
//! next one instead of compounding.

use std::time::{Duration, Instant};

/// When the next frame is due, from an anchor set on the first check after
/// play starts, resumes or seeks.
#[derive(Clone, Debug)]
pub struct FrameClock {
    frame_duration: Duration,
    anchor: Option<(Instant, u64)>,
}

impl FrameClock {
    /// A clock for frames `frame_duration` apart (a zero duration means
    /// 30 fps).
    pub fn new(frame_duration: Duration) -> Self {
        let frame_duration = if frame_duration.is_zero() {
            Duration::from_secs_f64(1.0 / 30.0)
        } else {
            frame_duration
        };
        Self {
            frame_duration,
            anchor: None,
        }
    }

    /// Forget the anchor: the next check re-anchors at the current frame.
    pub fn reset(&mut self) {
        self.anchor = None;
    }

    /// Whether `frame` is behind the schedule at `now`.
    pub fn due(&mut self, now: Instant, frame: u64) -> bool {
        let (start, start_frame) = *self.anchor.get_or_insert((now, frame));
        let elapsed = now.saturating_duration_since(start).as_secs_f64();
        let target = start_frame + (elapsed / self.frame_duration.as_secs_f64()) as u64;
        frame < target
    }

    /// How long until the frame after `frame` is due (zero when due now or
    /// not anchored).
    pub fn until_next(&self, now: Instant, frame: u64) -> Duration {
        let Some((start, start_frame)) = self.anchor else {
            return Duration::ZERO;
        };
        let next = frame.saturating_sub(start_frame) + 1;
        let at = start + self.frame_duration.mul_f64(next as f64);
        at.saturating_duration_since(now)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FRAME: Duration = Duration::from_millis(40);

    #[test]
    fn first_check_anchors_and_waits_a_frame() {
        let t0 = Instant::now();
        let mut clock = FrameClock::new(FRAME);
        assert!(!clock.due(t0, 1));
        assert!(!clock.due(t0 + Duration::from_millis(39), 1));
        assert!(clock.due(t0 + Duration::from_millis(41), 1));
    }

    #[test]
    fn a_late_tick_catches_up_without_drift() {
        let t0 = Instant::now();
        let mut clock = FrameClock::new(FRAME);
        clock.due(t0, 10);
        // 130 ms later the schedule is at frame 13; frame 11 is due, and so
        // is frame 12 at the same instant.
        let later = t0 + Duration::from_millis(130);
        assert!(clock.due(later, 11));
        assert!(clock.due(later, 12));
        assert!(!clock.due(later, 13));
    }

    #[test]
    fn reset_re_anchors() {
        let t0 = Instant::now();
        let mut clock = FrameClock::new(FRAME);
        clock.due(t0, 0);
        clock.reset();
        let later = t0 + Duration::from_secs(5);
        assert!(!clock.due(later, 7));
    }

    #[test]
    fn until_next_counts_down() {
        let t0 = Instant::now();
        let mut clock = FrameClock::new(FRAME);
        clock.due(t0, 0);
        assert_eq!(
            clock.until_next(t0 + Duration::from_millis(10), 0),
            Duration::from_millis(30)
        );
        assert_eq!(
            clock.until_next(t0 + Duration::from_millis(50), 0),
            Duration::ZERO
        );
    }

    #[test]
    fn a_zero_duration_means_30_fps() {
        let t0 = Instant::now();
        let mut clock = FrameClock::new(Duration::ZERO);
        clock.due(t0, 0);
        assert!(!clock.due(t0 + Duration::from_millis(30), 0));
        assert!(clock.due(t0 + Duration::from_millis(34), 0));
    }
}
