//! The Adjust panel's Stats section in the App: the preview's frame figures
//! once a second, the GPU, and the last calibration's confidence.

use makepad_widgets::*;
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

impl App {
    /// The last second's frame figures.
    pub(crate) fn show_stats(&mut self, cx: &mut Cx, stats: &Stats) {
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
    }
}
