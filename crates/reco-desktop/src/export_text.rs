//! The words the export sheet and the export card show.

/// `n` with thousands separated: "48,210".
pub fn grouped(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, digit) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(digit);
    }
    out
}

/// A size in the sheet's list: "1080p · 1920 × 1080".
pub fn size_label(name: &str, width: u32, height: u32) -> String {
    format!("{name} · {width} × {height}")
}

/// The card's line before the first frame: where the export starts (one
/// far in seeks first, and a bare "Starting…" would look stalled).
pub fn starting_line(start_secs: f64) -> String {
    if start_secs < 1.0 {
        "Starting…".into()
    } else {
        format!("Starting at {}…", crate::time_ruler::clock(start_secs))
    }
}

/// The card's progress line: "Frame 48,210 of 141,000 · 62 fps" (no rate
/// before there is one).
pub fn progress_detail(frames: u64, total: u64, fps: f64) -> String {
    let done = format!("Frame {} of {}", grouped(frames), grouped(total));
    if fps > 0.0 {
        format!("{done} · {fps:.0} fps")
    } else {
        done
    }
}

/// How long is left at `fps`: "About 25 min left", "About 40 s left", or
/// nothing before there is a rate.
pub fn time_left(frames: u64, total: u64, fps: f64) -> String {
    if fps <= 0.0 || frames >= total {
        return String::new();
    }
    let seconds = (total - frames) as f64 / fps;
    if seconds < 60.0 {
        return format!("About {} s left", seconds.round());
    }
    let minutes = (seconds / 60.0).round() as u64;
    match (minutes / 60, minutes % 60) {
        (0, m) => format!("About {m} min left"),
        (h, 0) => format!("About {h} h left"),
        (h, m) => format!("About {h} h {m} min left"),
    }
}

/// Whole percent done, 0 to 100.
pub fn percent(frames: u64, total: u64) -> u64 {
    if total == 0 {
        0
    } else {
        (frames.min(total) * 100) / total
    }
}

/// What the final notice adds about AI tracking: nothing when the export
/// didn't ask for it.
pub fn tracking_note(asked: bool, started: Option<&Result<(), String>>) -> &'static str {
    match (asked, started) {
        (false, _) => "",
        (true, Some(Ok(()))) => " · tracked with AI",
        (true, _) => " · without AI tracking",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_notice_says_whether_the_export_tracked() {
        assert_eq!(tracking_note(false, None), "");
        assert_eq!(tracking_note(true, Some(&Ok(()))), " · tracked with AI");
        assert_eq!(
            tracking_note(true, Some(&Err("no detector".into()))),
            " · without AI tracking"
        );
        assert_eq!(
            tracking_note(true, None),
            " · without AI tracking",
            "asked, never started"
        );
    }

    #[test]
    fn numbers_are_grouped_by_thousands() {
        assert_eq!(grouped(0), "0");
        assert_eq!(grouped(999), "999");
        assert_eq!(grouped(48_210), "48,210");
        assert_eq!(grouped(1_141_000), "1,141,000");
    }

    #[test]
    fn the_card_says_where_the_export_starts() {
        assert_eq!(starting_line(0.0), "Starting…");
        assert_eq!(starting_line(750.0), "Starting at 12:30…");
        assert_eq!(starting_line(4_000.0), "Starting at 1:06:40…");
    }

    #[test]
    fn sizes_show_their_pixels() {
        assert_eq!(size_label("1080p", 1920, 1080), "1080p · 1920 × 1080");
    }

    #[test]
    fn progress_reads_in_frames_and_time() {
        assert_eq!(
            progress_detail(48_210, 141_000, 62.4),
            "Frame 48,210 of 141,000 · 62 fps"
        );
        assert_eq!(progress_detail(1, 60, 0.0), "Frame 1 of 60");
        assert_eq!(time_left(48_210, 141_000, 62.0), "About 25 min left");
        assert_eq!(time_left(100, 1_300, 30.0), "About 40 s left");
        assert_eq!(time_left(10, 60, 0.0), "", "no rate yet");
        assert_eq!(time_left(60, 60, 30.0), "", "done");
        assert_eq!(percent(34, 100), 34);
        assert_eq!(percent(5, 0), 0);
        assert_eq!(percent(150, 100), 100);
    }
}
