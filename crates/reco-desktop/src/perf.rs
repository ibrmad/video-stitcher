//! How long the UI thread spends drawing a frame (DESIGN.md Rule 8: under
//! 4 ms), summarised every two seconds for `--perf-log`.

use std::time::{Duration, Instant};

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
}
