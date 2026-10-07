//! The report Report a bug sends or copies: what went wrong, how to reach
//! the person, and, when they agree, what helps find it (the version, the
//! system, the open files' names, the preview's figures, the last
//! calibration run, the log's newest lines). Markdown; the home folder
//! reads as `~`, so paths name no one.

use std::path::Path;

use crate::preview::stats::Stats;

/// Log lines a report carries.
pub const LOG_LINES: usize = 200;

/// What the app knows that helps find a bug.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SystemFacts {
    /// "0.5.4 (2fba497)".
    pub version: String,
    /// "macos aarch64".
    pub os: String,
    /// The GPU the preview runs on and its backend, once it started.
    pub gpu: Option<String>,
    /// Where AI tracking runs (`ai::capability_line`).
    pub ai: String,
    /// The left camera's file names, in play order.
    pub left: Vec<String>,
    /// The right camera's file names.
    pub right: Vec<String>,
    /// The calibration file's name.
    pub calibration: Option<String>,
    /// The preview's last figures.
    pub stats: Option<Stats>,
    /// The last calibration run: its confidence (0 to 1) and matches.
    pub calibration_run: Option<(f64, usize)>,
    /// The log's newest lines, oldest first.
    pub log: Vec<String>,
}

/// The report. `facts` is `None` when the person left "Include system info
/// and logs" off; `home` (the home folder) reads as `~`.
pub fn compose(
    description: &str,
    contact: &str,
    facts: Option<&SystemFacts>,
    home: Option<&Path>,
) -> String {
    let contact = match contact.trim() {
        "" => "(not provided)",
        given => given,
    };
    let mut report = format!("## User description\n{description}\n\n## Contact\n{contact}\n");
    if let Some(facts) = facts {
        add_facts(&mut report, facts);
    }
    match home {
        Some(home) => with_tilde(&report, home),
        None => report,
    }
}

fn add_facts(report: &mut String, facts: &SystemFacts) {
    let gpu = facts.gpu.as_deref().unwrap_or("not started");
    report.push_str(&format!(
        "\n## Environment\n- Reco {}\n- OS: {}\n- GPU: {gpu}\n",
        facts.version, facts.os
    ));
    if !facts.ai.is_empty() {
        report.push_str(&format!("- {}\n", facts.ai));
    }
    if !facts.left.is_empty() || !facts.right.is_empty() || facts.calibration.is_some() {
        report.push_str("\n## Files\n");
        for (camera, files) in [("Left", &facts.left), ("Right", &facts.right)] {
            if !files.is_empty() {
                report.push_str(&format!("- {camera}: {}\n", files.join(", ")));
            }
        }
        if let Some(calibration) = &facts.calibration {
            report.push_str(&format!("- Calibration: {calibration}\n"));
        }
    }
    if let Some(stats) = &facts.stats {
        report.push_str(&format!(
            "\n## Performance\n- FPS: {:.1}\n- Frame time: {:.1} ms (slowest 1%: {:.1} ms)\n",
            stats.fps, stats.frame_ms, stats.p99_ms
        ));
    }
    if let Some((confidence, matches)) = facts.calibration_run {
        report.push_str(&format!(
            "\n## Calibration\n- Confidence: {:.0}%\n- Matches: {matches}\n",
            confidence * 100.0
        ));
    }
    if !facts.log.is_empty() {
        // The telemetry's fitting finds the log by this header.
        report.push_str(&format!("\n## Log (last {} lines)\n```\n", facts.log.len()));
        for line in &facts.log {
            report.push_str(line);
            report.push('\n');
        }
        report.push_str("```\n");
    }
}

/// `text` with every path inside `home` starting at `~` instead.
fn with_tilde(text: &str, home: &Path) -> String {
    let home = home.to_string_lossy();
    let home = home.trim_end_matches(['/', '\\']);
    if home.is_empty() {
        return text.to_string();
    }
    let mut out = text.to_string();
    for separator in ['/', '\\'] {
        out = out.replace(&format!("{home}{separator}"), &format!("~{separator}"));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_words_and_the_contact_come_first() {
        assert_eq!(
            compose("It froze", "  ann on the forum ", None, None),
            "## User description\nIt froze\n\n## Contact\nann on the forum\n"
        );
        assert!(
            compose("x", "  ", None, None).ends_with("## Contact\n(not provided)\n"),
            "no contact"
        );
    }

    #[test]
    fn the_facts_follow_when_included() {
        let facts = SystemFacts {
            version: "0.5.4 (abc1234)".into(),
            os: "macos aarch64".into(),
            gpu: Some("Apple M1 Pro (Metal)".into()),
            ai: "AI: runs on CPU".into(),
            left: vec!["GX010120.MP4".into(), "GX020120.MP4".into()],
            right: vec!["GX010092.MP4".into()],
            calibration: Some("GX010120_calibration.json".into()),
            stats: Some(Stats {
                fps: 29.94,
                frame_ms: 6.04,
                p99_ms: 9.0,
                decode_ms: 2.0,
                render_ms: 4.04,
            }),
            calibration_run: Some((0.823, 1234)),
            log: vec!["[I] one".into(), "[E] two".into()],
        };
        assert_eq!(
            compose("It froze", "", Some(&facts), None),
            "## User description\nIt froze\n\n## Contact\n(not provided)\n\
             \n## Environment\n- Reco 0.5.4 (abc1234)\n- OS: macos aarch64\n- GPU: Apple M1 Pro (Metal)\n\
             - AI: runs on CPU\n\
             \n## Files\n- Left: GX010120.MP4, GX020120.MP4\n- Right: GX010092.MP4\n\
             - Calibration: GX010120_calibration.json\n\
             \n## Performance\n- FPS: 29.9\n- Frame time: 6.0 ms (slowest 1%: 9.0 ms)\n\
             \n## Calibration\n- Confidence: 82%\n- Matches: 1234\n\
             \n## Log (last 2 lines)\n```\n[I] one\n[E] two\n```\n"
        );
    }

    #[test]
    fn unknown_facts_are_left_out() {
        let facts = SystemFacts {
            version: "0.5.4".into(),
            os: "linux x86_64".into(),
            ..SystemFacts::default()
        };
        let report = compose("x", "", Some(&facts), None);
        assert!(report.contains("- GPU: not started\n"), "{report}");
        for section in ["## Files", "## Performance", "## Calibration", "## Log"] {
            assert!(!report.contains(section), "{section} in {report}");
        }
    }

    #[test]
    fn the_home_folder_reads_as_a_tilde() {
        let facts = SystemFacts {
            log: vec!["[I] export: /Users/ann/Movies/match.mp4".into()],
            ..SystemFacts::default()
        };
        let home = Path::new("/Users/ann");
        let report = compose("see /Users/ann/x.mp4", "", Some(&facts), Some(home));
        assert!(
            report.contains("~/Movies/match.mp4") && report.contains("see ~/x.mp4"),
            "{report}"
        );
        assert!(!report.contains("/Users/ann/"), "{report}");
        assert!(
            compose("/Users/anna/x", "", None, Some(home)).contains("/Users/anna/x"),
            "another person's folder is not the home"
        );
        assert!(
            compose("/a/b", "", None, Some(Path::new("/"))).contains("/a/b"),
            "a home of / changes nothing"
        );
    }
}
