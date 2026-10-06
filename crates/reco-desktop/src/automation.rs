//! The Slint app's benchmark hooks, built only with the `automation`
//! feature (off in releases): `RECO_AUTOLOAD` opens a match,
//! `RECO_AUTOEXPORT` exports it (`_MODEL` with AI tracking, `_LOOKAHEAD`
//! seconds of lookahead, `_REPEAT` runs back to back as `out_1.mp4`,
//! `out_2.mp4`, …, logging the GPU's memory after each so a leak across
//! exports shows), then quits; `RECO_VRAM_BUDGET_GB` sets the lookahead's
//! memory budget, to reach its risk zones on a large GPU.

use std::path::{Path, PathBuf};

use makepad_widgets::*;

use crate::App;

/// What `RECO_AUTOEXPORT` asks for.
#[derive(Clone, Debug, PartialEq)]
pub struct AutoExport {
    /// The first run's file is `output` with `_1` added.
    pub output: PathBuf,
    /// Track with this model.
    pub model: Option<PathBuf>,
    /// The lookahead, seconds.
    pub lookahead_secs: f64,
    /// Runs back to back.
    pub repeats: u32,
}

/// The auto-export `env` asks for (`None` without `RECO_AUTOEXPORT`).
pub fn auto_export(env: impl Fn(&str) -> Option<String>) -> Option<AutoExport> {
    let output = env("RECO_AUTOEXPORT").filter(|o| !o.trim().is_empty())?;
    Some(AutoExport {
        output: PathBuf::from(output.trim()),
        model: env("RECO_AUTOEXPORT_MODEL")
            .filter(|m| !m.trim().is_empty())
            .map(|m| PathBuf::from(m.trim())),
        lookahead_secs: env("RECO_AUTOEXPORT_LOOKAHEAD")
            .and_then(|v| v.trim().parse::<f64>().ok())
            .filter(|s| s.is_finite() && *s >= 0.0)
            .unwrap_or(0.0),
        repeats: env("RECO_AUTOEXPORT_REPEAT")
            .and_then(|v| v.trim().parse().ok())
            .filter(|n| *n > 0)
            .unwrap_or(1),
    })
}

/// `RECO_AUTOLOAD`'s "left[;left2],right[;right2],cal.json" as the command
/// line's `--left/--right/--calibration` (`None` when unset or malformed).
pub fn autoload_args(env: impl Fn(&str) -> Option<String>) -> Option<Vec<String>> {
    let raw = env("RECO_AUTOLOAD")?;
    let parts: Vec<&str> = raw.split(',').map(str::trim).collect();
    let [left, right, cal] = parts.as_slice() else {
        return None;
    };
    if [left, right, cal].iter().any(|p| p.is_empty()) {
        return None;
    }
    Some(
        ["--left", left, "--right", right, "--calibration", cal]
            .map(String::from)
            .to_vec(),
    )
}

/// Run `n`'s file (1-based): `base_n.ext`, so runs don't overwrite each
/// other.
pub fn run_path(base: &Path, n: u32) -> PathBuf {
    let stem = base
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("export");
    let ext = base.extension().and_then(|e| e.to_str()).unwrap_or("mp4");
    base.with_file_name(format!("{stem}_{n}.{ext}"))
}

/// `RECO_VRAM_BUDGET_GB` in bytes.
pub fn vram_budget(env: impl Fn(&str) -> Option<String>) -> Option<usize> {
    env("RECO_VRAM_BUDGET_GB")
        .and_then(|gb| gb.trim().parse::<f64>().ok())
        .filter(|gb| gb.is_finite() && *gb > 0.0)
        .map(|gb| (gb * 1e9) as usize)
}

/// The GPU's free and used memory by `nvidia-smi`, for the runs' log
/// lines (never fails a run).
pub fn vram_line() -> String {
    std::process::Command::new("nvidia-smi")
        .args([
            "--query-gpu=memory.free,memory.used",
            "--format=csv,noheader,nounits",
        ])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .filter(|s| !s.trim().is_empty())
        .map(|s| format!("free/used MB = {}", s.trim()))
        .unwrap_or_else(|| "nvidia-smi unavailable".into())
}

/// A benchmark run under way.
#[derive(Clone, Debug)]
pub(crate) struct AutoRun {
    export: AutoExport,
    /// Runs ended.
    done: u32,
    /// The first has started.
    started: bool,
    /// The last has ended: quit at the timer.
    quitting: bool,
}

impl AutoRun {
    pub(crate) fn new(export: AutoExport) -> Self {
        Self {
            export,
            done: 0,
            started: false,
            quitting: false,
        }
    }
}

impl App {
    /// The match is open, or the machine has said whether AI tracking runs:
    /// start the first run once both are in.
    pub(crate) fn auto_export_ready(&mut self, cx: &mut Cx) {
        let Some(run) = self.auto_run.as_ref() else {
            return;
        };
        // Open with its first frame shown (its rate known) and a range.
        let shown = self
            .live
            .as_ref()
            .is_some_and(|l| l.open && l.info.is_some());
        let waiting_for_ai = run.export.model.is_some() && self.ai_availability.is_none();
        if run.started || !shown || self.export_range.is_none() || waiting_for_ai {
            return;
        }
        log!(
            "RECO_AUTOEXPORT: baseline {}; starting export 1/{} -> {} (lookahead {}s)",
            vram_line(),
            run.export.repeats,
            run_path(&run.export.output, 1).display(),
            run.export.lookahead_secs
        );
        if let Some(run) = self.auto_run.as_mut() {
            run.started = true;
        }
        self.start_auto_run(cx, 1);
    }

    /// Run `n`: the sheet filled as the Slint app filled it, and Export.
    fn start_auto_run(&mut self, cx: &mut Cx, n: u32) {
        let Some(export) = self.auto_run.as_ref().map(|r| r.export.clone()) else {
            return;
        };
        self.open_export_sheet(cx);
        let output = run_path(&export.output, n);
        self.ui
            .text_input(cx, ids!(export_output))
            .set_text(cx, &output.display().to_string());
        if let Some(model) = &export.model {
            self.set_auto_tracking(cx, model, export.lookahead_secs);
        }
        self.begin_export(cx);
    }

    /// A run ended (`outcome`: done, failed, cancelled): the next after a
    /// pause, or quit after the last, or after any that didn't finish, so
    /// a script never waits.
    pub(crate) fn auto_export_ended(&mut self, cx: &mut Cx, outcome: &str) {
        let Some(run) = self.auto_run.as_mut() else {
            return;
        };
        run.done += 1;
        let (done, total) = (run.done, run.export.repeats);
        if outcome == "done" && done < total {
            log!(
                "RECO_AUTOEXPORT: run {done}/{total} complete, {}; starting next",
                vram_line()
            );
            self.auto_timer = cx.start_timeout(0.8);
        } else {
            log!(
                "RECO_AUTOEXPORT: run {done}/{total} {outcome}, final {}; quitting",
                vram_line()
            );
            run.quitting = true;
            self.auto_timer = cx.start_timeout(1.5);
        }
    }

    /// The pause after a run is over.
    pub(crate) fn auto_timer_fired(&mut self, cx: &mut Cx) {
        let Some((quitting, done)) = self.auto_run.as_ref().map(|r| (r.quitting, r.done)) else {
            return;
        };
        if quitting {
            cx.quit();
        } else {
            self.start_auto_run(cx, done + 1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env(pairs: &'static [(&'static str, &'static str)]) -> impl Fn(&str) -> Option<String> {
        move |key: &str| {
            pairs
                .iter()
                .find(|(k, _)| *k == key)
                .map(|(_, v)| v.to_string())
        }
    }

    #[test]
    fn the_export_reads_as_slint_read_it() {
        assert_eq!(auto_export(env(&[])), None);
        assert_eq!(
            auto_export(env(&[("RECO_AUTOEXPORT", "/out/run.mp4")])),
            Some(AutoExport {
                output: "/out/run.mp4".into(),
                model: None,
                lookahead_secs: 0.0,
                repeats: 1,
            })
        );
        assert_eq!(
            auto_export(env(&[
                ("RECO_AUTOEXPORT", "/out/run.mp4"),
                ("RECO_AUTOEXPORT_MODEL", "/m/yolo.onnx"),
                ("RECO_AUTOEXPORT_LOOKAHEAD", "4.5"),
                ("RECO_AUTOEXPORT_REPEAT", "3"),
            ])),
            Some(AutoExport {
                output: "/out/run.mp4".into(),
                model: Some("/m/yolo.onnx".into()),
                lookahead_secs: 4.5,
                repeats: 3,
            })
        );
        let odd = auto_export(env(&[
            ("RECO_AUTOEXPORT", "/out/run.mp4"),
            ("RECO_AUTOEXPORT_LOOKAHEAD", "soon"),
            ("RECO_AUTOEXPORT_REPEAT", "0"),
        ]))
        .unwrap();
        assert_eq!(
            (odd.lookahead_secs, odd.repeats),
            (0.0, 1),
            "what doesn't read is left out"
        );
    }

    #[test]
    fn autoload_becomes_the_command_line() {
        assert_eq!(autoload_args(env(&[])), None);
        assert_eq!(
            autoload_args(env(&[(
                "RECO_AUTOLOAD",
                "/a/l1.mp4;/a/l2.mp4, /a/r.mp4 ,/a/cal.json"
            )])),
            Some(
                [
                    "--left",
                    "/a/l1.mp4;/a/l2.mp4",
                    "--right",
                    "/a/r.mp4",
                    "--calibration",
                    "/a/cal.json"
                ]
                .map(String::from)
                .to_vec()
            )
        );
        assert_eq!(
            autoload_args(env(&[("RECO_AUTOLOAD", "/a/l.mp4,/a/r.mp4")])),
            None
        );
        assert_eq!(
            autoload_args(env(&[("RECO_AUTOLOAD", "/a/l.mp4,,/a/cal.json")])),
            None
        );
    }

    #[test]
    fn runs_number_their_files() {
        assert_eq!(
            run_path(Path::new("/out/run.mp4"), 1),
            PathBuf::from("/out/run_1.mp4")
        );
        assert_eq!(
            run_path(Path::new("/out/run"), 2),
            PathBuf::from("/out/run_2.mp4")
        );
    }

    #[test]
    fn the_budget_is_in_gigabytes() {
        assert_eq!(
            vram_budget(env(&[("RECO_VRAM_BUDGET_GB", "1.5")])),
            Some(1_500_000_000)
        );
        assert_eq!(vram_budget(env(&[("RECO_VRAM_BUDGET_GB", "lots")])), None);
        assert_eq!(vram_budget(env(&[])), None);
    }
}
