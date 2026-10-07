//! Anonymous usage data, opt-in in Preferences: the events and their JSON
//! for the reco-telemetry service. No names, no paths, no pictures: a
//! random client id, the app's version, and each event's figures. Pure; the
//! app does the sending.

use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};

/// Where events go.
pub const ENDPOINT: &str = "https://telemetry-ingestion-204135919265.us-central1.run.app/telemetry";
/// The app's name in every batch.
pub const APP_NAME: &str = "video-stitcher";
/// The most of a bug report an event carries. The service rejects events
/// whose props pass 128 KB once re-encoded, and escaping grows a byte at
/// most sixfold, so 16 KB always fits.
pub const MAX_REPORT_BYTES: usize = 16 * 1024;
/// The most of an error message an event carries.
pub const MAX_ERROR_BYTES: usize = 500;

/// One usage event.
#[derive(Clone, Debug, PartialEq)]
pub enum UsageEvent {
    /// The app started (or usage data was just turned on).
    AppOpen,
    /// The system: OS, GPU and the AI tracking's state.
    Context {
        /// "macos aarch64".
        os: String,
        /// The GPU's name.
        gpu: String,
        /// The AI tracking's state.
        ai: String,
    },
    /// A match opened.
    SourceInfo {
        /// Frame width.
        width: u32,
        /// Frame height.
        height: u32,
        /// Frames a second.
        fps: f64,
        /// The decoder ("hardware" or "software").
        decoder: String,
        /// The sync offset, in frames.
        sync_offset: i64,
    },
    /// Report a bug's text (fitted under [`MAX_REPORT_BYTES`]).
    BugReport {
        /// The report.
        report: String,
    },
    /// An export finished.
    ExportComplete {
        /// Frames written.
        frames: u64,
        /// Seconds it took.
        duration_secs: f64,
        /// The codec.
        codec: String,
    },
    /// An export failed.
    ExportError {
        /// Why.
        error: String,
        /// The codec.
        codec: String,
    },
    /// A calibration finished.
    CalibrationComplete {
        /// Its confidence, 0 to 1.
        confidence: f64,
        /// Matched points.
        matches: usize,
    },
    /// A calibration failed.
    CalibrationError {
        /// Why.
        error: String,
    },
}

impl UsageEvent {
    /// The event's name, as the service knows it.
    pub fn name(&self) -> &'static str {
        match self {
            Self::AppOpen => "app_open",
            Self::Context { .. } => "context",
            Self::SourceInfo { .. } => "source_info",
            Self::BugReport { .. } => "bug_report",
            Self::ExportComplete { .. } => "export_complete",
            Self::ExportError { .. } => "export_error",
            Self::CalibrationComplete { .. } => "calibration_complete",
            Self::CalibrationError { .. } => "calibration_error",
        }
    }

    fn props(&self) -> Option<Value> {
        Some(match self {
            Self::AppOpen => return None,
            Self::Context { os, gpu, ai } => json!({"os": os, "gpu": gpu, "ai": ai}),
            Self::SourceInfo {
                width,
                height,
                fps,
                decoder,
                sync_offset,
            } => json!({
                "width": width,
                "height": height,
                "fps": fps,
                "decoder": decoder,
                "sync_offset": sync_offset,
            }),
            Self::BugReport { report } => json!({"report": fit_report(report)}),
            Self::ExportComplete {
                frames,
                duration_secs,
                codec,
            } => json!({"frames": frames, "duration_sec": duration_secs, "codec": codec}),
            Self::ExportError { error, codec } => json!({
                "error_type": "export_failed",
                "error_message": truncated(error, MAX_ERROR_BYTES),
                "codec": codec,
            }),
            Self::CalibrationComplete {
                confidence,
                matches,
            } => json!({"confidence": confidence, "matches": matches}),
            Self::CalibrationError { error } => {
                json!({"error_message": truncated(error, MAX_ERROR_BYTES)})
            }
        })
    }
}

/// One event's batch, as the service takes it.
pub fn batch_json(
    event: &UsageEvent,
    client_id: &str,
    version: &str,
    at: SystemTime,
    batch_id: &str,
) -> String {
    let time = iso_time(at);
    json!({
        "schema_version": 1,
        "client_id": client_id,
        "app": {"name": APP_NAME, "version": version},
        "sent_at": time,
        "batch_id": batch_id,
        "events": [{
            "schema_version": 1,
            "ts": time,
            "name": event.name(),
            "client_id": client_id,
            "props": event.props(),
        }],
    })
    .to_string()
}

/// A new batch's id.
pub fn batch_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

/// `at` in UTC, as "2025-10-05T16:00:00.000Z".
pub fn iso_time(at: SystemTime) -> String {
    let secs = at.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
    let (year, month, day) = civil_from_days((secs / 86_400) as i64);
    format!(
        "{year}-{month:02}-{day:02}T{:02}:{:02}:{:02}.000Z",
        (secs / 3600) % 24,
        (secs / 60) % 60,
        secs % 60
    )
}

/// The date `days` after 1970-01-01: Howard Hinnant's algorithm (public
/// domain).
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe as i64 + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

/// `report` under [`MAX_REPORT_BYTES`], dropping the oldest log lines first:
/// the head (the words, the contact, the system) and the newest lines carry
/// the diagnosis. The head is cut only when it alone is too long.
pub fn fit_report(report: &str) -> String {
    if report.len() <= MAX_REPORT_BYTES {
        return report.to_string();
    }
    // The log's header as the bug report writes it ("(last " keeps a
    // "## Log" the person typed from passing for it).
    let Some((head, log)) = report.split_once("\n## Log (last ") else {
        return truncated(report, MAX_REPORT_BYTES).to_string();
    };
    // The header's rest and the opening fence come first; both are made
    // again below. The last line is the closing fence.
    let lines: Vec<&str> = log.lines().skip(2).collect();
    let header = |dropped: usize| format!("\n## Log (oldest {dropped} lines dropped)\n```\n");
    // Budgeting with the longest header never undercounts.
    let overhead = head.len() + header(lines.len()).len();
    if overhead >= MAX_REPORT_BYTES {
        return truncated(head, MAX_REPORT_BYTES).to_string();
    }
    let budget = MAX_REPORT_BYTES - overhead;
    let (mut kept, mut used) = (0, 0);
    for line in lines.iter().rev() {
        if used + line.len() + 1 > budget {
            break;
        }
        used += line.len() + 1;
        kept += 1;
    }
    let newest = &lines[lines.len() - kept..];
    let mut out = String::with_capacity(overhead + used + 4);
    out.push_str(head);
    out.push_str(&header(lines.len() - kept));
    for line in newest {
        out.push_str(line);
        out.push('\n');
    }
    // Kept lines end with the closing fence; none kept needs one.
    if newest.is_empty() {
        out.push_str("```\n");
    }
    out
}

/// At most `max` bytes of `s`, cut at a character's edge.
fn truncated(s: &str, max: usize) -> &str {
    if s.len() <= max {
        return s;
    }
    let mut end = max;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    fn at() -> SystemTime {
        UNIX_EPOCH + Duration::from_secs(1_759_680_000)
    }

    #[test]
    fn a_batch_is_the_services_json() {
        let text = batch_json(
            &UsageEvent::ExportComplete {
                frames: 1800,
                duration_secs: 60.5,
                codec: "h264".into(),
            },
            "cid",
            "0.5.4",
            at(),
            "bid",
        );
        let value: Value = serde_json::from_str(&text).expect("JSON");
        assert_eq!(
            value,
            json!({
                "schema_version": 1,
                "client_id": "cid",
                "app": {"name": "video-stitcher", "version": "0.5.4"},
                "sent_at": "2025-10-05T16:00:00.000Z",
                "batch_id": "bid",
                "events": [{
                    "schema_version": 1,
                    "ts": "2025-10-05T16:00:00.000Z",
                    "name": "export_complete",
                    "client_id": "cid",
                    "props": {"frames": 1800, "duration_sec": 60.5, "codec": "h264"}
                }]
            })
        );
    }

    fn props(event: UsageEvent) -> Value {
        let text = batch_json(&event, "c", "v", at(), "b");
        let value: Value = serde_json::from_str(&text).expect("JSON");
        value["events"][0]["props"].clone()
    }

    #[test]
    fn each_event_has_its_name_and_figures() {
        assert_eq!(UsageEvent::AppOpen.name(), "app_open");
        assert_eq!(props(UsageEvent::AppOpen), Value::Null);
        assert_eq!(
            props(UsageEvent::Context {
                os: "macos aarch64".into(),
                gpu: "Apple M1 Pro".into(),
                ai: "AI: off".into()
            }),
            json!({"os": "macos aarch64", "gpu": "Apple M1 Pro", "ai": "AI: off"})
        );
        assert_eq!(
            props(UsageEvent::SourceInfo {
                width: 5312,
                height: 2988,
                fps: 29.97,
                decoder: "hardware".into(),
                sync_offset: -12
            }),
            json!({"width": 5312, "height": 2988, "fps": 29.97, "decoder": "hardware", "sync_offset": -12})
        );
        assert_eq!(
            props(UsageEvent::CalibrationComplete {
                confidence: 0.82,
                matches: 1234
            }),
            json!({"confidence": 0.82, "matches": 1234})
        );
        assert_eq!(
            props(UsageEvent::CalibrationError {
                error: "no usable frame pairs".into()
            }),
            json!({"error_message": "no usable frame pairs"})
        );
        assert_eq!(
            props(UsageEvent::ExportError {
                error: "é".repeat(300),
                codec: "hevc".into()
            }),
            json!({"error_type": "export_failed", "error_message": "é".repeat(250), "codec": "hevc"}),
            "500 bytes at most, cut between characters"
        );
        assert_eq!(
            props(UsageEvent::BugReport {
                report: "x".repeat(20_000)
            })["report"]
                .as_str()
                .map(str::len),
            Some(MAX_REPORT_BYTES)
        );
        for (event, name) in [
            (
                UsageEvent::BugReport {
                    report: String::new(),
                },
                "bug_report",
            ),
            (
                UsageEvent::ExportError {
                    error: String::new(),
                    codec: String::new(),
                },
                "export_error",
            ),
            (
                UsageEvent::CalibrationComplete {
                    confidence: 0.0,
                    matches: 0,
                },
                "calibration_complete",
            ),
            (
                UsageEvent::CalibrationError {
                    error: String::new(),
                },
                "calibration_error",
            ),
        ] {
            assert_eq!(event.name(), name);
        }
    }

    #[test]
    fn times_read_in_utc() {
        assert_eq!(iso_time(at()), "2025-10-05T16:00:00.000Z");
        assert_eq!(iso_time(UNIX_EPOCH), "1970-01-01T00:00:00.000Z");
        assert_eq!(
            iso_time(UNIX_EPOCH + Duration::from_secs(951_782_400)),
            "2000-02-29T00:00:00.000Z",
            "a leap day"
        );
    }

    fn report_with_log(lines: usize) -> String {
        let mut r = String::from(
            "## User description\ncrash on export\n\n## Contact\nann\n\
             \n## Environment\n- Reco 0.5.4\n- OS: linux x86_64\n",
        );
        r.push_str(&format!("\n## Log (last {lines} lines)\n```\n"));
        for i in 0..lines {
            r.push_str(&format!(
                "[I] log line number {i} padded with enough detail to take up realistic space\n"
            ));
        }
        r.push_str("```\n");
        r
    }

    #[test]
    fn a_small_report_is_unchanged() {
        let report = report_with_log(3);
        assert_eq!(fit_report(&report), report);
    }

    #[test]
    fn a_long_report_keeps_its_head_and_newest_lines() {
        let fitted = fit_report(&report_with_log(400));
        assert!(fitted.len() <= MAX_REPORT_BYTES);
        assert!(fitted.contains("## User description") && fitted.contains("ann"));
        assert!(fitted.contains("log line number 399"));
        assert!(!fitted.contains("log line number 0 "));
        assert!(fitted.contains("lines dropped"));
        assert!(fitted.ends_with("```\n"));
    }

    #[test]
    fn a_long_report_without_a_log_is_cut() {
        let fitted = fit_report(&"x".repeat(20_000));
        assert_eq!(fitted.len(), MAX_REPORT_BYTES);
        let head = format!(
            "{}\n## Log (last 1 lines)\n```\nline\n```\n",
            "y".repeat(20_000)
        );
        let fitted = fit_report(&head);
        assert!(fitted.len() <= MAX_REPORT_BYTES && fitted.starts_with("yyy"));
    }

    #[test]
    fn cutting_never_splits_a_character() {
        assert_eq!(truncated(&"é".repeat(300), 499).len(), 498);
        assert_eq!(truncated("short", 500), "short");
    }
}
