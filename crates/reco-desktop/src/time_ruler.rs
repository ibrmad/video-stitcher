//! Time ruler arithmetic for the time panel: clock labels, and tick steps
//! that keep the labels apart at any window width. No Makepad types.

/// A clock label: "0:07", "12:34", "1:45:00". Minutes and seconds, with
/// hours once there are any; fractions of a second are dropped.
pub fn clock(seconds: f64) -> String {
    let total = if seconds.is_finite() && seconds > 0.0 {
        seconds.floor() as u64
    } else {
        0
    };
    let (h, m, s) = (total / 3600, total / 60 % 60, total % 60);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}

/// Steps a ruler may use, in seconds; past the last, whole days.
const LADDER: [f64; 18] = [
    1.0, 2.0, 5.0, 10.0, 15.0, 30.0, 60.0, 120.0, 300.0, 600.0, 900.0, 1800.0, 3600.0, 7200.0,
    10_800.0, 21_600.0, 43_200.0, 86_400.0,
];

/// The smallest step from the ruler's ladder (1, 2, 5, 10, 15, 30 s; 1, 2,
/// 5, 10, 15, 30 min; 1, 2, 3, 6, 12, 24 h) whose ticks land at least
/// `min_gap_px` apart when `duration` seconds span `width_px`.
pub fn tick_step(duration: f64, width_px: f64, min_gap_px: f64) -> f64 {
    if !(duration > 0.0 && width_px > 0.0) {
        return LADDER[0];
    }
    let needed = min_gap_px * duration / width_px;
    LADDER
        .iter()
        .copied()
        .find(|&step| step >= needed)
        .unwrap_or_else(|| (needed / 86_400.0).ceil() * 86_400.0)
}

/// Tick times from 0 up to and including `duration`, `tick_step` apart.
pub fn ticks(duration: f64, width_px: f64, min_gap_px: f64) -> Vec<f64> {
    let step = tick_step(duration, width_px, min_gap_px);
    let count = if duration > 0.0 {
        (duration / step + 1e-9).floor() as usize
    } else {
        0
    };
    (0..=count).map(|i| i as f64 * step).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clock_shows_minutes_and_seconds() {
        assert_eq!(clock(0.0), "0:00");
        assert_eq!(clock(7.9), "0:07");
        assert_eq!(clock(754.0), "12:34");
        assert_eq!(clock(3599.6), "59:59");
    }

    #[test]
    fn clock_adds_hours_once_there_are_any() {
        assert_eq!(clock(3600.0), "1:00:00");
        assert_eq!(clock(6300.0), "1:45:00");
        assert_eq!(clock(36_061.0), "10:01:01");
    }

    #[test]
    fn clock_treats_negative_and_invalid_times_as_zero() {
        assert_eq!(clock(-3.0), "0:00");
        assert_eq!(clock(f64::NAN), "0:00");
    }

    #[test]
    fn tick_step_keeps_labels_apart() {
        // A 1:45:00 match over 600 px: 60 px needs 630 s, so 15 min.
        assert_eq!(tick_step(6300.0, 600.0, 60.0), 900.0);
        // One minute over 600 px: 60 px is 6 s, so 10 s.
        assert_eq!(tick_step(60.0, 600.0, 60.0), 10.0);
        // Room to spare picks the finest step.
        assert_eq!(tick_step(10.0, 1000.0, 60.0), 1.0);
    }

    #[test]
    fn tick_step_grows_past_the_ladder_in_whole_days() {
        assert_eq!(tick_step(30.0 * 86_400.0, 100.0, 60.0), 18.0 * 86_400.0);
    }

    #[test]
    fn tick_step_survives_empty_input() {
        assert_eq!(tick_step(0.0, 600.0, 60.0), 1.0);
        assert_eq!(tick_step(6300.0, 0.0, 60.0), 1.0);
    }

    #[test]
    fn ticks_run_from_zero_to_the_end() {
        assert_eq!(
            ticks(6300.0, 600.0, 60.0),
            vec![0.0, 900.0, 1800.0, 2700.0, 3600.0, 4500.0, 5400.0, 6300.0]
        );
        assert_eq!(ticks(0.0, 600.0, 60.0), vec![0.0]);
    }
}
