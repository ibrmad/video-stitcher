//! The Adjust panel's Stats section in the App: the preview's frame figures
//! once a second, the GPU, the last calibration's confidence, and how AI
//! tracking did in the last export that measured it.

use makepad_widgets::*;
use reco_app::export::AiFigures;
use reco_app::preview::stats::Stats;

use crate::export_text::grouped;
use crate::App;

/// "29.9 fps".
pub(crate) fn fps_line(fps: f64) -> String {
    format!("{fps:.1} fps")
}

/// "2.1 ms".
pub(crate) fn ms_line(ms: f64) -> String {
    format!("{ms:.1} ms")
}

/// "82% confidence · 1,234 matches".
pub(crate) fn calibration_line(confidence: f64, matches: usize) -> String {
    format!(
        "{:.0}% confidence · {} matches",
        confidence * 100.0,
        grouped(matches as u64)
    )
}

/// A figure with its tenths while it is small: "1.3", "57".
fn short_figure(value: f64) -> String {
    if value >= 10.0 {
        format!("{value:.0}")
    } else {
        format!("{value:.1}")
    }
}

/// "8.5 ms · 1.3 a frame": the detector's time and what it found.
pub(crate) fn detection_line(ms: f64, per_frame: f64) -> String {
    format!(
        "{} ms · {} a frame",
        short_figure(ms),
        short_figure(per_frame)
    )
}

/// "9 tracked · ball 62%": players tracked, and how often the ball was
/// found.
pub(crate) fn tracking_line(tracks: u32, ball_pct: f64) -> String {
    format!("{tracks} tracked · ball {ball_pct:.0}%")
}

impl App {
    /// A tracked export's detector and tracker figures.
    pub(crate) fn show_ai_figures(&mut self, cx: &mut Cx, figures: &AiFigures) {
        self.set_label(
            cx,
            ids!(stats_detection),
            &detection_line(figures.detection_ms, figures.per_frame),
        );
        self.set_label(
            cx,
            ids!(stats_tracking),
            &tracking_line(figures.tracks, figures.ball_pct),
        );
        self.set_visible(cx, ids!(stats_ai), true);
    }

    /// The last second's frame figures.
    pub(crate) fn show_stats(&mut self, cx: &mut Cx, stats: &Stats) {
        self.last_stats = Some(stats.clone());
        self.set_label(cx, ids!(stats_fps), &fps_line(stats.fps));
        self.set_label(cx, ids!(stats_frame), &ms_line(stats.frame_ms));
        self.set_label(cx, ids!(stats_slowest), &ms_line(stats.p99_ms));
        self.set_label(cx, ids!(stats_decode), &ms_line(stats.decode_ms));
        self.set_label(cx, ids!(stats_render), &ms_line(stats.render_ms));
    }

    /// The GPU the preview runs on.
    pub(crate) fn show_gpu(&mut self, cx: &mut Cx, gpu: &str) {
        self.set_label(cx, ids!(stats_gpu), gpu);
    }

    /// The last calibration run's confidence and matches.
    pub(crate) fn show_calibration_stats(&mut self, cx: &mut Cx, confidence: f64, matches: usize) {
        self.last_calibration_run = Some((confidence, matches));
        self.set_label(
            cx,
            ids!(stats_calibration),
            &calibration_line(confidence, matches),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn figures_read_plainly() {
        assert_eq!(fps_line(29.94), "29.9 fps");
        assert_eq!(ms_line(2.06), "2.1 ms");
        assert_eq!(
            calibration_line(0.823, 1234),
            "82% confidence · 1,234 matches"
        );
        assert_eq!(detection_line(8.46, 1.33), "8.5 ms · 1.3 a frame");
        // Figures from 10 up drop their tenths, so the line fits the panel.
        assert_eq!(detection_line(380.42, 57.2), "380 ms · 57 a frame");
        assert_eq!(tracking_line(9, 61.6), "9 tracked · ball 62%");
    }
}
