//! How long the UI thread spends drawing a frame (DESIGN.md Rule 8: under
//! 4 ms), summarised every two seconds for `--perf-log`; and how long after
//! its launch the first window frame came (Rule 8: under 1 s).

use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// Set by the checks to the launch time, in seconds since the Unix epoch:
/// the first frame then logs how long after it came.
pub const LAUNCHED_AT: &str = "RECO_DESKTOP_LAUNCHED_AT";

/// How long after `launched_at` (seconds since the Unix epoch) `now` is;
/// `None` for text that isn't a time, or a launch after `now`.
pub fn since_launch(launched_at: &str, now: SystemTime) -> Option<Duration> {
    let secs: f64 = launched_at.trim().parse().ok()?;
    if !secs.is_finite() || secs < 0.0 {
        return None;
    }
    now.duration_since(UNIX_EPOCH + Duration::from_secs_f64(secs))
        .ok()
}

/// How often a summary is logged.
pub const WINDOW: Duration = Duration::from_secs(2);

/// Draw times since the last summary.
#[derive(Debug, Default)]
pub struct DrawStats {
    since: Option<Instant>,
    frames: u32,
    total: Duration,
    max: Duration,
}

impl DrawStats {
    /// Count one draw that took `spent`; a summary line once a window has
    /// passed.
    pub fn add(&mut self, spent: Duration, now: Instant) -> Option<String> {
        let since = *self.since.get_or_insert(now);
        self.frames += 1;
        self.total += spent;
        self.max = self.max.max(spent);
        if now.duration_since(since) < WINDOW {
            return None;
        }
        let avg = self.total.as_secs_f64() * 1000.0 / f64::from(self.frames);
        let line = format!(
            "ui draw: {} frames, avg {avg:.2} ms, max {:.2} ms",
            self.frames,
            self.max.as_secs_f64() * 1000.0
        );
        *self = Self {
            since: Some(now),
            ..Self::default()
        };
        Some(line)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summarises_once_a_window() {
        let t0 = Instant::now();
        let mut stats = DrawStats::default();
        assert_eq!(stats.add(Duration::from_millis(2), t0), None);
        assert_eq!(
            stats.add(Duration::from_millis(4), t0 + Duration::from_secs(1)),
            None
        );
        let line = stats
            .add(Duration::from_millis(3), t0 + WINDOW)
            .expect("a summary");
        assert_eq!(line, "ui draw: 3 frames, avg 3.00 ms, max 4.00 ms");
        assert_eq!(
            stats.add(
                Duration::from_millis(1),
                t0 + WINDOW + Duration::from_millis(10)
            ),
            None
        );
    }

    #[test]
    fn the_first_frame_counts_from_the_launch() {
        let launch = UNIX_EPOCH + Duration::from_millis(1_000_000_500);
        let now = launch + Duration::from_millis(420);
        let after = Some(Duration::from_millis(420));
        assert_eq!(since_launch("1000000.5", now), after);
        assert_eq!(since_launch(" 1000000.5\n", now), after);
        assert_eq!(since_launch("soon", now), None);
        assert_eq!(since_launch("NaN", now), None);
        assert_eq!(since_launch("-1", now), None);
        assert_eq!(since_launch("1000001", now), None, "a launch after now");
    }
}
