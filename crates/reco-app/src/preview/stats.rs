//! The preview's figures for the Adjust panel's Stats section: frames shown
//! a second, frame time (average and the slowest 1%), and its decode and
//! render parts, reported once a second while frames are shown.

use std::time::{Duration, Instant};

/// One second's figures.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Stats {
    /// Frames shown a second.
    pub fps: f64,
    /// Average time a frame took, ms.
    pub frame_ms: f64,
    /// The 99th percentile frame time, ms.
    pub p99_ms: f64,
    /// Average decode time, ms.
    pub decode_ms: f64,
    /// Average render time (stitch and hand-over), ms.
    pub render_ms: f64,
}

/// Collects frame times and reports them once a second.
#[derive(Debug)]
pub struct StatsMeter {
    since: Instant,
    frames: Vec<(Duration, Duration)>,
}

impl StatsMeter {
    /// A meter starting at `now`.
    pub fn new(now: Instant) -> Self {
        Self {
            since: now,
            frames: Vec::new(),
        }
    }

    /// A frame was shown at `now`: it waited `decode` for its frames and
    /// took `render` to draw.
    pub fn frame(&mut self, now: Instant, decode: Duration, render: Duration) {
        // A second starts at its first frame, not in the pause before it.
        if self.frames.is_empty() {
            self.since = now;
        }
        self.frames.push((decode, render));
    }

    /// The last second's figures, once a second has passed with frames in
    /// it (then it starts over).
    pub fn report(&mut self, now: Instant) -> Option<Stats> {
        let elapsed = now.saturating_duration_since(self.since);
        if elapsed < Duration::from_secs(1) {
            return None;
        }
        // A quiet second (paused) starts the next one afresh.
        self.since = now;
        let frames = std::mem::take(&mut self.frames);
        if frames.is_empty() {
            return None;
        }
        let ms = |d: Duration| d.as_secs_f64() * 1000.0;
        let count = frames.len() as f64;
        let decode_ms = frames.iter().map(|(d, _)| ms(*d)).sum::<f64>() / count;
        let render_ms = frames.iter().map(|(_, r)| ms(*r)).sum::<f64>() / count;
        let mut totals: Vec<f64> = frames.iter().map(|(d, r)| ms(*d) + ms(*r)).collect();
        totals.sort_by(f64::total_cmp);
        // The value 99% of frames stay at or under.
        let at = ((totals.len() as f64 * 0.99).ceil() as usize).clamp(1, totals.len()) - 1;
        Some(Stats {
            fps: count / elapsed.as_secs_f64(),
            frame_ms: decode_ms + render_ms,
            p99_ms: totals[at],
            decode_ms,
            render_ms,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    #[test]
    fn a_second_of_frames_is_reported() {
        let start = Instant::now();
        let mut meter = StatsMeter::new(start);
        for i in 0..30 {
            // One slow frame in thirty.
            let render = if i == 7 { ms(30) } else { ms(4) };
            meter.frame(start, ms(2), render);
        }
        assert_eq!(meter.report(start + ms(500)), None, "not a second yet");
        let stats = meter.report(start + ms(1000)).expect("a second");
        assert!((stats.fps - 30.0).abs() < 1e-9);
        assert!((stats.decode_ms - 2.0).abs() < 1e-9);
        assert!((stats.render_ms - (29.0 * 4.0 + 30.0) / 30.0).abs() < 1e-9);
        assert!((stats.frame_ms - (stats.decode_ms + stats.render_ms)).abs() < 1e-9);
        assert!(
            (stats.p99_ms - 32.0).abs() < 1e-9,
            "the slow frame: {}",
            stats.p99_ms
        );
        assert_eq!(
            meter.report(start + ms(2500)),
            None,
            "it starts over; no frames since"
        );
    }

    #[test]
    fn a_second_starts_at_its_first_frame() {
        let start = Instant::now();
        let mut meter = StatsMeter::new(start);
        // Paused for two seconds, then thirty frames over a second.
        let playing = start + ms(2000);
        for i in 0..30 {
            meter.frame(playing + ms(i * 33), ms(1), ms(4));
        }
        let stats = meter
            .report(playing + ms(1000))
            .expect("a second of frames");
        assert!(
            (stats.fps - 30.0).abs() < 1e-9,
            "not diluted by the pause: {}",
            stats.fps
        );
    }
}
