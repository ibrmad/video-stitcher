# Module 2: Time Panel and Status — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The time panel works for real. It has:
- play, pause and step, with a play/pause icon that follows the state;
- a ruler you can scrub, which seeks on release;
- camera lanes built from the actual files (a gap at each chapter, each camera's span after the sync offset);
- the export-range tint;
- recording the preview to a video at a chosen quality.

A status line says what is happening, toasts carry events and errors without overwriting it, and the preview aspect is remembered between runs.

**Architecture:**
- `reco-app` gains:
  - `settings` (`desktop.json` through reco-io's settings module);
  - `recording` (quality, frame size, folder, file name);
  - `toasts` (a pure model with expiry times);
  - `reveal` ("Show in folder").
- `reco_app::preview` gains:
  - `lanes`: per-file durations, probed on a short-lived thread and laid out on the stitched timeline;
  - `recorder`: its own fixed-size `StitchRenderer` → NV12 readback → an encoder thread, one frame per source frame shown.
- The render worker gains:
  - absolute seeks, coalesced per batch, with failed seeks reported;
  - the play state in `Time`;
  - panic reporting;
  - lanes;
  - recording.
- `reco-desktop`:
  - moves the live session's methods out of `main.rs` into `session_view.rs`;
  - wires transport, status line, ruler scrubbing, lanes and tint;
  - adds toast cards in the viewer, the recording UI and saved settings;
  - caches the side panels, and measures draw time behind `--perf-log`.

**Tech Stack:**
- Rust 1.92 (pinned); Makepad 2 (git rev pinned);
- wgpu 28; reco-core / reco-io (with the `config` feature) / reco-control;
- serde 1;
- Python 3 standard library for checks (`ffprobe`, when installed, to inspect recordings).

**Spec:**
- `crates/reco-desktop/DESIGN.md`: Modules table row 2, Look, Architecture, Rules 4, 8, 9 and 11.
- `crates/reco-desktop/PARITY.md`: Module 2.
- Behavioural reference: `crates/reco-desktop/docs/slint-inventory.md` §2.5, §2.6, §2.13, and the Slint code in `crates/reco-gui/src/{main.rs,playback.rs,toast.rs,settings.rs}`.
- Research summary (2026-10-05). The inventory misses these points, and the plan argues from them:
  - **Slint flaws not to copy:**
    - Recordings are cropped to the top-left of a 1080p render.
    - They are recorded per render (slow motion while panning), drop frames silently, and lose the last two frames.
    - Toasts are laid out below the window and overwrite the status line.
    - Space does nothing at the end of the file.
    - The f32 seek fraction can land one frame early.
  - **Engine facts:**
    - The encoder is constant frame rate and ignores `pts_us`.
    - `StitchPipeline::resize` never recreates the internal render target, so a recorder needs its own renderer.
    - `StitchRenderer`/`Nv12Converter` are `!Send`; only the encoder moves threads.
    - There is no public frame count. `total_frames()` ignores the sync offset.
    - A failed seek returns `Ok` and then looks like the end of the stream.
    - reco-io's file-duration helpers are private; `VideoDecoder::open(p)?.duration_secs()` is public.

## Global Constraints

- **Toolchain:** Rust `1.92.0`; do not bump it.
- **Editions:** `reco-app` 2024 (let-chains allowed); `reco-desktop` 2021 (no let-chains).
- **Makepad pin:** `rev = "62691a290eb58f7d5234960524429aaada3e572c"`.
- **Engine crates unchanged:** `reco-core`, `reco-io`, `reco-control`, `reco-calibrate`, `reco-autocam`, `reco-detect`, `reco-gui`. A gap gets a `crates/reco-desktop/FRICTION.md` entry and a ruling, not a patch. Enabling an existing reco-io feature from `reco-app`'s manifest is allowed.
- **Threads (DESIGN.md Architecture, Rule 4):**
  - The UI thread never blocks; settings writes are its only file I/O. Spawning the "Show in folder" command is allowed, but never waiting on it.
  - GPU work, opening, probing, seeking and encoding happen off the UI thread.
  - The one exception is quitting while recording (`Event::Shutdown`): the UI waits up to 3 s for the file to be finalised.
- **Settings file:** reco-io `settings`, namespace `"desktop"` (`~/Library/Application Support/reco/desktop.json`; `RECO_CONFIG_DIR` overrides it). Never the Slint app's `"gui"`. Values are stored by name, as the Slint app stored them:
  - `preview_aspect`: `"auto"|"16:9"|"4:3"|"21:9"`
  - `recording_quality`: `"fast"|"balanced"|"high"`
  - `recording_folder`: a path or `null`
- **Recording values (Slint parity unless marked New):**
  - **File name:** `reco_recording_{unix_seconds}.mp4`.
  - **Folder:** the saved `recording_folder` if it is a directory, else the first left file's folder, else the current folder.
  - **Encoding:** codec `"h264"`; quality fast/balanced/high; no audio.
  - **Size (New):** 1080 rows, as wide as the preview aspect (Auto → 1920×1080). Slint cropped a fixed render.
  - **Frames (New):** one frame per source frame shown. Nothing is recorded while paused, and no frame is dropped.
  - **Toasts:** "Recording started" (info, body: the path), "Recording saved" (info, 8 s, body `"{frames} frames · {file name}"`), "Recording failed" (error, body: the reason).
- **Toasts:**
  - Lifetimes: info 4 s, warn 7 s, error 10 s, or a custom time.
  - At most 4, the oldest leaving first, the newest at the bottom.
  - Dismissable.
  - They sit inside the viewer (clear of the Adjust panel and the time panel) and are never written into the status line.
  - New: a toast identical to one on screen refreshes it instead of stacking.
- **Seeks** go by frame index, never by fraction.
- **Theme:** every colour, size and spacing in `src/ui/` comes from `src/theme.rs` (`screens_use_theme_values_only`: no `key: <number>` and no `#hex` in `src/ui/*.rs` outside `mod.rs`, including Rust code). Text roles carry no padding or margin (Rule 11).
- **Icons:** every new SVG is 16×16, pins `<rect x="0" y="0" width="16" height="16" fill="none"/>`, and paints in `#000` (the widget tints it).
- **Code standards:**
  - `cargo fmt -p reco-app -p reco-desktop`.
  - `cargo clippy -p reco-app -p reco-desktop --no-deps --all-targets -- -D warnings`. `--no-deps` is needed because reco-core trips `unnecessary_cast` on 1.92 (FRICTION.md).
  - `///` on public items and `//!` on modules; tests in each logic module; conventional commits; no attribution lines.
- **Fixtures (never committed):**
  - Fast set: `~/dev/pitchcam-data/alfheim/{cam0.mp4,cam1.mp4,reco/match.json}` (1280×960, 30 fps, 60 s, sync offset 0).
  - Real set: `~/Downloads/match_recording/{left/GX010120.MP4,right/GX010092.MP4,left/GX010120_calibration.json}` (5.3K HEVC, 29.97 fps).
  - Chained: the same file twice, `cam0.mp4;cam0.mp4`.
  - Tests and checks skip with a message when a fixture is missing.
- **Checks:**
  - `tools/check_m2.py`, standard library only; `check_m2.py NAME…` runs single checks. Screenshots go to `target/desktop-checks/m2/`. Windows are hidden.
  - Every check launch gets a fresh `RECO_CONFIG_DIR`, so checks never touch the owner's settings.
  - Checks never click "Show in folder", because it would open Finder on the owner's screen.

## Review Focus

1. **A burst of seeks** (holding `]`, quick scrubs) must cost one seek per batch, the last target winning, and the playhead must end where the keys say. Owner: Task 4 (`relative_seeks_accumulate`, `seek_burst_lands_on_the_sum`).
2. **Recording while the preview resizes, pans, seeks or pauses.** The file keeps its fixed size and has exactly one frame per source frame shown: no slow motion, and no frames added while paused. The last frames are flushed, and stopping or quitting always leaves a playable file. Owner: Task 6 (`recording_has_one_frame_per_source_frame`, `recording_keeps_its_size_when_the_preview_resizes`) and Task 7 (`pausing_adds_no_frames`, `quitting_while_recording_finishes_the_file`).
3. **A seek past the real end** (the sync offset makes the engine overestimate the length) must say it failed and keep the frame on screen, never silently "finish". The ruler's length comes from the probed files. Owner: Task 4 (`seek_past_the_end_is_an_error`) and Task 5 (`length_is_where_both_cameras_have_video`, `lanes_arrive_after_open`).
4. **A flood of notices** (more than four, or the same error again and again) must show at most four, push the oldest out first, and refresh a repeat instead of stacking it. Owner: Task 3 (`at_most_four_newest_last`, `a_repeat_refreshes_instead_of_stacking`).
5. **A settings file that is missing, malformed or has unknown values** gives the defaults and never crashes; an unknown aspect reads as Auto. Owner: Task 2 (`missing_fields_take_defaults`, `unknown_values_fall_back`) and Task 8 (check `persist`: a malformed file still opens).

## File map

| File | Status | Responsibility |
|---|---|---|
| `crates/reco-app/Cargo.toml` | modify | reco-io features `ffmpeg` + `config`; serde; serde_json (dev) |
| `crates/reco-app/src/lib.rs` | modify | `pub mod recording; reveal; settings; toasts;` |
| `crates/reco-app/src/settings.rs` | create | `DesktopSettings`, `load`, `save` |
| `crates/reco-app/src/recording.rs` | create | `RecordingQuality`, `recording_size`, `recording_folder`, `recording_file_name` |
| `crates/reco-app/src/toasts.rs` | create | `Severity`, `Toast`, `Toasts` |
| `crates/reco-app/src/reveal.rs` | create | `reveal_command`, `reveal` |
| `crates/reco-app/src/preview/view.rs` | modify | `PreviewAspect::{name, from_name, index}` |
| `crates/reco-app/src/preview/playback.rs` | modify | `seek_to_frame`, failed-seek error, `seek_goal`, `set_total_frames`, `fps_rational` |
| `crates/reco-app/src/preview/lanes.rs` | create | `Lanes`, `camera_spans`, `skips`, `lanes`, `probe` |
| `crates/reco-app/src/preview/recorder.rs` | create | `Recorder`, `Recording` |
| `crates/reco-app/src/preview/session.rs` | modify | `build_renderer`, recording methods, `sync_offset`, `SessionError::Record` |
| `crates/reco-app/src/preview/worker.rs` | modify | `SeekTo`, coalescing, `Time{frame,state}`, panic report, lanes, recording |
| `crates/reco-desktop/src/session_view.rs` | create | the live session's `impl App` (moved from main.rs, then extended) |
| `crates/reco-desktop/src/main.rs` | modify | App fields, startup DSL icons, settings, perf timing, shutdown |
| `crates/reco-desktop/src/live.rs` | modify | `Live::new`, status line, `FpsMeter::reset` |
| `crates/reco-desktop/src/keys.rs` | modify | `repeats` |
| `crates/reco-desktop/src/cli.rs` | modify | `--export-range`, `--toast-demo`, `--perf-log` |
| `crates/reco-desktop/src/time_ruler.rs` | modify | `time_at`, `tint_span`, `settled` |
| `crates/reco-desktop/src/perf.rs` | create | `DrawStats` |
| `crates/reco-desktop/src/theme.rs` | modify | new tokens |
| `crates/reco-desktop/src/ui/time_panel.rs` | modify | ruler scrubbing, tint, hold; Show in folder |
| `crates/reco-desktop/src/ui/toasts.rs` | create | `RecoToastCard`, `RecoToasts` |
| `crates/reco-desktop/src/ui/viewer.rs` | modify | toast layer; quality dropdown and recording badge |
| `crates/reco-desktop/src/ui/preview.rs` | modify | key repeats |
| `crates/reco-desktop/src/ui/shell.rs` | modify | cached side panels |
| `crates/reco-desktop/src/ui/mod.rs` | modify | register `toasts` |
| `crates/reco-desktop/resources/icons/{close,stop}.svg` | create | icons |
| `crates/reco-desktop/tools/drive.py`, `test_drive.py` | modify | `launch_env`, `env=` |
| `crates/reco-desktop/tools/check_m2.py` | create | the Module 2 check, grown task by task |
| `crates/reco-desktop/{PARITY,DESIGN,FRICTION}.md` | modify | evidence and notes |

---

### Task 1: Move the live session out of main.rs; checks get their own settings folder

`main.rs` is about 840 lines and Module 2 adds more. The App's live-session methods move, unchanged, into `session_view.rs`. Child modules of the crate root see `App`'s private fields, so nothing else changes. Checks start writing settings from Task 8, so every launch must get its own settings folder before then.

**Files:**
- Create: `crates/reco-desktop/src/session_view.rs`
- Modify: `crates/reco-desktop/src/main.rs`
- Modify: `crates/reco-desktop/tools/drive.py`, `crates/reco-desktop/tools/test_drive.py`

**Interfaces:**
- Produces:
  - `session_view.rs` with `impl App { start_live, drain_preview, show_opening, show_live, show_failed, show_stopped, show_time, update_ruler, preview_actions }`. These are the same signatures as today, with visibility `pub(crate)` where main.rs calls them (`start_live`, `drain_preview`, `preview_actions`).
  - `drive.launch_env(base: dict, env: dict | None, hidden: bool) -> dict`.
  - `drive.App.launch(binary, args=(), hidden=True, timeout=60.0, env=None)`.

- [ ] **Step 1: Write the failing driver test**

Add to `tools/test_drive.py`:

```python
class LaunchEnv(unittest.TestCase):
    def test_each_launch_gets_its_own_settings_folder(self):
        a = drive.launch_env({"PATH": "/bin"}, None, hidden=True)
        b = drive.launch_env({"PATH": "/bin"}, None, hidden=True)
        self.assertEqual(a["MAKEPAD_HIDE_WINDOWS"], "1")
        self.assertTrue(os.path.isdir(a["RECO_CONFIG_DIR"]))
        self.assertNotEqual(a["RECO_CONFIG_DIR"], b["RECO_CONFIG_DIR"])

    def test_a_given_folder_wins(self):
        env = drive.launch_env({}, {"RECO_CONFIG_DIR": "/tmp/x"}, hidden=False)
        self.assertEqual(env["RECO_CONFIG_DIR"], "/tmp/x")
        self.assertNotIn("MAKEPAD_HIDE_WINDOWS", env)
```

Run: `/usr/bin/python3 crates/reco-desktop/tools/test_drive.py`
Expected: 2 errors (`module 'drive' has no attribute 'launch_env'`).

- [ ] **Step 2: Implement `launch_env` and use it**

In `drive.py`, above `class App`:

```python
def launch_env(base, env, hidden):
    """The environment for a launched app: hidden windows if asked, and a
    fresh settings folder (RECO_CONFIG_DIR) unless one is given, so checks
    never read or write the owner's settings."""
    out = dict(base)
    out.update(env or {})
    if hidden:
        out["MAKEPAD_HIDE_WINDOWS"] = "1"
    if "RECO_CONFIG_DIR" not in (env or {}):
        out["RECO_CONFIG_DIR"] = tempfile.mkdtemp(prefix="reco-desktop-config-")
    return out
```

Change `App.launch` to take `env=None` and build its environment with `launch_env(os.environ, env, hidden)` in place of the current `env = dict(os.environ)` / `MAKEPAD_HIDE_WINDOWS` lines.

Run: `/usr/bin/python3 crates/reco-desktop/tools/test_drive.py`
Expected: OK (10 tests).

- [ ] **Step 3: Move the live session's methods**

Create `src/session_view.rs`:

```rust
//! The live session's side of the App: it starts the render worker, routes
//! its events to the preview widget and the shell, and turns transport
//! actions into commands. Split from main.rs, which keeps the shell, the
//! look preview and startup.

use std::sync::Arc;
use std::time::Instant;

use makepad_widgets::makepad_platform::thread::SignalToUI;
use makepad_widgets::*;
use reco_app::preview::view::PreviewAspect;
use reco_app::preview::worker::{
    PreviewCommand, PreviewConfig, PreviewEvent, PreviewInfo, PreviewWorker,
};

use crate::live::{self, FpsMeter, Live};
use crate::names::middle_ellipsis;
use crate::time_ruler;
use crate::ui::preview::{PreviewAction, RecoPreview};
use crate::ui::time_panel::RecoTimeRuler;
use crate::{cli, display_device, App, Step, PROJECT_NAME_CHARS};

impl App {
}
```

Cut these methods from `impl App` in `main.rs` and paste them, unchanged, inside the new `impl App` block, in this order:
- `start_live`
- `drain_preview`
- `show_opening`
- `show_live`
- `show_failed`
- `show_stopped`
- `show_time`
- `update_ruler`
- `preview_actions`

Mark `start_live`, `drain_preview` and `preview_actions` `pub(crate)`. In `main.rs`:
- add `mod session_view;`;
- delete the imports only the moved code used (`Arc`, `Instant`, `SignalToUI`, `PreviewAspect`, the `reco_app::preview::worker` items, `FpsMeter`, `Live`, `PreviewAction`, `RecoPreview`). The compiler's unused-import warnings list them.
- `RecoTimeRuler`, `middle_ellipsis` and `time_ruler` stay in `main.rs`, because `show_state` uses them.

- [ ] **Step 4: Build, test, and check that nothing changed**

Run: `cargo build --profile desktop -p reco-desktop && cargo test -p reco-desktop && cargo clippy -p reco-desktop --no-deps --all-targets -- -D warnings && /usr/bin/python3 crates/reco-desktop/tools/check_m1.py`
Expected: builds with no warnings; 47 unit tests pass; "Module 1 check passed".

- [ ] **Step 5: Commit**

```bash
git add crates/reco-desktop/src/main.rs crates/reco-desktop/src/session_view.rs crates/reco-desktop/tools/drive.py crates/reco-desktop/tools/test_drive.py
git commit -m "refactor(desktop): move the live session into session_view.rs; checks get their own settings folder"
```

---

### Task 2: Settings and recording choices (reco-app)

**Files:**
- Modify: `crates/reco-app/Cargo.toml`, `crates/reco-app/src/lib.rs`, `crates/reco-app/src/preview/view.rs`
- Create: `crates/reco-app/src/recording.rs`, `crates/reco-app/src/settings.rs`

**Interfaces:**
- Produces:
  - `PreviewAspect::name(self) -> &'static str` (`"auto"`, `"16:9"`, `"4:3"`, `"21:9"`), `PreviewAspect::from_name(&str) -> PreviewAspect` (unknown → `Auto`), `PreviewAspect::index(self) -> usize`.
  - `recording::RECORDING_CODEC: &str = "h264"`.
  - `recording::RecordingQuality { Fast, Balanced (default), High }`, with:
    - `name(self) -> &'static str`
    - `from_name(&str) -> Self` (unknown → `Balanced`)
    - `index(self) -> usize`
    - `from_index(usize) -> Self` (out of range → `Balanced`)
  - `recording::recording_size(PreviewAspect) -> (u32, u32)`.
  - `recording::recording_folder(saved: Option<&Path>, first_left: &Path) -> PathBuf`.
  - `recording::recording_file_name(unix_secs: u64) -> String`.
  - `settings::DesktopSettings { pub preview_aspect: String, pub recording_quality: String, pub recording_folder: Option<PathBuf> }`, with `aspect()`, `set_aspect(PreviewAspect)`, `quality()` and `set_quality(RecordingQuality)`.
  - `settings::NAMESPACE = "desktop"`, `settings::load() -> DesktopSettings`, `settings::save(&DesktopSettings) -> Result<(), String>`.

- [ ] **Step 1: Dependencies and modules**

`crates/reco-app/Cargo.toml`: change the reco-io line and add serde:

```toml
reco-io = { path = "../reco-io", features = ["ffmpeg", "config"] }
serde = { version = "1", features = ["derive"] }

[dev-dependencies]
serde_json = "1"
```

`src/lib.rs`: below `pub mod preview;` add `pub mod recording;` and `pub mod settings;`.

- [ ] **Step 2: Write the failing tests**

Append to `preview/view.rs`'s tests:

```rust
    #[test]
    fn aspect_names_and_indices_round_trip() {
        for aspect in [
            PreviewAspect::Auto,
            PreviewAspect::Wide16x9,
            PreviewAspect::Classic4x3,
            PreviewAspect::Cinema21x9,
        ] {
            assert_eq!(PreviewAspect::from_name(aspect.name()), aspect);
            assert_eq!(PreviewAspect::from_index(aspect.index()), aspect);
        }
        assert_eq!(PreviewAspect::Classic4x3.name(), "4:3");
        assert_eq!(PreviewAspect::from_name("5:4"), PreviewAspect::Auto);
    }
```

Add `impl PreviewAspect` stubs, so the tests compile and fail:

```rust
    /// The settings name: "auto", "16:9", "4:3" or "21:9".
    pub fn name(self) -> &'static str {
        todo!()
    }

    /// The aspect saved under `name`; Auto for anything else.
    pub fn from_name(name: &str) -> Self {
        todo!()
    }

    /// The dropdown index (Auto, 16:9, 4:3, 21:9).
    pub fn index(self) -> usize {
        todo!()
    }
```

`src/recording.rs`:

```rust
//! The choices behind Record: quality, frame size, folder and file name.
//! The name and folder rules are the Slint app's, so recordings from both
//! apps sort together.

use std::path::{Path, PathBuf};

use crate::preview::view::PreviewAspect;

/// The codec recordings use (the Slint app's default; a choice arrives with
/// Preferences in Module 7).
pub const RECORDING_CODEC: &str = "h264";

/// Recording quality, named as the encoder presets name it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RecordingQuality {
    /// Smaller files, quicker to encode.
    Fast,
    /// The default.
    #[default]
    Balanced,
    /// Larger files, the best picture.
    High,
}

impl RecordingQuality {
    /// The encoder's and the settings' name.
    pub fn name(self) -> &'static str {
        todo!()
    }

    /// The quality called `name`; Balanced for anything else.
    pub fn from_name(name: &str) -> Self {
        todo!()
    }

    /// The dropdown index (Fast, Balanced, High).
    pub fn index(self) -> usize {
        todo!()
    }

    /// The quality at a dropdown index; Balanced out of range.
    pub fn from_index(index: usize) -> Self {
        todo!()
    }
}

/// The frame size a recording is made at: 1080 rows, as wide as the preview
/// aspect (16:9 for Auto). Widths are multiples of four for the NV12 path.
pub fn recording_size(aspect: PreviewAspect) -> (u32, u32) {
    todo!()
}

/// Where recordings go: the saved folder if it is a directory, else beside
/// the left camera's first file, else the current folder (canonical).
pub fn recording_folder(saved: Option<&Path>, first_left: &Path) -> PathBuf {
    todo!()
}

/// `reco_recording_<unix seconds>.mp4`, the Slint app's name.
pub fn recording_file_name(unix_secs: u64) -> String {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn qualities_round_trip() {
        for q in [RecordingQuality::Fast, RecordingQuality::Balanced, RecordingQuality::High] {
            assert_eq!(RecordingQuality::from_name(q.name()), q);
            assert_eq!(RecordingQuality::from_index(q.index()), q);
        }
        assert_eq!(RecordingQuality::High.name(), "high");
        assert_eq!(RecordingQuality::from_name("ultra"), RecordingQuality::Balanced);
        assert_eq!(RecordingQuality::from_index(7), RecordingQuality::Balanced);
    }

    #[test]
    fn recordings_are_1080_rows_as_wide_as_the_aspect() {
        assert_eq!(recording_size(PreviewAspect::Auto), (1920, 1080));
        assert_eq!(recording_size(PreviewAspect::Wide16x9), (1920, 1080));
        assert_eq!(recording_size(PreviewAspect::Classic4x3), (1440, 1080));
        assert_eq!(recording_size(PreviewAspect::Cinema21x9), (2520, 1080));
        for aspect in [PreviewAspect::Classic4x3, PreviewAspect::Cinema21x9] {
            let (w, h) = recording_size(aspect);
            assert_eq!((w % 4, h % 2), (0, 0), "NV12 needs w % 4 and even h");
        }
    }

    #[test]
    fn recordings_go_to_the_saved_folder_or_beside_the_video() {
        let dir = std::env::temp_dir().join(format!("reco-app-rec-folder-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let canonical = std::fs::canonicalize(&dir).unwrap();
        let video = dir.join("left.mp4");
        assert_eq!(recording_folder(None, &video), canonical);
        assert_eq!(recording_folder(Some(Path::new("/nonexistent/folder")), &video), canonical);
        assert_eq!(recording_folder(Some(&dir), Path::new("elsewhere.mp4")), canonical);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn names_follow_the_slint_app() {
        assert_eq!(recording_file_name(1_700_000_000), "reco_recording_1700000000.mp4");
    }
}
```

`src/settings.rs`:

```rust
//! The desktop app's settings, kept in `desktop.json` in reco-io's settings
//! folder (beside the Slint app's `gui.json`; `RECO_CONFIG_DIR` overrides
//! the folder). Values are stored by name, as the Slint app stored them,
//! and anything missing or unknown reads as its default.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::preview::view::PreviewAspect;
use crate::recording::RecordingQuality;

/// The settings file's name in reco-io's settings folder.
pub const NAMESPACE: &str = "desktop";

/// What the desktop app remembers between runs.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DesktopSettings {
    /// The preview aspect: "auto", "16:9", "4:3" or "21:9".
    pub preview_aspect: String,
    /// Recording quality: "fast", "balanced" or "high".
    pub recording_quality: String,
    /// Where recordings go; beside the left video when unset or missing.
    pub recording_folder: Option<PathBuf>,
}

impl Default for DesktopSettings {
    fn default() -> Self {
        todo!()
    }
}

impl DesktopSettings {
    /// The saved preview aspect (Auto when unknown).
    pub fn aspect(&self) -> PreviewAspect {
        todo!()
    }

    /// Remember `aspect`.
    pub fn set_aspect(&mut self, aspect: PreviewAspect) {
        todo!()
    }

    /// The saved recording quality (Balanced when unknown).
    pub fn quality(&self) -> RecordingQuality {
        todo!()
    }

    /// Remember `quality`.
    pub fn set_quality(&mut self, quality: RecordingQuality) {
        todo!()
    }
}

/// The saved settings, or the defaults when the file is missing or
/// unreadable (reco-io logs a warning for a malformed file).
pub fn load() -> DesktopSettings {
    reco_io::settings::load_or_default(NAMESPACE)
}

/// Save the settings: a small atomic write, the one file write the UI
/// thread may do.
pub fn save(settings: &DesktopSettings) -> Result<(), String> {
    reco_io::settings::save(NAMESPACE, settings).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_the_slint_app() {
        let d = DesktopSettings::default();
        assert_eq!(d.preview_aspect, "auto");
        assert_eq!(d.recording_quality, "balanced");
        assert_eq!(d.recording_folder, None);
    }

    #[test]
    fn missing_fields_take_defaults() {
        let s: DesktopSettings = serde_json::from_str(r#"{"preview_aspect":"4:3"}"#).unwrap();
        assert_eq!(s.aspect(), PreviewAspect::Classic4x3);
        assert_eq!(s.quality(), RecordingQuality::Balanced);
    }

    #[test]
    fn unknown_values_fall_back() {
        let s: DesktopSettings =
            serde_json::from_str(r#"{"preview_aspect":"5:4","recording_quality":"ultra","extra":1}"#).unwrap();
        assert_eq!(s.aspect(), PreviewAspect::Auto);
        assert_eq!(s.quality(), RecordingQuality::Balanced);
    }

    #[test]
    fn setters_store_names() {
        let mut s = DesktopSettings::default();
        s.set_aspect(PreviewAspect::Cinema21x9);
        s.set_quality(RecordingQuality::High);
        let text = serde_json::to_string(&s).unwrap();
        assert!(text.contains("\"21:9\"") && text.contains("\"high\""), "{text}");
        let back: DesktopSettings = serde_json::from_str(&text).unwrap();
        assert_eq!(back, s);
    }
}
```

- [ ] **Step 3: Run them to see them fail**

Run: `cargo test -p reco-app -- names round_trip recordings settings`
Expected: FAIL, every new test panics with `not yet implemented`.

- [ ] **Step 4: Implement**

`view.rs`:

```rust
    pub fn name(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Wide16x9 => "16:9",
            Self::Classic4x3 => "4:3",
            Self::Cinema21x9 => "21:9",
        }
    }

    pub fn from_name(name: &str) -> Self {
        match name {
            "16:9" => Self::Wide16x9,
            "4:3" => Self::Classic4x3,
            "21:9" => Self::Cinema21x9,
            _ => Self::Auto,
        }
    }

    pub fn index(self) -> usize {
        match self {
            Self::Auto => 0,
            Self::Wide16x9 => 1,
            Self::Classic4x3 => 2,
            Self::Cinema21x9 => 3,
        }
    }
```

`recording.rs`:

```rust
    pub fn name(self) -> &'static str {
        match self {
            Self::Fast => "fast",
            Self::Balanced => "balanced",
            Self::High => "high",
        }
    }

    pub fn from_name(name: &str) -> Self {
        match name {
            "fast" => Self::Fast,
            "high" => Self::High,
            _ => Self::Balanced,
        }
    }

    pub fn index(self) -> usize {
        match self {
            Self::Fast => 0,
            Self::Balanced => 1,
            Self::High => 2,
        }
    }

    pub fn from_index(index: usize) -> Self {
        match index {
            0 => Self::Fast,
            2 => Self::High,
            _ => Self::Balanced,
        }
    }
```

```rust
pub fn recording_size(aspect: PreviewAspect) -> (u32, u32) {
    match aspect {
        PreviewAspect::Auto | PreviewAspect::Wide16x9 => (1920, 1080),
        PreviewAspect::Classic4x3 => (1440, 1080),
        PreviewAspect::Cinema21x9 => (2520, 1080),
    }
}

pub fn recording_folder(saved: Option<&Path>, first_left: &Path) -> PathBuf {
    let folder = saved
        .filter(|p| p.is_dir())
        .map(Path::to_path_buf)
        .or_else(|| first_left.parent().filter(|p| !p.as_os_str().is_empty()).map(Path::to_path_buf))
        .unwrap_or_else(|| PathBuf::from("."));
    std::fs::canonicalize(&folder).unwrap_or(folder)
}

pub fn recording_file_name(unix_secs: u64) -> String {
    format!("reco_recording_{unix_secs}.mp4")
}
```

`settings.rs`:

```rust
impl Default for DesktopSettings {
    fn default() -> Self {
        Self {
            preview_aspect: PreviewAspect::Auto.name().into(),
            recording_quality: RecordingQuality::Balanced.name().into(),
            recording_folder: None,
        }
    }
}

impl DesktopSettings {
    pub fn aspect(&self) -> PreviewAspect {
        PreviewAspect::from_name(&self.preview_aspect)
    }

    pub fn set_aspect(&mut self, aspect: PreviewAspect) {
        self.preview_aspect = aspect.name().into();
    }

    pub fn quality(&self) -> RecordingQuality {
        RecordingQuality::from_name(&self.recording_quality)
    }

    pub fn set_quality(&mut self, quality: RecordingQuality) {
        self.recording_quality = quality.name().into();
    }
}
```

- [ ] **Step 5: Run the tests**

Run: `cargo test -p reco-app -- names round_trip recordings settings`
Expected: PASS (9 tests). Then run `cargo clippy -p reco-app --no-deps --all-targets -- -D warnings`. Expected: clean.

- [ ] **Step 6: Commit**

```bash
git add Cargo.lock crates/reco-app
git commit -m "feat(app): desktop settings and recording choices"
```

---

### Task 3: The toasts model (reco-app)

**Files:**
- Create: `crates/reco-app/src/toasts.rs`
- Modify: `crates/reco-app/src/lib.rs` (`pub mod toasts;`)

**Interfaces:**
- Produces:
  - `toasts::Severity { Info, Warn, Error }`, with `ttl(self) -> Duration` (4, 7, 10 s).
  - `toasts::Toast { pub id: u64, pub severity: Severity, pub title: String, pub body: String, pub expires_at: Instant }`.
  - `toasts::MAX_VISIBLE: usize = 4`.
  - `toasts::Toasts::default()`, with:
    - `push(&mut self, Severity, title: impl Into<String>, body: impl Into<String>, now: Instant) -> u64`
    - `push_for(&mut self, Severity, title, body, ttl: Duration, now: Instant) -> u64`
    - `dismiss(&mut self, id: u64) -> bool`
    - `expire(&mut self, now: Instant) -> bool`
    - `visible(&self) -> &[Toast]` (oldest first)
    - `next_expiry(&self) -> Option<Instant>`

- [ ] **Step 1: Write the failing tests**

`src/toasts.rs`:

```rust
//! Toasts: short notices in the viewer's corner (PARITY.md Module 2). Info
//! stays 4 s, a warning 7 s, an error 10 s, or a custom time. At most four
//! show, the oldest leaving first and the newest at the bottom. A notice
//! identical to one on screen refreshes it instead of stacking. Pure: the
//! caller passes the time.

use std::time::{Duration, Instant};

/// How serious a toast is: its dot colour and its default time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Severity {
    /// Something happened.
    Info,
    /// Worth a look.
    Warn,
    /// Something failed.
    Error,
}

impl Severity {
    /// How long a toast of this severity stays.
    pub fn ttl(self) -> Duration {
        todo!()
    }
}

/// One toast.
#[derive(Clone, Debug, PartialEq)]
pub struct Toast {
    /// Its id, for dismissing it.
    pub id: u64,
    /// How serious it is.
    pub severity: Severity,
    /// One line.
    pub title: String,
    /// The detail; may be empty.
    pub body: String,
    /// When it leaves.
    pub expires_at: Instant,
}

/// Toasts on screen at most.
pub const MAX_VISIBLE: usize = 4;

/// The toasts on screen, oldest first.
#[derive(Clone, Debug, Default)]
pub struct Toasts {
    shown: Vec<Toast>,
    next_id: u64,
}

impl Toasts {
    /// Show a toast for its severity's time; its id.
    pub fn push(&mut self, severity: Severity, title: impl Into<String>, body: impl Into<String>, now: Instant) -> u64 {
        self.push_for(severity, title, body, severity.ttl(), now)
    }

    /// Show a toast for `ttl`; its id. An identical toast on screen is
    /// refreshed and moved to the end instead.
    pub fn push_for(
        &mut self,
        severity: Severity,
        title: impl Into<String>,
        body: impl Into<String>,
        ttl: Duration,
        now: Instant,
    ) -> u64 {
        todo!()
    }

    /// Remove the toast `id`; whether it was there.
    pub fn dismiss(&mut self, id: u64) -> bool {
        todo!()
    }

    /// Remove the toasts due by `now`; whether any left.
    pub fn expire(&mut self, now: Instant) -> bool {
        todo!()
    }

    /// The toasts on screen, oldest first.
    pub fn visible(&self) -> &[Toast] {
        &self.shown
    }

    /// When the next toast leaves.
    pub fn next_expiry(&self) -> Option<Instant> {
        self.shown.iter().map(|t| t.expires_at).min()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn titles(t: &Toasts) -> Vec<&str> {
        t.visible().iter().map(|t| t.title.as_str()).collect()
    }

    #[test]
    fn ttls_follow_the_severity() {
        assert_eq!(Severity::Info.ttl(), Duration::from_secs(4));
        assert_eq!(Severity::Warn.ttl(), Duration::from_secs(7));
        assert_eq!(Severity::Error.ttl(), Duration::from_secs(10));
    }

    #[test]
    fn at_most_four_newest_last() {
        let now = Instant::now();
        let mut t = Toasts::default();
        for title in ["a", "b", "c", "d", "e"] {
            t.push(Severity::Info, title, "", now);
        }
        assert_eq!(titles(&t), ["b", "c", "d", "e"]);
    }

    #[test]
    fn expire_removes_only_the_due() {
        let now = Instant::now();
        let mut t = Toasts::default();
        t.push(Severity::Info, "info", "", now);
        t.push(Severity::Error, "error", "", now);
        assert!(!t.expire(now + Duration::from_secs(3)));
        assert!(t.expire(now + Duration::from_secs(5)));
        assert_eq!(titles(&t), ["error"]);
        assert_eq!(t.next_expiry(), Some(now + Duration::from_secs(10)));
    }

    #[test]
    fn dismiss_removes_one() {
        let now = Instant::now();
        let mut t = Toasts::default();
        let a = t.push(Severity::Info, "a", "", now);
        t.push(Severity::Info, "b", "", now);
        assert!(t.dismiss(a));
        assert!(!t.dismiss(a));
        assert_eq!(titles(&t), ["b"]);
    }

    #[test]
    fn a_repeat_refreshes_instead_of_stacking() {
        let now = Instant::now();
        let mut t = Toasts::default();
        let first = t.push(Severity::Error, "Couldn't seek", "x", now);
        t.push(Severity::Info, "other", "", now);
        let later = now + Duration::from_secs(5);
        let again = t.push(Severity::Error, "Couldn't seek", "x", later);
        assert_eq!(again, first);
        assert_eq!(titles(&t), ["other", "Couldn't seek"]);
        assert_eq!(t.visible()[1].expires_at, later + Duration::from_secs(10));
    }

    #[test]
    fn custom_times_are_kept() {
        let now = Instant::now();
        let mut t = Toasts::default();
        t.push_for(Severity::Info, "Recording saved", "", Duration::from_secs(8), now);
        assert_eq!(t.next_expiry(), Some(now + Duration::from_secs(8)));
    }
}
```

`lib.rs`: add `pub mod toasts;`.

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test -p reco-app toasts`
Expected: FAIL (`not yet implemented`).

- [ ] **Step 3: Implement**

```rust
    pub fn ttl(self) -> Duration {
        Duration::from_secs(match self {
            Self::Info => 4,
            Self::Warn => 7,
            Self::Error => 10,
        })
    }
```

```rust
    pub fn push_for(
        &mut self,
        severity: Severity,
        title: impl Into<String>,
        body: impl Into<String>,
        ttl: Duration,
        now: Instant,
    ) -> u64 {
        let (title, body) = (title.into(), body.into());
        let same = |t: &Toast| t.severity == severity && t.title == title && t.body == body;
        if let Some(index) = self.shown.iter().position(same) {
            let mut toast = self.shown.remove(index);
            toast.expires_at = now + ttl;
            let id = toast.id;
            self.shown.push(toast);
            return id;
        }
        self.next_id += 1;
        let id = self.next_id;
        self.shown.push(Toast { id, severity, title, body, expires_at: now + ttl });
        if self.shown.len() > MAX_VISIBLE {
            self.shown.remove(0);
        }
        id
    }

    pub fn dismiss(&mut self, id: u64) -> bool {
        let before = self.shown.len();
        self.shown.retain(|t| t.id != id);
        self.shown.len() != before
    }

    pub fn expire(&mut self, now: Instant) -> bool {
        let before = self.shown.len();
        self.shown.retain(|t| t.expires_at > now);
        self.shown.len() != before
    }
```

- [ ] **Step 4: Run the tests**

Run: `cargo test -p reco-app toasts`
Expected: PASS (6 tests).

- [ ] **Step 5: Commit**

```bash
git add crates/reco-app/src
git commit -m "feat(app): toasts model with times, a cap of four and refreshed repeats"
```

---
### Task 4: Absolute seeks, coalesced; failed seeks; the play state; crash reports (reco-app)

**Files:**
- Modify: `crates/reco-app/src/preview/playback.rs`
- Modify: `crates/reco-app/src/preview/worker.rs`
- Modify: `crates/reco-desktop/src/session_view.rs` (the `Time` event's new shape)

**Interfaces:**
- Consumes: `seek_target` (Module 1).
- Produces:
  - `playback::seek_goal(from: u64, seconds: f64, fps: f64, total: Option<u64>) -> u64`.
  - `Playback::seek_to_frame(&mut self, frame: u64) -> Result<(), SourceError>`. It clamps to the last frame, and errors when the videos end before the frame; the frame on screen then stays.
  - `PreviewCommand::SeekTo { frame: u64 }`.
  - `PreviewEvent::Time { frame: u64, state: PlayState }`, replacing `playing: bool`.
  - Seeks in one batch of commands collapse into one, and `SeekBy` counts from a pending target.
  - A worker panic sends `PreviewEvent::Stopped("The preview stopped unexpectedly: …")`.
  - Test-only `PreviewCommand::Crash`.

- [ ] **Step 1: Write the failing tests**

Append to `playback.rs`'s tests:

```rust
    #[test]
    fn relative_seeks_accumulate() {
        // Five presses of ] at 30 fps from the first frame: 25 s on.
        let mut at = 0;
        for _ in 0..5 {
            at = seek_goal(at, 5.0, 30.0, Some(1800));
        }
        assert_eq!(at, 750);
        assert_eq!(seek_goal(1790, 5.0, 30.0, Some(1800)), 1799);
        assert_eq!(seek_goal(10, -5.0, 30.0, None), 0);
    }

    #[test]
    fn seek_to_frame_shows_that_frame() {
        let Some((left, right, _)) = crate::preview::fixtures::fast_set() else { return };
        let mut playback = Playback::new();
        playback.open(&InputPath::Single(left), &InputPath::Single(right), 0).unwrap();
        playback.seek_to_frame(300).unwrap();
        assert_eq!(playback.frame_index(), 301);
        // Past the length: the last frame.
        playback.seek_to_frame(99_999).unwrap();
        assert_eq!(Some(playback.frame_index()), playback.total_frames());
    }

    #[test]
    fn seek_past_the_end_is_an_error() {
        let Some((left, right, _)) = crate::preview::fixtures::fast_set() else { return };
        let mut playback = Playback::new();
        playback.open(&InputPath::Single(left), &InputPath::Single(right), 0).unwrap();
        let before = playback.frame_index();
        // Past the real end, as a sync offset can make the engine's total.
        assert!(playback.seek_to(99_999).is_err());
        assert_eq!(playback.frame_index(), before, "the frame on screen stays");
        assert_eq!(playback.state(), PlayState::Paused);
    }
```

Add stubs above the tests:

```rust
/// The frame a seek of `seconds` reaches from `from` (0-based) at `fps`,
/// inside `total` frames when known. Seeks in a burst chain through this
/// from the pending target, so they cost one seek.
pub fn seek_goal(from: u64, seconds: f64, fps: f64, total: Option<u64>) -> u64 {
    todo!()
}
```

```rust
    /// Show `frame` (0-based), kept inside the videos (blocking: decodes).
    pub fn seek_to_frame(&mut self, frame: u64) -> Result<(), SourceError> {
        todo!()
    }
```

In `worker.rs`'s tests, change the play check in `readback_renders_and_plays` to the new shape:

```rust
        let moved = wait_for(&worker, 10, |e| {
            matches!(e, PreviewEvent::Time { frame, state: PlayState::Playing } if *frame >= 10)
        });
```

and append:

```rust
    #[test]
    fn seek_burst_lands_on_the_sum() {
        let Some((left, right, cal)) = fixtures::fast_set() else { return };
        let worker = readback_worker();
        worker.send(PreviewCommand::Resize { width: 320, height: 180 });
        worker.send(PreviewCommand::Open { left: InputPath::Single(left), right: InputPath::Single(right), calibration: cal });
        assert!(wait_for(&worker, 30, |e| matches!(e, PreviewEvent::Ready(_))).is_some());
        for _ in 0..5 {
            worker.send(PreviewCommand::SeekBy { seconds: 5.0 });
        }
        let landed = wait_for(&worker, 20, |e| matches!(e, PreviewEvent::Time { frame, .. } if *frame >= 751));
        assert!(matches!(landed, Some(PreviewEvent::Time { frame: 751, .. })), "{landed:?}");
    }

    #[test]
    fn a_crash_is_reported() {
        let worker = readback_worker();
        worker.send(PreviewCommand::Crash);
        let event = wait_for(&worker, 5, |e| matches!(e, PreviewEvent::Stopped(_)));
        assert!(matches!(&event, Some(PreviewEvent::Stopped(m)) if m.contains("stopped unexpectedly")), "{event:?}");
    }
```

Add the command variants (the tests reference them):

```rust
    /// Show this frame (0-based). Seeks go by frame index, never by a
    /// fraction of the length.
    SeekTo {
        /// The frame.
        frame: u64,
    },
    /// Panic on the render thread (tests of the crash report).
    #[cfg(test)]
    Crash,
```

Change `PreviewEvent::Time` to:

```rust
    /// The playhead moved, or the play state changed.
    Time {
        /// Frames taken so far (the frame on screen is `frame - 1`).
        frame: u64,
        /// Playing, paused or finished.
        state: PlayState,
    },
```

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test -p reco-app -- --test-threads=1 seek crash readback_renders`
Expected: compile errors in `worker.rs` (the `Time` construction and `last_time` still use `playing`; there are no `SeekTo`/`Crash` arms). Then, once Step 3's worker part compiles, the stubs fail with `not yet implemented`.

- [ ] **Step 3: Implement**

`playback.rs`:

```rust
pub fn seek_goal(from: u64, seconds: f64, fps: f64, total: Option<u64>) -> u64 {
    let delta = (seconds * fps).round() as i64;
    match total {
        Some(total) => seek_target(from, delta, total),
        None => (from as i64 + delta).max(0) as u64,
    }
}
```

```rust
    pub fn seek_to_frame(&mut self, frame: u64) -> Result<(), SourceError> {
        let frame = self.total_frames.map_or(frame, |total| frame.min(total.saturating_sub(1)));
        self.seek_to(frame)
    }
```

Replace `seek_to` with:

```rust
    fn seek_to(&mut self, frame: u64) -> Result<(), SourceError> {
        let Some(source) = self.source.as_mut() else { return Ok(()) };
        let before = self.frame_index;
        source.seek(frame)?;
        self.frame_index = frame;
        self.clock.reset();
        if self.state == PlayState::Finished {
            self.state = PlayState::Paused;
        }
        if !self.step_forward()? {
            // The engine reports a seek it could not make as the end of the
            // stream: keep the frame on screen and say so.
            self.frame_index = before;
            self.state = PlayState::Paused;
            return Err(SourceError::Read { reason: format!("the videos end before frame {frame}") });
        }
        Ok(())
    }
```

`worker.rs`:
1. `#[derive(Clone)]` on `struct Outbox`.
2. The worker's `last_time` becomes `(u64, PlayState)`, initialised to `(0, PlayState::Empty)`.
3. Add the field `pending_seek: Option<u64>` (initialised to `None`), documented "A seek waiting for the end of this batch of commands".
4. In `step()`, build the time as `let time = (session.playback().frame_index(), session.playback().state());` and send `PreviewEvent::Time { frame: time.0, state: time.1 }`.
5. In `apply`, replace the `SeekBy` arm and add the new ones:

```rust
            PreviewCommand::SeekTo { frame } => self.pending_seek = Some(frame),
            PreviewCommand::SeekBy { seconds } => {
                if let Some(s) = self.session.as_ref() {
                    let playback = s.playback();
                    let from = self.pending_seek.unwrap_or(playback.frame_index().saturating_sub(1));
                    self.pending_seek = Some(seek_goal(from, seconds, playback.fps(), playback.total_frames()));
                }
            }
            #[cfg(test)]
            PreviewCommand::Crash => panic!("a test crash"),
```

6. In `run`, after the `while let Ok(c) = commands.try_recv()` loop and before the step:

```rust
            if let Some(frame) = self.pending_seek.take() {
                self.transport(|s| s.playback_mut().seek_to_frame(frame));
            }
```

7. In `PreviewWorker::spawn`, report panics:

```rust
        let report = outbox.clone();
        std::thread::Builder::new()
            .name("reco-preview".into())
            .spawn(move || {
                let run = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
                    Worker::new(config, outbox).run(command_rx)
                }));
                if let Err(panic) = run {
                    let reason = panic
                        .downcast_ref::<&str>()
                        .map(|s| s.to_string())
                        .or_else(|| panic.downcast_ref::<String>().cloned())
                        .unwrap_or_default();
                    report.send(PreviewEvent::Stopped(format!("The preview stopped unexpectedly: {reason}")));
                }
            })
            .expect("spawn the preview thread");
```

Import `seek_goal` from `super::playback`.

`reco-desktop/src/session_view.rs`: import `reco_app::preview::playback::PlayState` and change the arm in `drain_preview`:

```rust
                Some(PreviewEvent::Time { frame, state }) => {
                    self.show_time(cx, frame, state == PlayState::Playing)
                }
```

- [ ] **Step 4: Run the tests**

Run: `cargo test -p reco-app -- --test-threads=1 && cargo build -p reco-desktop`
Expected: all reco-app tests pass, including the 5 new ones (`relative_seeks_accumulate`, `seek_to_frame_shows_that_frame`, `seek_past_the_end_is_an_error`, `seek_burst_lands_on_the_sum` and `a_crash_is_reported`); reco-desktop builds.

- [ ] **Step 5: Commit**

```bash
git add crates/reco-app/src crates/reco-desktop/src/session_view.rs
git commit -m "feat(app): absolute, coalesced seeks; failed seeks reported; play state in Time; crash reports"
```

---

### Task 5: Camera lanes from the files (reco-app)

**Files:**
- Create: `crates/reco-app/src/preview/lanes.rs`
- Modify: `crates/reco-app/src/preview/mod.rs` (`pub mod lanes;`), `playback.rs`, `session.rs`, `worker.rs`

**Interfaces:**
- Produces:
  - `lanes::Lanes { pub left: Vec<(f64, f64)>, pub right: Vec<(f64, f64)>, pub length: f64 }` (Clone, Debug, PartialEq).
  - `lanes::camera_spans(durations: &[f64], skip: f64) -> Vec<(f64, f64)>`.
  - `lanes::skips(sync_offset: i64, fps: f64) -> (f64, f64)` (left, right).
  - `lanes::lanes(left: &[f64], right: &[f64], sync_offset: i64, fps: f64) -> Lanes`.
  - `lanes::probe(input: &InputPath) -> Vec<f64>` (blocking).
  - `Playback::set_total_frames(&mut self, frames: u64)`.
  - `PreviewSession::sync_offset(&self) -> i64`.
  - `PreviewEvent::Lanes(Lanes)`: sent once per open, after a probe thread measured every file. The worker also sets the playback length from it.

- [ ] **Step 1: Write the failing tests**

`preview/lanes.rs`:

```rust
//! Where each camera's files sit on the stitched timeline, for the time
//! panel's lanes. Every file's duration is probed off the render thread and
//! laid end to end, shifted by the frames the sync offset trims from the
//! camera that started first. The pair plays only while both cameras have
//! video, which also fixes the engine's length estimate (it ignores the
//! sync offset).

use std::path::PathBuf;

use reco_io::ffmpeg::decoder::VideoDecoder;
use reco_io::stitch_job::InputPath;

/// Each camera's files on the stitched timeline, and how long the pair plays.
#[derive(Clone, Debug, PartialEq)]
pub struct Lanes {
    /// The left camera's files as (start, end) seconds.
    pub left: Vec<(f64, f64)>,
    /// The right camera's files.
    pub right: Vec<(f64, f64)>,
    /// Seconds both cameras have video.
    pub length: f64,
}

/// Files of `durations` seconds laid end to end, less `skip` seconds the sync
/// offset trims from this camera's start. A file wholly before zero is
/// dropped and the first one kept is clipped at zero.
pub fn camera_spans(durations: &[f64], skip: f64) -> Vec<(f64, f64)> {
    todo!()
}

/// The seconds each camera skips for `sync_offset` frames at `fps`: a
/// positive offset skips the right camera's start (it started first), a
/// negative one the left's.
pub fn skips(sync_offset: i64, fps: f64) -> (f64, f64) {
    todo!()
}

/// Both cameras' lanes from their files' durations.
pub fn lanes(left: &[f64], right: &[f64], sync_offset: i64, fps: f64) -> Lanes {
    todo!()
}

/// Each file's duration in seconds; a file that cannot be read counts as
/// zero. Blocking (it opens every file): run it off the render thread.
pub fn probe(input: &InputPath) -> Vec<f64> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn files_lie_end_to_end() {
        assert_eq!(camera_spans(&[60.0, 40.0], 0.0), [(0.0, 60.0), (60.0, 100.0)]);
    }

    #[test]
    fn the_sync_offset_trims_the_camera_that_started_first() {
        assert_eq!(skips(30, 30.0), (0.0, 1.0));
        assert_eq!(skips(-15, 30.0), (0.5, 0.0));
        assert_eq!(camera_spans(&[60.0], 1.0), [(0.0, 59.0)]);
    }

    #[test]
    fn a_file_wholly_before_the_start_is_dropped() {
        assert_eq!(camera_spans(&[1.0, 10.0], 2.0), [(0.0, 9.0)]);
    }

    #[test]
    fn length_is_where_both_cameras_have_video() {
        let l = lanes(&[60.0, 60.0], &[100.0], 30, 30.0);
        assert_eq!(l.right, [(0.0, 99.0)]);
        assert_eq!(l.length, 99.0);
        assert_eq!(lanes(&[], &[10.0], 0, 30.0).length, 0.0);
    }

    #[test]
    fn probing_the_fixture_gives_its_duration() {
        let Some((left, _, _)) = crate::preview::fixtures::fast_set() else { return };
        let durations = probe(&InputPath::Single(left));
        assert_eq!(durations.len(), 1);
        assert!((durations[0] - 60.0).abs() < 0.5, "{durations:?}");
        assert_eq!(probe(&InputPath::Single("/nonexistent.mp4".into())), [0.0]);
    }
}
```

`preview/mod.rs`: add `pub mod lanes;`.

Add the worker test:

```rust
    #[test]
    fn lanes_arrive_after_open() {
        let Some((left, right, cal)) = fixtures::fast_set() else { return };
        let worker = readback_worker();
        worker.send(PreviewCommand::Resize { width: 320, height: 180 });
        worker.send(PreviewCommand::Open { left: InputPath::Single(left), right: InputPath::Single(right), calibration: cal });
        let event = wait_for(&worker, 30, |e| matches!(e, PreviewEvent::Lanes(_) | PreviewEvent::Failed(_)));
        let Some(PreviewEvent::Lanes(lanes)) = event else { panic!("{event:?}") };
        assert_eq!((lanes.left.len(), lanes.right.len()), (1, 1));
        assert!((lanes.length - 60.0).abs() < 0.5, "{lanes:?}");
    }
```

with the event variant:

```rust
    /// Each camera's files on the timeline and the playable length (once
    /// per open, after the files were measured).
    Lanes(Lanes),
```

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test -p reco-app -- --test-threads=1 lanes files_lie sync_offset wholly length_is probing`
Expected: FAIL. The pure tests panic with `not yet implemented`; `lanes_arrive_after_open` times out (`None`).

- [ ] **Step 3: Implement**

`lanes.rs`:

```rust
pub fn camera_spans(durations: &[f64], skip: f64) -> Vec<(f64, f64)> {
    let mut start = -skip;
    let mut spans = Vec::new();
    for duration in durations {
        let end = start + duration.max(0.0);
        if end > 0.0 {
            spans.push((start.max(0.0), end));
        }
        start = end;
    }
    spans
}

pub fn skips(sync_offset: i64, fps: f64) -> (f64, f64) {
    if fps <= 0.0 {
        return (0.0, 0.0);
    }
    ((-sync_offset).max(0) as f64 / fps, sync_offset.max(0) as f64 / fps)
}

pub fn lanes(left: &[f64], right: &[f64], sync_offset: i64, fps: f64) -> Lanes {
    let (left_skip, right_skip) = skips(sync_offset, fps);
    let left = camera_spans(left, left_skip);
    let right = camera_spans(right, right_skip);
    let end = |spans: &[(f64, f64)]| spans.last().map_or(0.0, |s| s.1);
    let length = end(&left).min(end(&right));
    Lanes { left, right, length }
}

pub fn probe(input: &InputPath) -> Vec<f64> {
    let paths: Vec<PathBuf> = match input {
        InputPath::Single(path) => vec![path.clone()],
        InputPath::Chained(paths) => paths.clone(),
    };
    paths
        .iter()
        .map(|path| VideoDecoder::open(path).ok().and_then(|d| d.duration_secs()).unwrap_or(0.0))
        .collect()
}
```

`playback.rs`:

```rust
    /// Replace the engine's length estimate (it ignores the sync offset)
    /// with the probed one.
    pub fn set_total_frames(&mut self, frames: u64) {
        self.total_frames = Some(frames);
    }
```

`session.rs`:

```rust
    /// The calibration's sync offset in frames (positive: the right camera
    /// started first).
    pub fn sync_offset(&self) -> i64 {
        self.renderer.calibration().sync_offset
    }
```

`worker.rs`:
1. Add `use super::lanes::{self, Lanes};`, `const PROBE_POLL: Duration = Duration::from_millis(50);` (documented "How often the worker looks for the file probe's answer"), and the field `lanes_rx: Option<Receiver<Lanes>>` (initialised to `None`).
2. In `open()`'s `Ok(session)` arm, before `self.session = Some(session);`:

```rust
                let (tx, rx) = mpsc::channel();
                let (left, right) = (left.clone(), right.clone());
                let (sync_offset, fps) = (session.sync_offset(), session.playback().fps());
                let probe = std::thread::Builder::new().name("reco-probe".into()).spawn(move || {
                    let _ = tx.send(lanes::lanes(&lanes::probe(&left), &lanes::probe(&right), sync_offset, fps));
                });
                self.lanes_rx = probe.is_ok().then_some(rx);
```

3. Add:

```rust
    /// Take the probe's answer when it has come: the exact length goes to
    /// playback, the lanes to the UI.
    fn collect_lanes(&mut self) {
        let Some(rx) = self.lanes_rx.as_ref() else { return };
        match rx.try_recv() {
            Ok(lanes) => {
                self.lanes_rx = None;
                if let Some(s) = self.session.as_mut() {
                    let frames = (lanes.length * s.playback().fps()).floor() as u64;
                    if frames > 0 {
                        s.playback_mut().set_total_frames(frames);
                    }
                }
                self.out.send(PreviewEvent::Lanes(lanes));
            }
            Err(mpsc::TryRecvError::Empty) => {}
            Err(mpsc::TryRecvError::Disconnected) => self.lanes_rx = None,
        }
    }
```

4. In `run`, call `self.collect_lanes();` after the pending seek and before `self.step(dt)`.
5. Replace `wake_after` so the worker also wakes for the probe:

```rust
    fn wake_after(&self) -> Option<Duration> {
        let session = self.session.as_ref()?;
        let next_frame = session.playback().until_next_frame().map(|d| d.max(POLL_FLOOR));
        let sooner = |wait: Option<Duration>, other: Duration| Some(wait.map_or(other, |w| w.min(other)));
        let mut wait = next_frame;
        if self.dirty || self.easing {
            wait = sooner(wait, ANIMATION_TICK);
        }
        if self.lanes_rx.is_some() {
            wait = sooner(wait, PROBE_POLL);
        }
        wait
    }
```

- [ ] **Step 4: Run the tests**

Run: `cargo test -p reco-app -- --test-threads=1 && cargo build -p reco-desktop`
Expected: all pass, including the 6 new ones; reco-desktop builds (its event match has a catch-all).

- [ ] **Step 5: Commit**

```bash
git add crates/reco-app/src
git commit -m "feat(app): camera lanes and the playable length from the files"
```

---

### Task 6: The recorder (reco-app)

**Files:**
- Create: `crates/reco-app/src/preview/recorder.rs`
- Modify: `crates/reco-app/src/preview/mod.rs` (`pub mod recorder;`), `session.rs`, `playback.rs`

**Interfaces:**
- Consumes: `RecordingQuality`, `RECORDING_CODEC` (Task 2); `StereoYuv` (Module 1).
- Produces:
  - `session::build_renderer(gpu: GpuContext, calibration: MatchCalibration, input: (u32, u32), size: (u32, u32), format: wgpu::TextureFormat) -> Result<StitchRenderer, SessionError>` (`pub(crate)`).
  - `SessionError::Record(String)`.
  - `Playback::fps_rational(&self) -> (i32, i32)`.
  - `recorder::Recording { pub path: PathBuf, pub frames: u64 }` (Clone, Debug, PartialEq).
  - `recorder::Recorder::start(gpu, calibration, input: (u32, u32), size: (u32, u32), fps: (i32, i32), path: &Path, quality: RecordingQuality) -> Result<Recorder, String>`, with:
    - `record(&mut self, frame: &StereoYuv, pose: ViewportPosition) -> Result<(), String>`
    - `finish(self) -> Result<Recording, String>`
  - `PreviewSession`:
    - `start_recording(&mut self, path: &Path, size: (u32, u32), quality: RecordingQuality) -> Result<(), SessionError>`
    - `is_recording(&self) -> bool`
    - `record_frame(&mut self) -> Result<(), SessionError>`
    - `stop_recording(&mut self) -> Option<Result<Recording, SessionError>>`

- [ ] **Step 1: Write the failing tests**

`preview/recorder.rs`, the module docs, types and tests first:

```rust
//! Recording the preview (Record in the view bar). A second renderer at a
//! fixed size draws each source frame shown with the preview's pose; the GPU
//! converts it to NV12 and reads it back; an encoder thread writes it.
//!
//! One frame per source frame shown, so the file plays at the source rate
//! whatever the display does, and nothing is recorded while paused. No frame
//! is dropped: the render thread waits when the encoder's queue is full.
//! The readback runs two frames behind, so finishing flushes it. The
//! renderer stays on the render thread (its GPU types are not `Send`); only
//! the encoder moves. The preview's own renderer is not reused: its render
//! target keeps its first size when the preview resizes.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, SyncSender};
use std::thread::JoinHandle;

use reco_core::calibration::MatchCalibration;
use reco_core::detect::director::ViewportPosition;
use reco_core::encoder::{Encoder, OutputFrame, PixelFormat};
use reco_core::gpu::GpuContext;
use reco_core::lens::rig_correction::render_pitch;
use reco_core::render::stitch_renderer::StitchRenderer;
use reco_core::wgpu;
use reco_io::adapters::create_encoder;

use super::playback::StereoYuv;
use super::session::{FOV_DEFAULT, build_renderer};
use crate::recording::{RECORDING_CODEC, RecordingQuality};

/// Frames queued for the encoder before the render thread waits for it.
const ENCODE_QUEUE: usize = 8;

/// A finished recording.
#[derive(Clone, Debug, PartialEq)]
pub struct Recording {
    /// The file.
    pub path: PathBuf,
    /// Frames written.
    pub frames: u64,
}

/// A recording in progress.
pub struct Recorder {
    renderer: StitchRenderer,
    size: (u32, u32),
    frames: Option<SyncSender<Vec<u8>>>,
    encoder: Option<JoinHandle<Result<u64, String>>>,
    path: PathBuf,
}

#[cfg(test)]
mod tests {
    use reco_io::ffmpeg::decoder::VideoDecoder;
    use reco_io::stitch_job::InputPath;

    use super::*;
    use crate::preview::fixtures;
    use crate::preview::session::PreviewSession;

    fn temp_video(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("reco-app-{name}-{}.mp4", std::process::id()))
    }

    fn open_session() -> Option<PreviewSession> {
        let (left, right, cal) = fixtures::fast_set()?;
        let gpu = GpuContext::new_blocking().ok()?;
        PreviewSession::open(gpu, &InputPath::Single(left), &InputPath::Single(right), &cal, (320, 180)).ok()
    }

    #[test]
    fn recording_has_one_frame_per_source_frame() {
        let Some(mut session) = open_session() else { return };
        let path = temp_video("one-per-frame");
        session.start_recording(&path, (640, 360), RecordingQuality::Fast).expect("start");
        for i in 0..15 {
            if i > 0 {
                session.playback_mut().step_forward().unwrap();
            }
            session.record_frame().expect("record");
        }
        let recording = session.stop_recording().expect("was recording").expect("finish");
        assert_eq!(recording.frames, 15, "every frame reaches the file, the readback's last two too");
        let video = VideoDecoder::open(&path).expect("a playable file");
        assert_eq!((video.width(), video.height()), (640, 360));
        let secs = video.duration_secs().unwrap_or(0.0);
        assert!((secs - 0.5).abs() < 0.07, "15 frames at 30 fps last 0.5 s, not {secs}");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn recording_keeps_its_size_when_the_preview_resizes() {
        let Some(mut session) = open_session() else { return };
        let path = temp_video("fixed-size");
        session.start_recording(&path, (640, 360), RecordingQuality::Fast).expect("start");
        session.record_frame().unwrap();
        session.resize(1000, 300);
        session.playback_mut().step_forward().unwrap();
        session.record_frame().unwrap();
        session.stop_recording().unwrap().unwrap();
        let video = VideoDecoder::open(&path).expect("a playable file");
        assert_eq!((video.width(), video.height()), (640, 360));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn a_folder_that_does_not_exist_fails_to_start() {
        let Some(mut session) = open_session() else { return };
        let err = session
            .start_recording(Path::new("/nonexistent/folder/x.mp4"), (640, 360), RecordingQuality::Fast)
            .err()
            .expect("no folder, no recording");
        assert!(err.to_string().starts_with("Couldn't record"), "{err}");
        assert!(!session.is_recording());
    }
}
```

`preview/mod.rs`: add `pub mod recorder;`.

In `session.rs`, add the `SessionError::Record(String)` variant with the Display arm `Self::Record(e) => write!(f, "Couldn't record: {e}")`, the field `recorder: Option<Recorder>` (initialised `None` in `open`), and stubs:

```rust
    /// Start recording to `path` at `size` (blocking for a moment while the
    /// encoder opens: run on the worker).
    pub fn start_recording(&mut self, path: &Path, size: (u32, u32), quality: RecordingQuality) -> Result<(), SessionError> {
        todo!()
    }

    /// Whether a recording is running.
    pub fn is_recording(&self) -> bool {
        self.recorder.is_some()
    }

    /// Record the frame on screen with the current pose. The worker calls
    /// this once per new source frame.
    pub fn record_frame(&mut self) -> Result<(), SessionError> {
        todo!()
    }

    /// Stop: flush, close the file, report it (`None` when not recording).
    pub fn stop_recording(&mut self) -> Option<Result<Recording, SessionError>> {
        todo!()
    }
```

with `use std::path::Path;` (already there), `use super::recorder::{Recorder, Recording};` and `use crate::recording::RecordingQuality;`.

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test -p reco-app -- --test-threads=1 recording a_folder`
Expected: FAIL (`not yet implemented`).

- [ ] **Step 3: Implement**

`session.rs`: pull the renderer set-up out of `open` into a shared function and use it there:

```rust
/// Reco's renderer for `calibration` at `size`, set up as the preview uses
/// it: the calibration's lens correction, blend, tilt and roll, and colour
/// match on.
pub(crate) fn build_renderer(
    gpu: GpuContext,
    calibration: MatchCalibration,
    input: (u32, u32),
    size: (u32, u32),
    format: wgpu::TextureFormat,
) -> Result<StitchRenderer, SessionError> {
    let lens_correction = calibration.lens_correction_amount;
    let viewport = ViewportConfig {
        width: size.0,
        height: size.1,
        fov_degrees: FOV_DEFAULT,
        blend_width: calibration.blend_width,
        rig_tilt: calibration.rig_tilt as f32,
        rig_roll: calibration.rig_roll as f32,
        ..ViewportConfig::default()
    };
    let mut renderer = StitchRenderer::new(calibration, gpu, viewport, input.0, input.1, format, InputFormat::Yuv420p)
        .map_err(|e| SessionError::Render(e.to_string()))?;
    renderer.pipeline_mut().set_lens_correction_amount(lens_correction);
    renderer.set_color_match(true);
    Ok(renderer)
}
```

In `open`, replace the viewport and renderer lines with `let renderer = build_renderer(gpu, calibration, (input_w, input_h), size, OUTPUT_FORMAT)?;` and drop the `lens_correction` local. Then:

```rust
    pub fn start_recording(&mut self, path: &Path, size: (u32, u32), quality: RecordingQuality) -> Result<(), SessionError> {
        let input = self.playback.input_dimensions().ok_or(SessionError::NoFrame)?;
        let recorder = Recorder::start(
            self.gpu().clone(),
            self.renderer.calibration().clone(),
            input,
            size,
            self.playback.fps_rational(),
            path,
            quality,
        )
        .map_err(SessionError::Record)?;
        self.recorder = Some(recorder);
        Ok(())
    }

    pub fn record_frame(&mut self) -> Result<(), SessionError> {
        let (Some(recorder), Some(frame)) = (self.recorder.as_mut(), self.playback.current_frame()) else {
            return Ok(());
        };
        recorder.record(frame, self.pose.current_pose()).map_err(SessionError::Record)
    }

    pub fn stop_recording(&mut self) -> Option<Result<Recording, SessionError>> {
        self.recorder.take().map(|r| r.finish().map_err(SessionError::Record))
    }
```

`playback.rs`:

```rust
    /// The exact source rate as a fraction (30000/1001 for 29.97), for the
    /// encoder; the rounded rate when the source does not say.
    pub fn fps_rational(&self) -> (i32, i32) {
        self.info
            .as_ref()
            .and_then(|i| i.fps_rational)
            .unwrap_or_else(|| ((self.fps().round() as i32).max(1), 1))
    }
```

`recorder.rs`:

```rust
impl Recorder {
    /// Open the encoder and a renderer at `size` (blocking for a moment).
    pub fn start(
        gpu: GpuContext,
        calibration: MatchCalibration,
        input: (u32, u32),
        size: (u32, u32),
        fps: (i32, i32),
        path: &Path,
        quality: RecordingQuality,
    ) -> Result<Self, String> {
        let renderer = build_renderer(gpu, calibration, input, size, wgpu::TextureFormat::Rgba8Unorm)
            .map_err(|e| e.to_string())?;
        let (mut encoder, _name) =
            create_encoder(path, size.0, size.1, fps, RECORDING_CODEC, quality.name(), None, None, None)
                .map_err(|e| format!("the encoder didn't start: {e}"))?;
        let (tx, rx) = mpsc::sync_channel::<Vec<u8>>(ENCODE_QUEUE);
        let (width, height) = size;
        let thread = std::thread::Builder::new()
            .name("reco-record".into())
            .spawn(move || {
                let mut written = 0u64;
                for data in rx {
                    encoder
                        .submit(OutputFrame { data: &data, width, height, format: PixelFormat::Nv12, pts_us: 0 })
                        .map_err(|e| format!("frame {written} didn't encode: {e}"))?;
                    written += 1;
                }
                encoder.finish().map_err(|e| format!("the file didn't close: {e}"))?;
                Ok(written)
            })
            .map_err(|e| e.to_string())?;
        Ok(Self { renderer, size, frames: Some(tx), encoder: Some(thread), path: path.to_path_buf() })
    }

    /// Record `frame` as the preview shows it: the pose's yaw, pitch and
    /// FOV, kept inside the picture for this recording's aspect.
    pub fn record(&mut self, frame: &StereoYuv, pose: ViewportPosition) -> Result<(), String> {
        let fov = pose.fov_degrees.unwrap_or(FOV_DEFAULT);
        self.renderer.pipeline_mut().set_fov(fov);
        let aspect = self.size.0 as f32 / self.size.1 as f32;
        let clamped = self.renderer.clamp_pose(pose.yaw, pose.pitch, fov, aspect);
        let pitch = render_pitch(clamped.yaw, clamped.pitch, self.renderer.pipeline().viewport().rig_tilt);
        let nv12 = self
            .renderer
            .render_and_readback_nv12(&frame.left.as_planes(), &frame.right.as_planes(), clamped.yaw, pitch)
            .map_err(|e| e.to_string())?
            .map(<[u8]>::to_vec);
        match nv12 {
            Some(nv12) => self.send(nv12),
            None => Ok(()),
        }
    }

    fn send(&self, nv12: Vec<u8>) -> Result<(), String> {
        let tx = self.frames.as_ref().ok_or("the recording has stopped")?;
        tx.send(nv12).map_err(|_| "the encoder stopped".to_string())
    }

    /// Flush the last frames, close the file and wait for it (blocking:
    /// call on the render thread). An encoder error wins over a flush error.
    pub fn finish(mut self) -> Result<Recording, String> {
        let mut flushed = Ok(());
        loop {
            let next = self.renderer.flush_nv12().map(|o| o.map(<[u8]>::to_vec));
            match next {
                Ok(Some(nv12)) => {
                    if self.send(nv12).is_err() {
                        break;
                    }
                }
                Ok(None) => break,
                Err(e) => {
                    flushed = Err(e.to_string());
                    break;
                }
            }
        }
        drop(self.frames.take());
        let frames = match self.encoder.take().map(JoinHandle::join) {
            Some(Ok(result)) => result?,
            _ => return Err("the encoder thread crashed".into()),
        };
        flushed?;
        Ok(Recording { path: self.path.clone(), frames })
    }
}
```

`FOV_DEFAULT` is already `pub` in `session.rs`.

- [ ] **Step 4: Run the tests**

Run: `cargo test -p reco-app -- --test-threads=1`
Expected: all pass, including the 3 new recorder tests. The session tests still pass after the `build_renderer` change.

- [ ] **Step 5: Commit**

```bash
git add crates/reco-app/src
git commit -m "feat(app): the preview recorder: a fixed-size renderer, NV12 readback and an encoder thread"
```

---

### Task 7: Recording in the worker (reco-app)

**Files:**
- Modify: `crates/reco-app/src/preview/worker.rs`

**Interfaces:**
- Consumes: the `PreviewSession` recording methods and `Recording` (Task 6); `RecordingQuality` (Task 2).
- Produces:
  - `PreviewCommand::StartRecording { path: PathBuf, size: (u32, u32), quality: RecordingQuality }` and `PreviewCommand::StopRecording`.
  - `PreviewEvent::RecordingStarted { path: PathBuf }`, `PreviewEvent::Recorded { frames: u64 }` (the count so far, one event per recorded frame), `PreviewEvent::RecordingSaved(Recording)`, `PreviewEvent::RecordingFailed(String)`.
  - The worker records once per new frame on screen (`frame_index` changed), before presenting. It finishes the recording on `StopRecording`, on `Quit`, and when the UI drops its handle.

- [ ] **Step 1: Write the failing tests**

Append to `worker.rs`'s tests:

```rust
    fn temp_video(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("reco-app-worker-{name}-{}.mp4", std::process::id()))
    }

    fn open_fast(worker: &PreviewWorker) -> bool {
        let Some((left, right, cal)) = fixtures::fast_set() else { return false };
        worker.send(PreviewCommand::Resize { width: 320, height: 180 });
        worker.send(PreviewCommand::Open { left: InputPath::Single(left), right: InputPath::Single(right), calibration: cal });
        wait_for(worker, 30, |e| matches!(e, PreviewEvent::Ready(_))).is_some()
    }

    fn start_recording(worker: &PreviewWorker, path: &std::path::Path) {
        worker.send(PreviewCommand::StartRecording {
            path: path.to_path_buf(),
            size: (640, 360),
            quality: RecordingQuality::Fast,
        });
        let started = wait_for(worker, 10, |e| matches!(e, PreviewEvent::RecordingStarted { .. } | PreviewEvent::RecordingFailed(_)));
        assert!(matches!(started, Some(PreviewEvent::RecordingStarted { .. })), "{started:?}");
    }

    /// The highest `Recorded` count seen within `secs`.
    fn recorded_within(worker: &PreviewWorker, secs: f64) -> u64 {
        let deadline = Instant::now() + Duration::from_secs_f64(secs);
        let mut frames = 0;
        while Instant::now() < deadline {
            while let Some(e) = worker.try_event() {
                if let PreviewEvent::Recorded { frames: n } = e {
                    frames = frames.max(n);
                }
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        frames
    }

    fn saved(worker: &PreviewWorker) -> Recording {
        worker.send(PreviewCommand::StopRecording);
        let event = wait_for(worker, 20, |e| matches!(e, PreviewEvent::RecordingSaved(_) | PreviewEvent::RecordingFailed(_)));
        match event {
            Some(PreviewEvent::RecordingSaved(recording)) => recording,
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn records_while_playing() {
        let worker = readback_worker();
        if !open_fast(&worker) {
            return;
        }
        let path = temp_video("plays");
        start_recording(&worker, &path);
        worker.send(PreviewCommand::TogglePlay);
        std::thread::sleep(Duration::from_millis(1000));
        let recording = saved(&worker);
        assert!((20..=40).contains(&recording.frames), "about a second at 30 fps: {}", recording.frames);
        let secs = reco_io::ffmpeg::decoder::VideoDecoder::open(&path).expect("playable").duration_secs().unwrap_or(0.0);
        assert!((secs - recording.frames as f64 / 30.0).abs() < 0.1, "{secs}");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn pausing_adds_no_frames() {
        let worker = readback_worker();
        if !open_fast(&worker) {
            return;
        }
        let path = temp_video("paused");
        start_recording(&worker, &path);
        worker.send(PreviewCommand::TogglePlay);
        std::thread::sleep(Duration::from_millis(600));
        worker.send(PreviewCommand::TogglePlay);
        let at_pause = recorded_within(&worker, 0.4);
        // Panning while paused moves the view, not the video: no new frames.
        worker.send(PreviewCommand::Pan { dx: 80.0, dy: 0.0 });
        assert_eq!(recorded_within(&worker, 0.5), 0, "no frames while paused");
        assert_eq!(saved(&worker).frames, at_pause);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn quitting_while_recording_finishes_the_file() {
        let worker = readback_worker();
        if !open_fast(&worker) {
            return;
        }
        let path = temp_video("quit");
        start_recording(&worker, &path);
        worker.send(PreviewCommand::TogglePlay);
        std::thread::sleep(Duration::from_millis(500));
        drop(worker);
        // The worker closes the file on its own thread after Quit.
        let deadline = Instant::now() + Duration::from_secs(10);
        let mut playable = false;
        while Instant::now() < deadline && !playable {
            playable = reco_io::ffmpeg::decoder::VideoDecoder::open(&path)
                .ok()
                .and_then(|v| v.duration_secs())
                .is_some_and(|s| s > 0.0);
            std::thread::sleep(Duration::from_millis(100));
        }
        assert!(playable, "the recording was finished after the worker was dropped");
        let _ = std::fs::remove_file(&path);
    }
```

Add the command and event variants (documented as in Interfaces), and the imports `use super::recorder::Recording;` and `use crate::recording::RecordingQuality;`.

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test -p reco-app -- --test-threads=1 records_while pausing quitting`
Expected: FAIL. The worker has no arms for the new commands (a compile error until the arms exist); then `RecordingStarted` never arrives.

- [ ] **Step 3: Implement**

Worker fields (initialised `None` and `0`):

```rust
    /// The frame last recorded (`frame_index`), so each is recorded once.
    recorded_at: Option<u64>,
    /// Frames recorded so far.
    recorded: u64,
```

`apply` arms:

```rust
            PreviewCommand::StartRecording { path, size, quality } => self.start_recording(&path, size, quality),
            PreviewCommand::StopRecording => self.finish_recording(),
```

Methods:

```rust
    fn start_recording(&mut self, path: &std::path::Path, size: (u32, u32), quality: RecordingQuality) {
        let Some(session) = self.session.as_mut() else { return };
        if session.is_recording() {
            return;
        }
        match session.start_recording(path, size, quality) {
            Ok(()) => {
                self.recorded_at = None;
                self.recorded = 0;
                self.dirty = true;
                self.out.send(PreviewEvent::RecordingStarted { path: path.to_path_buf() });
            }
            Err(e) => self.out.send(PreviewEvent::RecordingFailed(e.to_string())),
        }
    }

    /// Record the frame on screen when it is new since the last one recorded
    /// (playing, stepping or seeking); a pan alone records nothing.
    fn record_new_frame(&mut self) {
        let Some(session) = self.session.as_mut() else { return };
        let frame = session.playback().frame_index();
        if !session.is_recording() || self.recorded_at == Some(frame) {
            return;
        }
        match session.record_frame() {
            Ok(()) => {
                self.recorded_at = Some(frame);
                self.recorded += 1;
                self.out.send(PreviewEvent::Recorded { frames: self.recorded });
            }
            Err(e) => {
                let _ = session.stop_recording();
                self.out.send(PreviewEvent::RecordingFailed(e.to_string()));
            }
        }
    }

    /// Finish a running recording and say how it went.
    fn finish_recording(&mut self) {
        let Some(result) = self.session.as_mut().and_then(PreviewSession::stop_recording) else { return };
        match result {
            Ok(recording) => self.out.send(PreviewEvent::RecordingSaved(recording)),
            Err(e) => self.out.send(PreviewEvent::RecordingFailed(e.to_string())),
        }
    }
```

In `step()`, call `self.record_new_frame();` right after the `Time` event is sent and before `if advanced || moved || self.dirty`. In `run()`, call `self.finish_recording();` after the `while !self.quit` loop, so `Quit` and a dropped handle both close the file.

- [ ] **Step 4: Run the tests**

Run: `cargo test -p reco-app -- --test-threads=1 && cargo clippy -p reco-app --no-deps --all-targets -- -D warnings && cargo build -p reco-desktop`
Expected: all pass (3 new); clippy clean; reco-desktop builds.

- [ ] **Step 5: Commit**

```bash
git add crates/reco-app/src
git commit -m "feat(app): record from the render worker, one frame per new frame, finished on stop or quit"
```

---
### Task 8: Remember the preview aspect (desktop); start the Module 2 check

**Files:**
- Modify: `crates/reco-desktop/src/main.rs`, `crates/reco-desktop/src/session_view.rs`
- Create: `crates/reco-desktop/tools/check_m2.py`

**Interfaces:**
- Consumes: `reco_app::settings::{DesktopSettings, load, save}`, `PreviewAspect::{index, from_index}` (Task 2).
- Produces:
  - `App.settings: DesktopSettings` (`#[rust]`).
  - `App::apply_settings(&mut self, cx)`: puts the saved choices into the widgets.
  - `App::save_settings(&self)`: logs an error on failure.
  - `check_m2.py` with:
    - helpers `expect`, `launch(files=FAST, extra=(), config_dir=None)`, `ready`, `wait_for`, `text_of`, `seconds`;
    - a `CHECKS` table, where `check_m2.py NAME…` runs the named checks;
    - the `persist` check.

- [ ] **Step 1: Write the failing check**

`tools/check_m2.py`:

```python
#!/usr/bin/env python3
"""Module 2 check: the time panel and status (PARITY.md, Module 2).

Run after `cargo build --profile desktop -p reco-desktop`. Opens the fast
fixture pair (RECO_FIXTURE_LEFT/RIGHT/CAL, else the alfheim set) through
--left/--right/--calibration and drives the app through Makepad's --remote
control. Each launch gets its own settings folder (drive.launch_env).
Screenshots go to target/desktop-checks/m2/. `check_m2.py NAME...` runs the
named checks (all by default); exits non-zero if any check failed.
"""
import json
import os
import shutil
import subprocess
import sys
import tempfile
import time

import drive

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "..", ".."))
BIN = os.path.join(ROOT, "target", "desktop", "reco-desktop")
OUT = os.path.join(ROOT, "target", "desktop-checks", "m2")
HOME = os.path.expanduser("~")
FAST = (
    os.environ.get("RECO_FIXTURE_LEFT", f"{HOME}/dev/pitchcam-data/alfheim/cam0.mp4"),
    os.environ.get("RECO_FIXTURE_RIGHT", f"{HOME}/dev/pitchcam-data/alfheim/cam1.mp4"),
    os.environ.get("RECO_FIXTURE_CAL", f"{HOME}/dev/pitchcam-data/alfheim/reco/match.json"),
)
REAL = (
    f"{HOME}/Downloads/match_recording/left/GX010120.MP4",
    f"{HOME}/Downloads/match_recording/right/GX010092.MP4",
    f"{HOME}/Downloads/match_recording/left/GX010120_calibration.json",
)

FAILURES = []


def expect(ok, message):
    print(f"{'ok' if ok else 'FAIL'}: {message}")
    if not ok:
        FAILURES.append(message)


def launch(files=FAST, extra=(), config_dir=None):
    left, right, cal = files
    env = {"RECO_CONFIG_DIR": config_dir} if config_dir else None
    return drive.App.launch(BIN, ["--window-size", "1280x820", "--left", left, "--right", right,
                                  "--calibration", cal, *extra], env=env)


def wait_for(probe, secs):
    deadline = time.monotonic() + secs
    while time.monotonic() < deadline:
        value = probe()
        if value:
            return value
        time.sleep(0.2)
    return None


def ready(app):
    """Wait for the stitched preview; its rect, or None."""
    return wait_for(lambda: app.rect("preview"), 30)


def text_of(app, widget_id):
    for item in app.snap(widget_id):
        if item.get("i") == widget_id:
            return item.get("t", "")
    return None


def seconds(clock):
    """'1:05' or '1:02:03' to seconds; None for anything else."""
    try:
        return drive.parse_cputime(clock)
    except (AttributeError, ValueError):
        return None


def pick_aspect_4x3(app):
    """Choose 4:3 from the aspect dropdown with the keyboard."""
    app.click_id("aspect")
    app.key("down")
    app.key("down")
    app.key("return")
    time.sleep(0.5)


def check_persist():
    """The preview aspect survives a restart; a malformed file still opens."""
    config = tempfile.mkdtemp(prefix="reco-desktop-config-")
    with launch(config_dir=config) as app:
        expect(ready(app) is not None, "persist: the preview appears")
        pick_aspect_4x3(app)
    path = os.path.join(config, "desktop.json")
    saved = json.load(open(path)) if os.path.exists(path) else {}
    expect(saved.get("preview_aspect") == "4:3", f"persist: the aspect is saved ({saved.get('preview_aspect')})")
    with launch(config_dir=config) as app:
        ready(app)
        time.sleep(1.0)
        x, y, w, h = app.rect("preview")
        expect(abs(w / h - 4 / 3) < 0.03, f"persist: the aspect comes back after a restart ({w}x{h})")
    with open(path, "w") as f:
        f.write("{")
    with launch(config_dir=config) as app:
        expect(ready(app) is not None, "persist: a malformed settings file still opens")
        expect(app.errors() == [], "persist: no errors in the app log")


CHECKS = {
    "persist": check_persist,
}


def main():
    if not all(os.path.exists(p) for p in FAST):
        print("skip: fixtures not found (set RECO_FIXTURE_LEFT/RIGHT/CAL)")
        return
    os.makedirs(OUT, exist_ok=True)
    names = sys.argv[1:] or list(CHECKS)
    for name in names:
        CHECKS[name]()
    if FAILURES:
        print(f"\nModule 2 check FAILED: {len(FAILURES)} failure(s):")
        for message in FAILURES:
            print(f"  - {message}")
        sys.exit(1)
    print(f"Module 2 check passed. Screenshots: {OUT}")


if __name__ == "__main__":
    main()
```

(`shutil` and `subprocess` are used by later checks.)

- [ ] **Step 2: Run it to see it fail**

Run: `cargo build --profile desktop -p reco-desktop && /usr/bin/python3 crates/reco-desktop/tools/check_m2.py persist`
Expected: `FAIL: persist: the aspect is saved (None)` and `FAIL: persist: the aspect comes back after a restart`.

- [ ] **Step 3: Implement**

`main.rs`: add the field and import:

```rust
use reco_app::settings::{self, DesktopSettings};
```

```rust
    /// What the app remembers between runs (desktop.json).
    #[rust]
    settings: DesktopSettings,
```

and these methods in `impl App`:

```rust
    /// Show the saved choices in their widgets.
    fn apply_settings(&mut self, cx: &mut Cx) {
        let aspect = self.settings.aspect();
        self.ui.drop_down(cx, ids!(aspect)).set_selected_item(cx, aspect.index());
        if let Some(mut preview) = self.ui.widget(cx, ids!(preview)).borrow_mut::<ui::preview::RecoPreview>() {
            preview.set_aspect(cx, aspect);
        }
    }

    /// Save the settings; a failure is logged, not shown (nothing is lost
    /// but the choice for the next run).
    fn save_settings(&self) {
        if let Err(e) = settings::save(&self.settings) {
            error!("couldn't save the settings: {e}");
        }
    }
```

In `handle_startup`, after the command line is parsed:

```rust
        self.settings = settings::load();
        self.apply_settings(cx);
```

`session_view.rs`, in `preview_actions`, replace the aspect branch:

```rust
        if let Some(index) = self.ui.drop_down(cx, ids!(aspect)).selected(actions) {
            let aspect = PreviewAspect::from_index(index);
            if let Some(mut preview) = self.ui.widget(cx, ids!(preview)).borrow_mut::<RecoPreview>() {
                preview.set_aspect(cx, aspect);
            }
            self.settings.set_aspect(aspect);
            self.save_settings();
            // A dropdown takes the keyboard on a click. A pick made with the
            // mouse gives it back to the preview's shortcuts; arrow keys on
            // a focused dropdown keep stepping through the choices.
            if self.pointer_input {
                cx.set_key_focus(Area::Empty);
            }
        }
```

`save_settings` and `apply_settings` are private in `main.rs`; `session_view` is a child module, so it can call them.

- [ ] **Step 4: Run the check**

Run: `cargo build --profile desktop -p reco-desktop && cargo test -p reco-desktop && /usr/bin/python3 crates/reco-desktop/tools/check_m2.py persist`
Expected: unit tests pass; the 4 `persist` lines are `ok`; "Module 2 check passed".

- [ ] **Step 5: Commit**

```bash
git add crates/reco-desktop/src crates/reco-desktop/tools/check_m2.py
git commit -m "feat(desktop): remember the preview aspect; start the Module 2 check"
```

---

### Task 9: Transport and the status line (desktop)

**Files:**
- Modify: `crates/reco-desktop/src/live.rs`, `src/keys.rs`, `src/ui/preview.rs`, `src/session_view.rs`, `src/main.rs`
- Modify: `crates/reco-desktop/tools/check_m2.py`

**Interfaces:**
- Consumes: `PreviewEvent::Time { frame, state }` (Task 4); `PlayState`.
- Produces:
  - `live::StatusInputs<'a> { state: Option<PlayState>, played: bool, fps: Option<f64>, recording: Option<f64>, problem: Option<&'a str> }` (Default).
  - `live::status_line(&StatusInputs) -> String`.
  - `FpsMeter::reset(&mut self)`.
  - `Live::new(worker, files) -> Live`, with fields `state: PlayState`, `played: bool`, `fps_reading: Option<f64>` and `problem: Option<String>` (replacing `playing`). `Live::status(&self) -> String`.
  - `keys::repeats(KeyCommand) -> bool`.
  - `App.play_icon`, `App.pause_icon: Option<ScriptHandleRef>` (`#[live]`), and `App::set_play_icon(&mut self, cx, playing: bool)`.

- [ ] **Step 1: Write the failing tests and check**

Append to `live.rs`'s tests:

```rust
    #[test]
    fn status_says_what_playback_does() {
        let s = |state, played, fps| {
            status_line(&StatusInputs { state: Some(state), played, fps, ..StatusInputs::default() })
        };
        assert_eq!(s(PlayState::Paused, false, None), "Ready");
        assert_eq!(s(PlayState::Playing, true, None), "Playing");
        assert_eq!(s(PlayState::Playing, true, Some(29.96)), "30.0 fps");
        assert_eq!(s(PlayState::Paused, true, None), "Paused");
        assert_eq!(s(PlayState::Finished, true, None), "Finished");
        assert_eq!(status_line(&StatusInputs::default()), "");
    }

    #[test]
    fn recording_and_problems_come_first() {
        let recording = StatusInputs {
            state: Some(PlayState::Playing),
            recording: Some(75.0),
            problem: Some("x"),
            ..StatusInputs::default()
        };
        assert_eq!(status_line(&recording), "Recording · 1:15");
        let problem = StatusInputs { state: Some(PlayState::Paused), problem: Some("Couldn't seek"), ..StatusInputs::default() };
        assert_eq!(status_line(&problem), "Couldn't seek");
    }

    #[test]
    fn a_reset_meter_starts_over() {
        let t0 = Instant::now();
        let mut m = FpsMeter::default();
        m.tick(t0);
        m.reset();
        assert_eq!(m.tick(t0 + Duration::from_secs(5)), None, "a reset meter waits a second again");
    }
```

Add to `keys.rs`'s tests:

```rust
    #[test]
    fn toggles_do_not_repeat() {
        assert!(!repeats(KeyCommand::TogglePlay));
        assert!(!repeats(KeyCommand::ResetView));
        assert!(!repeats(KeyCommand::Fullscreen));
        assert!(repeats(KeyCommand::Pan { dx: 20.0, dy: 0.0 }));
        assert!(repeats(KeyCommand::Zoom { degrees: 5.0 }));
        assert!(repeats(KeyCommand::SeekBy { seconds: 5.0 }));
    }
```

Stubs, so the tests compile and fail:
- in `live.rs`: `StatusInputs` (as in Interfaces, `#[derive(Clone, Debug, Default, PartialEq)]`, documented fields), `pub fn status_line(s: &StatusInputs) -> String { todo!() }`, and `pub fn reset(&mut self) { todo!() }` on `FpsMeter`;
- in `keys.rs`: `pub fn repeats(command: KeyCommand) -> bool { todo!() }`.

Add to `check_m2.py`:

```python
def frame_pixels(app, name):
    """Pixels over the preview's frame, every 6 points: exact, so one video
    frame can be told from the next."""
    png = app.grab(os.path.join(OUT, f"{name}.png"))
    scale = png.width / app.get("/s")["w"][0]["sz"][0]
    x, y, w, h = app.rect("preview")
    return [png.pixel(int((x + i) * scale), int((y + j) * scale))[:3]
            for j in range(2, int(h) - 2, 6) for i in range(2, int(w) - 2, 6)]


def widget_pixels(app, widget_id, name):
    """Every pixel of one widget, for telling an icon from another."""
    png = app.grab(os.path.join(OUT, f"{name}.png"))
    scale = png.width / app.get("/s")["w"][0]["sz"][0]
    x, y, w, h = app.rect(widget_id)
    return [png.pixel(int((x + i) * scale), int((y + j) * scale))[:3]
            for j in range(int(h)) for i in range(int(w))]


def check_transport():
    """Step, play and pause; the play button's icon; the status line; the end."""
    with launch() as app:
        ready(app)
        time.sleep(1.0)
        expect(text_of(app, "status_text") == "Ready", f"transport: Ready after opening ({text_of(app, 'status_text')})")
        paused_icon = widget_pixels(app, "play_pause", "icon-paused")
        a = frame_pixels(app, "step-a")
        app.click_id("step_forward")
        time.sleep(0.6)
        b = frame_pixels(app, "step-b")
        app.click_id("step_back")
        time.sleep(1.0)
        c = frame_pixels(app, "step-c")
        expect(a != b, "transport: step forward shows the next frame")
        expect(a == c, "transport: step back shows the frame before again")
        app.key("space")
        time.sleep(2.5)
        status = text_of(app, "status_text") or ""
        expect(status.endswith("fps"), f"transport: the status line shows the frame rate while playing ({status})")
        expect(widget_pixels(app, "play_pause", "icon-playing") != paused_icon,
               "transport: the play button shows pause while playing")
        app.key("space")
        time.sleep(0.5)
        expect(text_of(app, "status_text") == "Paused", f"transport: Paused ({text_of(app, 'status_text')})")
        for _ in range(12):
            app.key("]")
        time.sleep(1.5)
        app.key("space")
        finished = wait_for(lambda: text_of(app, "status_text") == "Finished", 10)
        expect(bool(finished), "transport: playback finishes at the end")
        app.key("space")
        time.sleep(1.5)
        t = seconds(text_of(app, "time_current"))
        expect(t is not None and t < 5, f"transport: Space after the end plays from the start ({t})")
        expect(app.errors() == [], "transport: no errors in the app log")
```

and register it: `"transport": check_transport,` in `CHECKS`.

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test -p reco-desktop -- status reset_meter toggles && cargo build --profile desktop -p reco-desktop && /usr/bin/python3 crates/reco-desktop/tools/check_m2.py transport`
Expected: the unit tests panic with `not yet implemented`. The check fails on the play icon (it never changes), on "Paused" (the status keeps the last fps reading) and on "Finished" (the status never says so).

- [ ] **Step 3: Implement**

`live.rs`:

```rust
use reco_app::preview::playback::PlayState;

use crate::time_ruler::clock;
```

```rust
/// What the status line is made from.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct StatusInputs<'a> {
    /// The play state; `None` before the videos opened.
    pub state: Option<PlayState>,
    /// Whether playback has run since opening ("Paused", not "Ready").
    pub played: bool,
    /// The last frame-rate reading while playing.
    pub fps: Option<f64>,
    /// Seconds recorded, while recording.
    pub recording: Option<f64>,
    /// A failure to show until playback moves on.
    pub problem: Option<&'a str>,
}

/// The status line: recording, then a problem, then what playback does.
pub fn status_line(s: &StatusInputs) -> String {
    if let Some(secs) = s.recording {
        return format!("Recording · {}", clock(secs));
    }
    if let Some(problem) = s.problem {
        return problem.to_string();
    }
    match s.state {
        Some(PlayState::Playing) => s.fps.map_or_else(|| "Playing".to_string(), |f| format!("{f:.1} fps")),
        Some(PlayState::Paused) if s.played => "Paused".to_string(),
        Some(PlayState::Paused) => "Ready".to_string(),
        Some(PlayState::Finished) => "Finished".to_string(),
        Some(PlayState::Empty) | None => String::new(),
    }
}
```

`FpsMeter::reset`: `*self = Self::default();`.

Replace `Live`'s `playing: bool` with the new fields, and add a constructor and the status:

```rust
    /// Playing, paused or finished.
    pub state: PlayState,
    /// Whether playback has run since opening.
    pub played: bool,
    /// The last frame-rate reading while playing.
    pub fps_reading: Option<f64>,
    /// A failure to show in the status line until playback moves on.
    pub problem: Option<String>,
```

```rust
impl Live {
    /// A session that has just been asked to open `files`.
    pub fn new(worker: PreviewWorker, files: FileArgs) -> Self {
        Self {
            worker,
            files,
            info: None,
            frame: 0,
            state: PlayState::Paused,
            played: false,
            fps: FpsMeter::default(),
            fps_reading: None,
            problem: None,
        }
    }

    /// The status line for this session.
    pub fn status(&self) -> String {
        status_line(&StatusInputs {
            state: self.info.as_ref().map(|_| self.state),
            played: self.played,
            fps: self.fps_reading,
            recording: None,
            problem: self.problem.as_deref(),
        })
    }
}
```

In `start_live`, build it with `Live::new(worker, files)`.

`keys.rs`:

```rust
/// Whether holding the key repeats the command: moves and seeks do; play,
/// reset and fullscreen act once per press.
pub fn repeats(command: KeyCommand) -> bool {
    matches!(command, KeyCommand::Pan { .. } | KeyCommand::Zoom { .. } | KeyCommand::SeekBy { .. })
}
```

`ui/preview.rs`, in the `KeyDown` branch:

```rust
                if let Some(command) = command_for_key(ke.key_code, &ke.modifiers) {
                    if !ke.is_repeat || repeats(command) {
                        self.key(cx, command);
                    }
                }
```

(import `repeats` from `crate::keys`).

`main.rs`, App fields and the startup DSL:

```rust
    /// The play button's two icons.
    #[live]
    play_icon: Option<ScriptHandleRef>,
    #[live]
    pause_icon: Option<ScriptHandleRef>,
```

```
    startup() do #(App::script_component(vm)){
        play_icon: crate_resource("self:resources/icons/play.svg")
        pause_icon: crate_resource("self:resources/icons/pause.svg")
        ui: Root{
```

`session_view.rs`:

```rust
    /// Show pause while playing, play otherwise. The SVG handle is swapped
    /// in place; Makepad reloads the icon when the handle changes.
    pub(crate) fn set_play_icon(&mut self, cx: &mut Cx, playing: bool) {
        let want = if playing { self.pause_icon.clone() } else { self.play_icon.clone() };
        let button = self.ui.button(cx, ids!(play_pause));
        if let Some(mut inner) = button.borrow_mut() {
            let same = inner.draw_icon.svg.as_ref().map(|h| h.as_handle()) == want.as_ref().map(|h| h.as_handle());
            if !same {
                inner.draw_icon.svg = want;
            }
        }
        button.redraw(cx);
    }
```

Replace `show_time`:

```rust
    /// The playhead moved, or the play state changed.
    fn show_time(&mut self, cx: &mut Cx, frame: u64, state: PlayState) {
        let Some(live) = self.live.as_mut() else { return };
        let playing = state == PlayState::Playing;
        if playing && live.state != PlayState::Playing {
            live.fps.reset();
            live.fps_reading = None;
        }
        if playing || frame != live.frame {
            live.problem = None;
        }
        live.played |= playing;
        live.frame = frame;
        live.state = state;
        live.fps_reading = if playing {
            live.fps.tick(Instant::now()).or(live.fps_reading)
        } else {
            None
        };
        let fps = live.info.as_ref().map_or(0.0, |i| i.fps);
        let now = if fps > 0.0 { frame.saturating_sub(1) as f64 / fps } else { 0.0 };
        let status = live.status();
        self.set_label(cx, ids!(time_current), &time_ruler::clock(now));
        self.set_label(cx, ids!(status_text), &status);
        self.set_play_icon(cx, playing);
        self.update_ruler(cx);
    }
```

Update the `Time` arm in `drain_preview` to `Some(PreviewEvent::Time { frame, state }) => self.show_time(cx, frame, state),`. In `show_live`, replace the literal `"Ready"` status with `&live.status()`; the session is the `live` binding there. `show_stopped` keeps the problem:

```rust
    fn show_stopped(&mut self, cx: &mut Cx, message: &str) {
        error!("preview: {message}");
        let (title, _) = live::failure_text(message);
        if let Some(live) = self.live.as_mut() {
            live.problem = Some(title.clone());
        }
        self.set_label(cx, ids!(status_text), &title);
    }
```

- [ ] **Step 4: Run the tests and the check**

Run: `cargo test -p reco-desktop && cargo clippy -p reco-desktop --no-deps --all-targets -- -D warnings && cargo build --profile desktop -p reco-desktop && /usr/bin/python3 crates/reco-desktop/tools/check_m2.py transport`
Expected: the unit tests pass (4 new); clippy is clean; every `transport` line is `ok`.

- [ ] **Step 5: Commit**

```bash
git add crates/reco-desktop/src crates/reco-desktop/tools/check_m2.py
git commit -m "feat(desktop): play/pause icon, a status line that follows playback, keys that don't repeat toggles"
```

---

### Task 10: The ruler: scrubbing, lanes from the files, the export-range tint (desktop)

**Files:**
- Modify: `crates/reco-desktop/src/time_ruler.rs`, `src/ui/time_panel.rs`, `src/cli.rs`, `src/live.rs`, `src/session_view.rs`, `src/main.rs`, `src/theme.rs`
- Modify: `crates/reco-desktop/tools/check_m2.py`

**Interfaces:**
- Consumes: `PreviewCommand::SeekTo` (Task 4); `PreviewEvent::Lanes`, `Lanes` (Task 5).
- Produces:
  - `time_ruler::time_at(x: f64, width: f64, duration: f64) -> f64`.
  - `time_ruler::tint_span(range: (f64, f64), duration: f64) -> Option<(f64, f64)>`.
  - `time_ruler::SETTLE_TOLERANCE: f64 = 0.5` and `time_ruler::settled(target: f64, reported: f64) -> bool`.
  - `ui::time_panel::RulerAction { Scrub(f64), Seek(f64), None }`: Scrub while dragging, Seek on release.
  - `RecoTimeRuler::set_export_range(&mut self, cx, Option<(f64, f64)>)`.
  - `cli::Args.export_range: Option<(f64, f64)>` from `--export-range START-END` (seconds; for checks until Module 6's export dialog sets it).
  - `Live.lanes: Option<Lanes>`.
  - Theme token `reco_range_tint`.

- [ ] **Step 1: Write the failing tests and check**

Append to `time_ruler.rs`'s tests:

```rust
    #[test]
    fn time_at_maps_the_width_onto_the_length() {
        assert_eq!(time_at(50.0, 200.0, 60.0), 15.0);
        assert_eq!(time_at(-5.0, 200.0, 60.0), 0.0);
        assert_eq!(time_at(500.0, 200.0, 60.0), 60.0);
        assert_eq!(time_at(10.0, 0.0, 60.0), 0.0);
    }

    #[test]
    fn the_tint_shows_only_a_partial_range() {
        assert_eq!(tint_span((10.0, 40.0), 60.0), Some((10.0 / 60.0, 40.0 / 60.0)));
        assert_eq!(tint_span((10.0, 90.0), 60.0), Some((10.0 / 60.0, 1.0)));
        assert_eq!(tint_span((0.0, 60.0), 60.0), None);
        assert_eq!(tint_span((-5.0, 90.0), 60.0), None);
        assert_eq!(tint_span((30.0, 20.0), 60.0), None);
        assert_eq!(tint_span((10.0, 20.0), 0.0), None);
    }

    #[test]
    fn a_seek_settles_within_half_a_second() {
        assert!(settled(30.0, 30.4));
        assert!(!settled(30.0, 12.0));
    }
```

with stubs `pub fn time_at(…) -> f64 { todo!() }`, `pub fn tint_span(…) -> Option<(f64, f64)> { todo!() }`, the constant, and `pub fn settled(…) -> bool { todo!() }`, each documented as in Interfaces.

Append to `cli.rs`'s tests:

```rust
    #[test]
    fn parses_an_export_range() {
        assert_eq!(Args::parse(["--export-range=10-40"]).unwrap().export_range, Some((10.0, 40.0)));
        assert_eq!(Args::parse(["--export-range", "12.5-60"]).unwrap().export_range, Some((12.5, 60.0)));
        assert!(Args::parse(["--export-range=40-10"]).is_err());
        assert!(Args::parse(["--export-range=soon"]).is_err());
    }
```

and the field (documented "The export range to tint on the ruler, from `--export-range START-END` in seconds (checks; Module 6's export dialog sets it)"): `pub export_range: Option<(f64, f64)>,`.

Add to `check_m2.py`:

```python
def ruler_point(app, secs, length, lane=None):
    """Window point at `secs` on the ruler: in the ruler band, or in lane 0/1."""
    x, y, w, h = app.rect("timeline")
    row = 8 if lane is None else 16 + 16 * lane + 8
    return x + w * secs / length, y + row


def pixel_at(png, app, point):
    scale = png.width / app.get("/s")["w"][0]["sz"][0]
    return png.pixel(int(point[0] * scale), int(point[1] * scale))[:3]


def check_ruler():
    """Scrubbing seeks on release; lanes come from the files; the export range is tinted."""
    with launch() as app:
        ready(app)
        time.sleep(1.0)
        (x0, y0), (x1, _) = ruler_point(app, 6, 60.0), ruler_point(app, 30, 60.0)
        app.get("/m", k="down", x=x0, y=y0)
        for step in range(1, 6):
            app.get("/m", k="move", x=x0 + (x1 - x0) * step / 5, y=y0, wait=1)
        during = seconds(text_of(app, "time_current"))
        app.get("/m", k="up", x=x1, y=y0, wait=1)
        expect(during is not None and abs(during - 30) <= 1, f"ruler: the clock follows a scrub ({during})")
        time.sleep(2.0)
        after = seconds(text_of(app, "time_current"))
        expect(after is not None and abs(after - 30) <= 1, f"ruler: releasing seeks there ({after})")
        expect(app.errors() == [], "ruler: no errors in the app log")

    left, right, cal = FAST
    with launch((f"{left};{left}", f"{right};{right}", cal)) as app:
        ready(app)
        total = wait_for(lambda: text_of(app, "time_total") == "2:00", 20)
        expect(bool(total), f"ruler: the length comes from the files ({text_of(app, 'time_total')})")
        png = app.grab(os.path.join(OUT, "lanes-chained.png"))
        block = pixel_at(png, app, ruler_point(app, 30, 120.0, lane=0))
        gap = min((pixel_at(png, app, ruler_point(app, 60 + d / 20, 120.0, lane=0)) for d in range(-20, 21)), key=sum)
        expect(sum(gap) + 60 < sum(block), f"ruler: a gap between the two files ({gap} vs {block})")

    with launch(extra=("--export-range", "10-40")) as app:
        ready(app)
        time.sleep(1.0)
        png = app.grab(os.path.join(OUT, "export-range.png"))
        tinted = pixel_at(png, app, ruler_point(app, 27.5, 60.0, lane=0))
        plain = pixel_at(png, app, ruler_point(app, 52.5, 60.0, lane=0))
        expect(tinted[1] > plain[1] + 4, f"ruler: the export range is tinted ({tinted} vs {plain})")
```

and register `"ruler": check_ruler,`.

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test -p reco-desktop -- time_at tint settles export_range && cargo build --profile desktop -p reco-desktop && /usr/bin/python3 crates/reco-desktop/tools/check_m2.py ruler`
Expected: the unit tests fail (`not yet implemented`, or the unknown flag leaves `export_range` `None`). The check fails on the scrub (the clock doesn't move), on the length (the engine's 2:00 estimate may pass, but the gap doesn't: one block per camera) and on the tint.

- [ ] **Step 3: Implement**

`time_ruler.rs`:

```rust
pub fn time_at(x: f64, width: f64, duration: f64) -> f64 {
    if width <= 0.0 || duration <= 0.0 {
        return 0.0;
    }
    (x / width).clamp(0.0, 1.0) * duration
}

pub fn tint_span(range: (f64, f64), duration: f64) -> Option<(f64, f64)> {
    if duration <= 0.0 {
        return None;
    }
    let (start, end) = (range.0.clamp(0.0, duration), range.1.clamp(0.0, duration));
    if end <= start || (start <= 0.0 && end >= duration) {
        return None;
    }
    Some((start / duration, end / duration))
}

pub const SETTLE_TOLERANCE: f64 = 0.5;

pub fn settled(target: f64, reported: f64) -> bool {
    (reported - target).abs() <= SETTLE_TOLERANCE
}
```

`cli.rs`: in `Args::parse`, before the `--left/--right/--calibration` arm:

```rust
            } else if let Some((_, value)) = flag_value(arg, &mut iter, &["--export-range"])? {
                out.export_range = Some(parse_range(&value)?);
```

and

```rust
/// `START-END` in seconds (`10-40`, `12.5-60`) into a range.
fn parse_range(value: &str) -> Result<(f64, f64), String> {
    let bad = || format!("export range `{value}` is not START-END in seconds");
    let (start, end) = value.split_once('-').ok_or_else(bad)?;
    let start: f64 = start.trim().parse().map_err(|_| bad())?;
    let end: f64 = end.trim().parse().map_err(|_| bad())?;
    if start < 0.0 || end <= start {
        return Err(bad());
    }
    Ok((start, end))
}
```

`theme.rs`, next to the lane tokens: `reco_range_tint: #x34d3992e` (the accent at 18%, over the ruler and lanes).

`ui/time_panel.rs`:
- DSL: `tint_color: theme.reco_range_tint` in `RecoTimeRuler`'s defaults.
- Rust: imports `use std::time::{Duration, Instant};` and `use crate::time_ruler::{clock, settled, ticks, time_at, tint_span};`.
- Add:

```rust
/// How long the ruler holds a sought playhead while the worker seeks.
const SETTLE: Duration = Duration::from_millis(1500);

/// What the ruler asks of the App.
#[derive(Clone, Debug, Default)]
pub enum RulerAction {
    /// Dragging over this time (seconds): show it, don't seek yet.
    Scrub(f64),
    /// Released here: seek.
    Seek(f64),
    #[default]
    None,
}
```

Fields on `RecoTimeRuler`:

```rust
    #[live]
    tint_color: Vec4f,
    /// The export range to tint, in seconds.
    #[rust]
    export_range: Option<(f64, f64)>,
    /// A finger is scrubbing: the worker's reports don't move the playhead.
    #[rust]
    dragging: bool,
    /// A seek in flight: its target and when it was asked for.
    #[rust]
    hold: Option<(f64, Instant)>,
```

Replace `set_timeline` and add the range setter and the scrub helper:

```rust
    /// Show a timeline: its length, the playhead, and each lane's files.
    /// While scrubbing, or until a seek lands (or `SETTLE` passes), the
    /// playhead stays where the user put it.
    pub fn set_timeline(&mut self, cx: &mut Cx, duration: f64, playhead: f64, lanes: Vec<Vec<(f64, f64)>>) {
        self.duration = duration;
        self.lanes = lanes;
        if !self.dragging {
            self.hold = self.hold.filter(|(target, since)| since.elapsed() < SETTLE && !settled(*target, playhead));
            self.playhead = self.hold.map_or(playhead, |(target, _)| target);
        }
        self.area.redraw(cx);
    }

    /// Tint the export range (`None`: no range).
    pub fn set_export_range(&mut self, cx: &mut Cx, range: Option<(f64, f64)>) {
        self.export_range = range;
        self.area.redraw(cx);
    }

    fn scrub_to(&mut self, cx: &mut Cx, fe_x: f64, rect: Rect) {
        self.playhead = time_at(fe_x - rect.pos.x, rect.size.x, self.duration);
        self.area.redraw(cx);
    }
```

Replace `handle_event`:

```rust
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, _scope: &mut Scope) {
        if self.disabled || self.duration <= 0.0 {
            return;
        }
        let uid = self.widget_uid();
        match event.hits(cx, self.area) {
            Hit::FingerHoverIn(_) | Hit::FingerHoverOver(_) => cx.set_cursor(MouseCursor::EwResize),
            Hit::FingerDown(fe) if fe.is_primary_hit() => {
                self.dragging = true;
                cx.set_cursor(MouseCursor::EwResize);
                self.scrub_to(cx, fe.abs.x, fe.rect);
                cx.widget_action(uid, RulerAction::Scrub(self.playhead));
            }
            Hit::FingerMove(fe) if self.dragging => {
                self.scrub_to(cx, fe.abs.x, fe.rect);
                cx.widget_action(uid, RulerAction::Scrub(self.playhead));
            }
            Hit::FingerUp(fe) if self.dragging => {
                self.dragging = false;
                if !fe.cancelled {
                    self.scrub_to(cx, fe.abs.x, fe.rect);
                }
                self.hold = Some((self.playhead, Instant::now()));
                cx.widget_action(uid, RulerAction::Seek(self.playhead));
            }
            _ => {}
        }
    }
```

In `draw_walk`, after the lanes and before the playhead, tint the range over ruler and lanes:

```rust
        // The export range, over the ruler and the lanes.
        if let Some((a, b)) = self.export_range.and_then(|r| tint_span(r, self.duration)) {
            self.line(cx, Rect { pos: dvec2(x0 + a * width, y0), size: dvec2((b - a) * width, height) }, self.tint_color);
        }
```

`live.rs`: `Live` gains `pub lanes: Option<Lanes>` (documented "Each camera's files, once probed"; `None` in `Live::new`), with `use reco_app::preview::lanes::Lanes;`.

`session_view.rs`:

```rust
    /// The files' lanes and the exact length arrived.
    fn show_lanes(&mut self, cx: &mut Cx, lanes: Lanes) {
        self.set_label(cx, ids!(time_total), &time_ruler::clock(lanes.length));
        if let Some(live) = self.live.as_mut() {
            live.lanes = Some(lanes);
        }
        self.update_ruler(cx);
    }

    /// Scrubbing shows the time; releasing seeks.
    pub(crate) fn ruler_actions(&mut self, cx: &mut Cx, actions: &Actions) {
        let uid = self.ui.widget(cx, ids!(timeline)).widget_uid();
        for action in actions.filter_widget_actions_cast::<RulerAction>(uid) {
            match action {
                RulerAction::Scrub(secs) => self.set_label(cx, ids!(time_current), &time_ruler::clock(secs)),
                RulerAction::Seek(secs) => {
                    if let Some(live) = self.live.as_ref() {
                        let fps = live.info.as_ref().map_or(0.0, |i| i.fps);
                        if fps > 0.0 {
                            live.worker.send(PreviewCommand::SeekTo { frame: (secs * fps).floor() as u64 });
                        }
                    }
                }
                RulerAction::None => {}
            }
        }
    }
```

Replace `update_ruler`:

```rust
    /// Each camera's files (one block each until they are probed), the
    /// playhead, and the export range.
    fn update_ruler(&mut self, cx: &mut Cx) {
        let range = self.args.export_range;
        let Some((length, playhead, lanes)) = self.live.as_ref().and_then(|l| {
            let info = l.info.as_ref()?;
            let playhead = if info.fps > 0.0 { l.frame.saturating_sub(1) as f64 / info.fps } else { 0.0 };
            Some(match l.lanes.as_ref() {
                Some(lanes) => (lanes.length, playhead, vec![lanes.left.clone(), lanes.right.clone()]),
                None => {
                    let length = live::length_secs(info);
                    (length, playhead, vec![vec![(0.0, length)], vec![(0.0, length)]])
                }
            })
        }) else {
            return;
        };
        if let Some(mut ruler) = self.ui.widget(cx, ids!(timeline)).borrow_mut::<RecoTimeRuler>() {
            ruler.set_timeline(cx, length, playhead, lanes);
            ruler.set_export_range(cx, range);
        }
    }
```

Add `Some(PreviewEvent::Lanes(lanes)) => self.show_lanes(cx, lanes),` to `drain_preview`. Imports: `Lanes`, `RulerAction`. `main.rs`: call `self.ruler_actions(cx, actions);` at the end of `handle_actions`.

- [ ] **Step 4: Run the tests and the check**

Run: `cargo test -p reco-desktop && cargo clippy -p reco-desktop --no-deps --all-targets -- -D warnings && cargo build --profile desktop -p reco-desktop && /usr/bin/python3 crates/reco-desktop/tools/check_m2.py ruler transport`
Expected: unit tests pass (4 new); clippy is clean; every `ruler` and `transport` line is `ok`.

- [ ] **Step 5: Commit**

```bash
git add crates/reco-desktop/src crates/reco-desktop/tools/check_m2.py
git commit -m "feat(desktop): scrub the ruler, lanes from the files, the export-range tint"
```

---
### Task 11: Toasts in the viewer (desktop)

Makepad's built-in `Toaster` doesn't fit, for four reasons:
- its times are fixed at 4 s, 8 s or indefinite;
- it places itself against the whole window, so it can't stay clear of the Adjust panel;
- it reads raw mouse events instead of hit-testing, so clicks fall through to the preview;
- its body is a single line.

So the toasts are four card slots in a layer over the viewer's canvas, fed by `reco_app::toasts`.

**Files:**
- Create: `crates/reco-desktop/src/ui/toasts.rs`, `crates/reco-desktop/src/toast_view.rs`, `crates/reco-desktop/resources/icons/close.svg`
- Modify: `src/ui/mod.rs`, `src/ui/viewer.rs`, `src/theme.rs`, `src/cli.rs`, `src/main.rs`, `src/session_view.rs`
- Modify: `crates/reco-desktop/tools/check_m2.py`

**Interfaces:**
- Consumes: `reco_app::toasts::{Toasts, Severity}` (Task 3); `live::failure_text`.
- Produces:
  - DSL `mod.widgets.RecoToastCard` and `mod.widgets.RecoToasts`. The ids are `toasts`, `toast_0`…`toast_3`, and in each card `dot_info`, `dot_warn`, `dot_error`, `title`, `body_row`, `body` and `close`.
  - `App.toasts: Toasts`, `App.toast_timer: Timer`.
  - `App::toast(&mut self, cx, Severity, title: &str, body: &str)` and `App::toast_for(&mut self, cx, Severity, title, body, ttl: Duration)`.
  - `App::expire_toasts`, `App::toast_actions`, `App::toast_demo`.
  - `cli::Args.toast_demo: bool` (`--toast-demo`: sample toasts for checks and design review).
  - Failed opens and failures after opening now also raise an error toast.
  - Theme token `reco_toast_width`.

- [ ] **Step 1: Write the failing tests and check**

`cli.rs` test:

```rust
    #[test]
    fn toast_demo_flag() {
        assert!(Args::parse(["--toast-demo"]).unwrap().toast_demo);
        assert!(!Args::parse(Vec::<String>::new()).unwrap().toast_demo);
    }
```

plus the field (documented "Show sample toasts, `--toast-demo` (checks and design review)"): `pub toast_demo: bool,`.

`check_m2.py`:

```python
def title_rect(app, text):
    """The rect of a toast title showing `text`, or None."""
    for item in app.snap(text):
        if item.get("t") == text and item.get("i") == "title":
            return item["r"]
    return None


def check_toasts():
    """At most four, newest at the bottom, inside the viewer; close and
    expiry; the status line keeps its own text; a failed open raises one."""
    with launch(extra=("--toast-demo",)) as app:
        ready(app)
        shown = [app.rect(f"toast_{i}") for i in range(4)]
        expect(all(shown), f"toasts: four show ({sum(1 for r in shown if r)})")
        expect(title_rect(app, "Calibration saved") is None, "toasts: the oldest of five left first")
        newest, older = title_rect(app, "Recording saved"), title_rect(app, "Couldn't open the videos")
        expect(bool(newest and older and newest[1] > older[1]), "toasts: the newest is at the bottom")
        x, y, w, h = app.rect("canvas")
        inside = all(r and r[0] >= x and r[0] + r[2] <= x + w and r[1] + r[3] <= y + h for r in shown)
        expect(inside, "toasts: inside the viewer, clear of the Adjust panel and the time panel")
        expect(text_of(app, "status_text") == "Ready", f"toasts: the status line keeps its own text ({text_of(app, 'status_text')})")
        close = app.rect("close")
        app.get("/click", x=close[0] + close[2] / 2, y=close[1] + close[3] / 2, wait=1)
        expect(title_rect(app, "Recording started") is None, "toasts: a close button dismisses its toast")
        gone = wait_for(lambda: title_rect(app, "Recording saved") is None, 6)
        expect(bool(gone), "toasts: an info toast leaves after about four seconds")
        expect(title_rect(app, "Low calibration confidence") is not None, "toasts: a warning stays longer")
        expect(app.errors() == [], "toasts: no errors in the app log")
    _, right, cal = FAST
    with launch(("/nonexistent/left.mp4", right, cal)) as app:
        failed = wait_for(lambda: title_rect(app, "Couldn't open the videos"), 15)
        expect(bool(failed), "toasts: a failed open raises an error toast")
```

Register `"toasts": check_toasts,`.

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test -p reco-desktop toast_demo && cargo build --profile desktop -p reco-desktop && /usr/bin/python3 crates/reco-desktop/tools/check_m2.py toasts`
Expected: the unit test fails (the unknown flag is ignored), and the check fails (`four show (0)`).

- [ ] **Step 3: Implement**

`resources/icons/close.svg`:

```svg
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16" width="16" height="16">
<rect x="0" y="0" width="16" height="16" fill="none"/>
<path d="M4.5 4.5l7 7M11.5 4.5l-7 7" stroke="#000" stroke-width="1.5" stroke-linecap="round" fill="none"/>
</svg>
```

`theme.rs`: `reco_toast_width: 320.0` (next to `reco_progress_width`).

`cli.rs`, in the loop: `} else if arg == "--toast-demo" { out.toast_demo = true;`.

`ui/toasts.rs`:

```rust
//! Toasts, drawn: four card slots in a layer over the viewer's canvas,
//! bottom right, so notices stay clear of the Adjust panel and the time
//! panel. A card is a floating panel as Rerun's menus are: a dot for its
//! kind, the title, a wrapping detail and a close button. The cards take
//! the pointer; the layer itself does not. The App fills the slots from
//! `reco_app::toasts` (toast_view.rs).

use makepad_widgets::*;

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    mod.widgets.RecoToastCard = RoundedView{
        visible: false
        width: theme.reco_toast_width height: Fit flow: Down spacing: theme.reco_gap_xs
        padding: Inset{left: theme.reco_pad right: theme.reco_gap_s top: theme.reco_gap_s bottom: theme.reco_gap}
        show_bg: true new_batch: true
        cursor: MouseCursor.Default capture_overload: true
        draw_bg +: {
            color: theme.reco_band
            border_radius: theme.container_corner_radius
            border_size: theme.reco_separator_width
            border_color: theme.reco_hover
        }
        View{
            width: Fill height: Fit flow: Right spacing: theme.reco_gap align: Align{y: 0.5}
            dot_info := RecoDot{}
            dot_warn := RecoDotBusy{visible: false}
            dot_error := RecoDotError{visible: false}
            title := RecoStrong{width: Fill text: ""}
            close := RecoRowIcon{
                draw_icon +: {svg: crate_resource("self:resources/icons/close.svg")}
            }
        }
        body_row := View{
            width: Fill height: Fit
            padding: Inset{left: theme.reco_dot + theme.reco_gap right: theme.reco_gap_s}
            body := RecoSubdued{width: Fill text: ""}
        }
    }

    mod.widgets.RecoToasts = View{
        width: Fill height: Fill flow: Down spacing: theme.reco_gap
        align: Align{x: 1.0 y: 1.0}
        padding: Inset{right: theme.reco_pad bottom: theme.reco_pad}
        toast_0 := mod.widgets.RecoToastCard{}
        toast_1 := mod.widgets.RecoToastCard{}
        toast_2 := mod.widgets.RecoToastCard{}
        toast_3 := mod.widgets.RecoToastCard{}
    }
}
```

`ui/mod.rs`: `mod toasts;`, and `toasts::script_mod(vm);` right after `controls::script_mod(vm);`.

`ui/viewer.rs`: the last child of `canvas`, after `export_card`:

```
            // Notices, bottom right: clear of the Adjust panel and the time
            // panel (Module 2).
            toasts := RecoToasts{}
```

`src/toast_view.rs`:

```rust
//! Toasts in the App: notices pushed from anywhere, shown in the viewer's
//! four card slots, expired by one timer, dismissed by their close buttons.
//! They never touch the status line.

use std::time::{Duration, Instant};

use makepad_widgets::*;
use reco_app::toasts::Severity;

use crate::App;

/// The card slots, oldest first.
fn slots() -> [LiveId; 4] {
    [live_id!(toast_0), live_id!(toast_1), live_id!(toast_2), live_id!(toast_3)]
}

impl App {
    /// Show a notice for its severity's time.
    pub(crate) fn toast(&mut self, cx: &mut Cx, severity: Severity, title: &str, body: &str) {
        self.toasts.push(severity, title, body, Instant::now());
        self.show_toasts(cx);
    }

    /// Show a notice for `ttl`.
    pub(crate) fn toast_for(&mut self, cx: &mut Cx, severity: Severity, title: &str, body: &str, ttl: Duration) {
        self.toasts.push_for(severity, title, body, ttl, Instant::now());
        self.show_toasts(cx);
    }

    /// Fill the slots from the model; arm the timer for the next expiry.
    fn show_toasts(&mut self, cx: &mut Cx) {
        let shown = self.toasts.visible().to_vec();
        for (index, slot) in slots().into_iter().enumerate() {
            let toast = shown.get(index);
            self.ui.widget(cx, &[slot]).set_visible(cx, toast.is_some());
            let Some(toast) = toast else { continue };
            self.ui.label(cx, &[slot, live_id!(title)]).set_text(cx, &toast.title);
            self.ui.label(cx, &[slot, live_id!(body)]).set_text(cx, &toast.body);
            self.ui.widget(cx, &[slot, live_id!(body_row)]).set_visible(cx, !toast.body.is_empty());
            for (dot, severity) in [
                (live_id!(dot_info), Severity::Info),
                (live_id!(dot_warn), Severity::Warn),
                (live_id!(dot_error), Severity::Error),
            ] {
                self.ui.widget(cx, &[slot, dot]).set_visible(cx, toast.severity == severity);
            }
        }
        cx.stop_timer(self.toast_timer);
        self.toast_timer = match self.toasts.next_expiry() {
            Some(at) => cx.start_timeout(at.saturating_duration_since(Instant::now()).as_secs_f64().max(0.05)),
            None => Timer::empty(),
        };
        self.ui.widget(cx, ids!(toasts)).redraw(cx);
    }

    /// The timer fired: drop the toasts that are due.
    pub(crate) fn expire_toasts(&mut self, cx: &mut Cx) {
        self.toasts.expire(Instant::now());
        self.show_toasts(cx);
    }

    /// A close button was clicked: dismiss its toast.
    pub(crate) fn toast_actions(&mut self, cx: &mut Cx, actions: &Actions) {
        let ids: Vec<u64> = self.toasts.visible().iter().map(|t| t.id).collect();
        let closed: Vec<u64> = slots()
            .into_iter()
            .zip(ids)
            .filter(|(slot, _)| self.ui.button(cx, &[*slot, live_id!(close)]).clicked(actions))
            .map(|(_, id)| id)
            .collect();
        if closed.is_empty() {
            return;
        }
        for id in closed {
            self.toasts.dismiss(id);
        }
        self.show_toasts(cx);
    }

    /// Sample notices (`--toast-demo`): five pushed, so the oldest has left.
    pub(crate) fn toast_demo(&mut self, cx: &mut Cx) {
        for (severity, title, body) in [
            (Severity::Info, "Calibration saved", ""),
            (Severity::Info, "Recording started", "~/Movies/reco_recording_1700000000.mp4"),
            (Severity::Warn, "Low calibration confidence", "The cameras matched on few points; check the seam."),
            (Severity::Error, "Couldn't open the videos", "Invalid input path (/nonexistent/left.mp4): file not found"),
            (Severity::Info, "Recording saved", "905 frames · reco_recording_1700000000.mp4"),
        ] {
            self.toast(cx, severity, title, body);
        }
    }
}
```

`main.rs`:
- `mod toast_view;`
- fields `#[rust] toasts: reco_app::toasts::Toasts,` and `#[rust] toast_timer: Timer,`, each with a `///` line;
- in `handle_event`, before `self.match_event(cx, event);`: `if self.toast_timer.is_event(event).is_some() { self.expire_toasts(cx); }`;
- in `handle_actions`: `self.toast_actions(cx, actions);`;
- in `handle_startup`, after `start_live` (no files given): `if self.args.toast_demo && self.args.files.is_none() { self.toast_demo(cx); }`.

`session_view.rs`:
- At the end of `show_live`: `if self.args.toast_demo { self.toast_demo(cx); }`.
- In `show_failed`, after the labels: `self.toast(cx, Severity::Error, &title, &detail);`.
- In `show_stopped`, take both parts with `let (title, detail) = live::failure_text(message);` and add `self.toast(cx, Severity::Error, &title, &detail);`.
- Import `reco_app::toasts::Severity`.

- [ ] **Step 4: Run the tests and the check**

Run: `cargo test -p reco-desktop && cargo clippy -p reco-desktop --no-deps --all-targets -- -D warnings && cargo build --profile desktop -p reco-desktop && /usr/bin/python3 crates/reco-desktop/tools/check_m2.py toasts transport`
Expected: unit tests pass (`every_self_resource_exists` and `every_icon_pins_its_viewbox` include close.svg; `screens_use_theme_values_only` passes); every `toasts` and `transport` line is `ok`.

- [ ] **Step 5: Commit**

```bash
git add crates/reco-desktop/src crates/reco-desktop/resources/icons/close.svg crates/reco-desktop/tools/check_m2.py
git commit -m "feat(desktop): toasts in the viewer: four at most, timed, dismissable, never in the status line"
```

---

### Task 12: Recording in the app (desktop), and Show in folder

**Files:**
- Create: `crates/reco-app/src/reveal.rs`, `crates/reco-desktop/resources/icons/stop.svg`
- Modify: `crates/reco-app/src/lib.rs`, `crates/reco-desktop/src/{main.rs,live.rs,session_view.rs,theme.rs}`, `src/ui/{viewer.rs,time_panel.rs}`
- Modify: `crates/reco-desktop/tools/check_m2.py`

**Interfaces:**
- Consumes:
  - the worker's recording commands and events (Task 7);
  - `recording_size`, `recording_folder`, `recording_file_name`, `RecordingQuality` (Task 2);
  - `App::toast`/`toast_for` (Task 11);
  - `App::set_play_icon` (Task 9).
- Produces:
  - `reveal::reveal_command(path: &Path) -> (String, Vec<String>)` and `reveal::reveal(path: &Path) -> std::io::Result<()>`.
  - DSL ids `record_quality` (dropdown Fast/Balanced/High), `recording_badge`, `recording_time` (view bar) and `show_in_folder` (time panel controls row).
  - `App.record_icon`, `App.stop_icon` (`#[live]`).
  - `App::swap_icon(&mut self, cx, button: &[LiveId], icon: Option<ScriptHandleRef>)`; `set_play_icon` uses it.
  - `Live.recording: Option<u64>` (frames so far) and `Live.last_output: Option<PathBuf>`.
  - `App::record_actions`, `App::finish_recording_on_quit`.
  - Theme token `reco_quality_width`.

- [ ] **Step 1: Write the failing tests and check**

`crates/reco-app/src/reveal.rs`:

```rust
//! "Show in folder": show a file in the system's file manager. Finder and
//! Explorer select it; elsewhere its folder opens. The command is spawned
//! and not waited for (a small thread reaps it).

use std::path::Path;
use std::process::Command;

/// The program and arguments that show `path`.
pub fn reveal_command(path: &Path) -> (String, Vec<String>) {
    todo!()
}

/// Show `path` in the file manager; returns once the command started.
pub fn reveal(path: &Path) -> std::io::Result<()> {
    let (program, args) = reveal_command(path);
    let mut child = Command::new(program).args(args).spawn()?;
    std::thread::spawn(move || child.wait());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(target_os = "macos")]
    #[test]
    fn finder_selects_the_file() {
        assert_eq!(
            reveal_command(Path::new("/tmp/a b.mp4")),
            ("open".to_string(), vec!["-R".to_string(), "/tmp/a b.mp4".to_string()])
        );
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    #[test]
    fn the_folder_opens_elsewhere() {
        assert_eq!(
            reveal_command(Path::new("/tmp/x/a.mp4")),
            ("xdg-open".to_string(), vec!["/tmp/x".to_string()])
        );
    }
}
```

`lib.rs`: `pub mod reveal;`.

`check_m2.py`:

```python
def ffprobe(path):
    """(width, height, frames) of a video, or None without ffprobe."""
    tool = shutil.which("ffprobe")
    if tool is None:
        return None
    out = subprocess.run([tool, "-v", "error", "-select_streams", "v:0", "-count_frames",
                          "-show_entries", "stream=width,height,nb_read_frames", "-of", "json", path],
                         capture_output=True, text=True)
    stream = (json.loads(out.stdout or "{}").get("streams") or [{}])[0]
    return stream.get("width"), stream.get("height"), int(stream.get("nb_read_frames") or 0)


def recordings(folder):
    return sorted(f for f in os.listdir(folder) if f.startswith("reco_recording_") and f.endswith(".mp4"))


def check_record():
    """Record: the badge and the quality, a 1920x1080 file with one frame per
    frame played, toasts, Show in folder; quitting while recording still
    leaves a playable file. Never clicks Show in folder (it opens Finder)."""
    config = tempfile.mkdtemp(prefix="reco-desktop-config-")
    folder = tempfile.mkdtemp(prefix="reco-desktop-recordings-")
    with open(os.path.join(config, "desktop.json"), "w") as f:
        json.dump({"recording_folder": folder}, f)
    with launch(config_dir=config) as app:
        ready(app)
        time.sleep(1.0)
        expect(app.rect("record_quality") is not None, "record: the quality shows before recording")
        app.click_id("record_button")
        expect(bool(wait_for(lambda: title_rect(app, "Recording started"), 10)), "record: a toast says recording started")
        expect(app.rect("recording_badge") is not None, "record: the badge shows while recording")
        expect(app.rect("record_quality") is None, "record: the quality hides while recording")
        app.key("space")
        time.sleep(3.0)
        status = text_of(app, "status_text") or ""
        expect(status.startswith("Recording ·"), f"record: the status line says recording ({status})")
        app.key("space")
        time.sleep(0.5)
        app.click_id("record_button")
        expect(bool(wait_for(lambda: title_rect(app, "Recording saved"), 15)), "record: a toast says the recording was saved")
        expect(app.rect("show_in_folder") is not None, "record: Show in folder appears")
        expect(app.rect("record_quality") is not None and app.rect("recording_badge") is None,
               "record: the quality returns and the badge goes")
        body = next((i.get("t", "") for i in app.snap("frames ·") if i.get("i") == "body"), "")
        files = recordings(folder)
        expect(len(files) == 1, f"record: one file in the recording folder ({files})")
        probe = ffprobe(os.path.join(folder, files[0])) if files else None
        if files and probe is None:
            print("skip: ffprobe is not installed; the file's size and frames are not checked")
        elif probe:
            w, h, frames = probe
            expect((w, h) == (1920, 1080), f"record: 1920x1080 for Auto ({w}x{h})")
            expect(80 <= frames <= 100, f"record: about 3 s at 30 fps, one frame per frame played ({frames})")
            told = body.split()[0] if body else ""
            expect(told == str(frames), f"record: the toast counts the frames written ({body!r} vs {frames})")
        for _ in range(2):
            app.click_id("record_button")
            time.sleep(0.8)
            app.click_id("record_button")
            time.sleep(1.5)
        expect(all(app.rect(f"toast_{i}") for i in range(4)), "record: more notices than fit show four")
        expect(app.errors() == [], "record: no errors in the app log")
    with launch(config_dir=config) as app:
        ready(app)
        before = set(recordings(folder))
        app.click_id("record_button")
        wait_for(lambda: title_rect(app, "Recording started"), 10)
        app.key("space")
        time.sleep(1.5)
    new = sorted(set(recordings(folder)) - before)
    expect(len(new) == 1, f"record: quitting while recording leaves a file ({new})")
    if new and shutil.which("ffprobe"):
        expect(ffprobe(os.path.join(folder, new[0]))[2] > 0, "record: and it plays")
```

Register `"record": check_record,`.

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test -p reco-app -- finder the_folder && cargo build --profile desktop -p reco-desktop && /usr/bin/python3 crates/reco-desktop/tools/check_m2.py record`
Expected: `finder_selects_the_file` panics (`not yet implemented`). The check fails, starting with "the quality shows before recording" (there is no `record_quality` yet).

- [ ] **Step 3: Implement**

`reveal.rs`:

```rust
pub fn reveal_command(path: &Path) -> (String, Vec<String>) {
    if cfg!(target_os = "macos") {
        ("open".into(), vec!["-R".into(), path.display().to_string()])
    } else if cfg!(target_os = "windows") {
        ("explorer".into(), vec![format!("/select,{}", path.display())])
    } else {
        let folder = path.parent().unwrap_or(path);
        ("xdg-open".into(), vec![folder.display().to_string()])
    }
}
```

`resources/icons/stop.svg`:

```svg
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16" width="16" height="16">
<rect x="0" y="0" width="16" height="16" fill="none"/>
<rect x="4" y="4" width="8" height="8" rx="1.5" fill="#000"/>
</svg>
```

`theme.rs`: `reco_quality_width: 84.0` (next to `reco_aspect_width`).

`ui/viewer.rs`, in the view bar, replace the record `Tip` with:

```
            Tip{text: "Recording quality"
                record_quality := RecoDropDown{
                    animator +: {disabled: {default: @on}}
                    width: theme.reco_quality_width labels: ["Fast" "Balanced" "High"]
                }
            }
            // While recording: a red dot and the time recorded.
            recording_badge := View{
                visible: false
                width: Fit height: Fit flow: Right spacing: theme.reco_gap_s align: Align{y: 0.5}
                RecoDotError{}
                recording_time := RecoText{text: "0:00" draw_text +: {color: theme.reco_record}}
            }
            Tip{text: "Record the preview as you watch"
                record_button := RecoIconButton{
                    animator +: {disabled: {default: @on}}
                    icon_walk: Walk{width: theme.reco_row_icon height: theme.reco_row_icon}
                    draw_icon +: {svg: crate_resource("self:resources/icons/record.svg") color: theme.reco_record}
                }
            }
```

`ui/time_panel.rs`, in the controls row after `status_text`:

```
            // After a recording: the file in Finder.
            show_in_folder := RecoFlatButton{visible: false text: "Show in folder"}
```

`main.rs`:
- fields `#[live] record_icon: Option<ScriptHandleRef>,` and `#[live] stop_icon: Option<ScriptHandleRef>,`, and in the startup DSL:

```
        record_icon: crate_resource("self:resources/icons/record.svg")
        stop_icon: crate_resource("self:resources/icons/stop.svg")
```

- in `apply_settings`: `self.ui.drop_down(cx, ids!(record_quality)).set_selected_item(cx, self.settings.quality().index());`;
- in `apply_shell`, next to the aspect line: `self.ui.widget(cx, ids!(record_quality)).set_disabled(cx, !loaded);`;
- in `handle_actions`: `self.record_actions(cx, actions);`;
- in `handle_event`'s first `match`, add `Event::Shutdown => self.finish_recording_on_quit(),`.

`live.rs`: `Live` gains

```rust
    /// Frames recorded so far, while recording.
    pub recording: Option<u64>,
    /// The last file written, for Show in folder.
    pub last_output: Option<PathBuf>,
```

(both `None` in `Live::new`), and `status()` passes the recording time:

```rust
            recording: self
                .recording
                .map(|frames| frames as f64 / self.info.as_ref().map_or(30.0, |i| i.fps.max(1.0))),
```

`session_view.rs`. Replace `set_play_icon`'s body with `self.swap_icon(cx, ids!(play_pause), want)` and add:

```rust
    /// Swap a button's icon in place (Makepad reloads it when the handle
    /// changes).
    pub(crate) fn swap_icon(&mut self, cx: &mut Cx, button: &[LiveId], icon: Option<ScriptHandleRef>) {
        let button = self.ui.button(cx, button);
        if let Some(mut inner) = button.borrow_mut() {
            let same = inner.draw_icon.svg.as_ref().map(|h| h.as_handle()) == icon.as_ref().map(|h| h.as_handle());
            if !same {
                inner.draw_icon.svg = icon;
            }
        }
        button.redraw(cx);
    }

    /// Record or stop. A recording is 1080 rows at the preview aspect, in
    /// the saved folder or beside the left video, named as the Slint app
    /// named them.
    fn toggle_recording(&mut self) {
        let Some(live) = self.live.as_ref() else { return };
        if live.recording.is_some() {
            live.worker.send(PreviewCommand::StopRecording);
            return;
        }
        let first_left = live.files.left.first().cloned().unwrap_or_default();
        let folder = recording_folder(self.settings.recording_folder.as_deref(), &first_left);
        let now = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
        live.worker.send(PreviewCommand::StartRecording {
            path: folder.join(recording_file_name(now)),
            size: recording_size(self.settings.aspect()),
            quality: self.settings.quality(),
        });
    }

    /// The view bar while recording: the badge and Stop; otherwise the
    /// quality and Record.
    fn show_recording(&mut self, cx: &mut Cx, recording: bool) {
        self.set_visible(cx, ids!(recording_badge), recording);
        self.set_visible(cx, ids!(record_quality), !recording);
        if recording {
            self.set_visible(cx, ids!(show_in_folder), false);
            self.set_label(cx, ids!(recording_time), "0:00");
        }
        let icon = if recording { self.stop_icon.clone() } else { self.record_icon.clone() };
        self.swap_icon(cx, ids!(record_button), icon);
    }

    fn recording_started(&mut self, cx: &mut Cx, path: PathBuf) {
        if let Some(live) = self.live.as_mut() {
            live.recording = Some(0);
        }
        self.show_recording(cx, true);
        self.toast(cx, Severity::Info, "Recording started", &path.display().to_string());
    }

    fn recorded(&mut self, cx: &mut Cx, frames: u64) {
        let Some(live) = self.live.as_mut() else { return };
        live.recording = Some(frames);
        let fps = live.info.as_ref().map_or(30.0, |i| i.fps.max(1.0));
        let status = live.status();
        self.set_label(cx, ids!(recording_time), &time_ruler::clock(frames as f64 / fps));
        self.set_label(cx, ids!(status_text), &status);
    }

    fn recording_saved(&mut self, cx: &mut Cx, recording: Recording) {
        let file = recording.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        if let Some(live) = self.live.as_mut() {
            live.recording = None;
            live.last_output = Some(recording.path.clone());
        }
        self.show_recording(cx, false);
        self.set_visible(cx, ids!(show_in_folder), true);
        self.toast_for(cx, Severity::Info, "Recording saved", &format!("{} frames · {file}", recording.frames), Duration::from_secs(8));
        self.refresh_status(cx);
    }

    fn recording_failed(&mut self, cx: &mut Cx, reason: &str) {
        if let Some(live) = self.live.as_mut() {
            live.recording = None;
        }
        self.show_recording(cx, false);
        self.toast(cx, Severity::Error, "Recording failed", reason);
        self.refresh_status(cx);
    }

    fn refresh_status(&mut self, cx: &mut Cx) {
        if let Some(status) = self.live.as_ref().map(Live::status) {
            self.set_label(cx, ids!(status_text), &status);
        }
    }

    /// Record, the quality, and Show in folder.
    pub(crate) fn record_actions(&mut self, cx: &mut Cx, actions: &Actions) {
        if self.ui.button(cx, ids!(record_button)).clicked(actions) {
            self.toggle_recording();
        }
        if let Some(index) = self.ui.drop_down(cx, ids!(record_quality)).selected(actions) {
            self.settings.set_quality(RecordingQuality::from_index(index));
            self.save_settings();
            if self.pointer_input {
                cx.set_key_focus(Area::Empty);
            }
        }
        if self.ui.button(cx, ids!(show_in_folder)).clicked(actions) {
            if let Some(path) = self.live.as_ref().and_then(|l| l.last_output.clone()) {
                if let Err(e) = reco_app::reveal::reveal(&path) {
                    error!("couldn't show {}: {e}", path.display());
                }
            }
        }
    }

    /// Quitting while recording: stop and wait up to 3 s for the file to
    /// close, so it plays. The one time the UI thread waits (DESIGN Rule 4).
    pub(crate) fn finish_recording_on_quit(&mut self) {
        let Some(live) = self.live.as_ref() else { return };
        if live.recording.is_none() {
            return;
        }
        live.worker.send(PreviewCommand::StopRecording);
        let deadline = Instant::now() + Duration::from_secs(3);
        while Instant::now() < deadline {
            match live.worker.try_event() {
                Some(PreviewEvent::RecordingSaved(_) | PreviewEvent::RecordingFailed(_)) => return,
                Some(_) => {}
                None => std::thread::sleep(Duration::from_millis(10)),
            }
        }
    }
```

Add to `drain_preview`:

```rust
                Some(PreviewEvent::RecordingStarted { path }) => self.recording_started(cx, path),
                Some(PreviewEvent::Recorded { frames }) => self.recorded(cx, frames),
                Some(PreviewEvent::RecordingSaved(recording)) => self.recording_saved(cx, recording),
                Some(PreviewEvent::RecordingFailed(reason)) => self.recording_failed(cx, &reason),
```

Imports: `std::path::PathBuf`, `std::time::{Duration, SystemTime, UNIX_EPOCH}`, `reco_app::preview::recorder::Recording`, `reco_app::recording::{recording_file_name, recording_folder, recording_size, RecordingQuality}`.

- [ ] **Step 4: Run the tests and checks**

Run: `cargo test -p reco-app reveal && cargo test -p reco-desktop && cargo clippy -p reco-app -p reco-desktop --no-deps --all-targets -- -D warnings && cargo build --profile desktop -p reco-desktop && /usr/bin/python3 crates/reco-desktop/tools/check_m2.py record toasts transport && /usr/bin/python3 crates/reco-desktop/tools/check_m0.py`
Expected:
- tests pass and clippy is clean;
- every `record`, `toasts` and `transport` line is `ok`;
- the Module 0 check passes.

The view bar gained the quality dropdown and the badge. If a Module 0 expectation measured the view bar's trailing icons against fixed neighbours, update that expectation to the new layout and record a ruling.

- [ ] **Step 5: Commit**

```bash
git add crates/reco-app/src crates/reco-desktop/src crates/reco-desktop/resources/icons/stop.svg crates/reco-desktop/tools
git commit -m "feat(desktop): record the preview: quality, a badge, toasts, Show in folder; quitting finishes the file"
```

---

### Task 13: Cheaper frames: cached side panels and draw timing (desktop)

Every preview frame redraws the window's draw list, which re-walks every uncached widget. Caching the static side panels (`new_batch: true` gives a View its own draw list, skipped when clean) cuts that walk. Rule 8 (UI work under 4 ms a frame) gets a measurement: `--perf-log` logs the time spent drawing every two seconds.

**Files:**
- Create: `crates/reco-desktop/src/perf.rs`
- Modify: `src/main.rs`, `src/cli.rs`, `src/ui/shell.rs`
- Modify: `crates/reco-desktop/tools/check_m2.py`

**Interfaces:**
- Produces:
  - `perf::WINDOW: Duration` (2 s).
  - `perf::DrawStats::add(&mut self, spent: Duration, now: Instant) -> Option<String>`. It returns a line `"ui draw: N frames, avg A ms, max M ms"` once per window.
  - `cli::Args.perf_log: bool` (`--perf-log`).

- [ ] **Step 1: Write the failing tests and check**

`src/perf.rs`:

```rust
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
        todo!()
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
        assert_eq!(stats.add(Duration::from_millis(4), t0 + Duration::from_secs(1)), None);
        let line = stats.add(Duration::from_millis(3), t0 + WINDOW).expect("a summary");
        assert_eq!(line, "ui draw: 3 frames, avg 3.00 ms, max 4.00 ms");
        assert_eq!(stats.add(Duration::from_millis(1), t0 + WINDOW + Duration::from_millis(10)), None);
    }
}
```

`cli.rs` test:

```rust
    #[test]
    fn perf_log_flag() {
        assert!(Args::parse(["--perf-log"]).unwrap().perf_log);
    }
```

with the field (documented "Log how long drawing takes, `--perf-log` (checks)"): `pub perf_log: bool,`.

`check_m2.py`:

```python
def check_perf():
    """Rule 8: drawing a frame takes under 4 ms on average while playing."""
    for name, files in (("fast", FAST), ("real", REAL)):
        if not all(os.path.exists(p) for p in files):
            print(f"skip: perf {name}: fixtures not found")
            continue
        with launch(files, extra=("--perf-log",)) as app:
            ready(app)
            app.key("space")
            time.sleep(6.5)
            app.key("space")
            lines = [line for line in app.log_lines() if "ui draw:" in line]
        averages = [float(line.split("avg ")[1].split(" ms")[0]) for line in lines]
        print(f"perf {name}: {lines[-1].split('- ')[-1] if lines else 'no summary'}")
        expect(len(averages) >= 2, f"perf {name}: draw times are logged ({len(averages)} summaries)")
        expect(all(a < 4.0 for a in averages), f"perf {name}: under 4 ms a frame on average ({averages})")
```

Register `"perf": check_perf,`. Add `mod perf;` to `main.rs`.

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test -p reco-desktop -- summarises perf_log && cargo build --profile desktop -p reco-desktop && /usr/bin/python3 crates/reco-desktop/tools/check_m2.py perf`
Expected: the unit tests fail (`not yet implemented` / flag ignored); the check fails with "draw times are logged (0 summaries)".

- [ ] **Step 3: Implement**

`perf.rs`:

```rust
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
        *self = Self { since: Some(now), ..Self::default() };
        Some(line)
    }
```

`cli.rs`, in the loop: `} else if arg == "--perf-log" { out.perf_log = true;`.

`main.rs`: field `#[rust] draw_stats: perf::DrawStats,` (documented); in `handle_event`, replace the last line with:

```rust
        let draw = matches!(event, Event::Draw(_));
        let started = std::time::Instant::now();
        self.ui.handle_event(cx, event, &mut Scope::empty());
        if draw && self.args.perf_log {
            if let Some(line) = self.draw_stats.add(started.elapsed(), std::time::Instant::now()) {
                log!("{line}");
            }
        }
```

`ui/shell.rs`: give the static side panels their own draw lists (skipped when clean):

```
            a: View{width: Fill height: Fill new_batch: true media_panel := RecoMediaPanel{}}
```

```
                    b: View{width: Fill height: Fill new_batch: true inspector := RecoInspector{}}
```

and add to the module doc: "The side panels keep their own draw lists, so a playing preview doesn't redraw them."

- [ ] **Step 4: Run the tests and checks**

Run: `cargo test -p reco-desktop && cargo build --profile desktop -p reco-desktop && /usr/bin/python3 crates/reco-desktop/tools/check_m2.py perf && /usr/bin/python3 crates/reco-desktop/tools/check_m0.py && /usr/bin/python3 crates/reco-desktop/tools/check_m1.py`
Expected: the unit tests pass; `perf` reports averages under 4 ms for each fixture set present; Module 0 and Module 1 still pass (cached panels draw the same).

- [ ] **Step 5: Commit**

```bash
git add crates/reco-desktop/src crates/reco-desktop/tools/check_m2.py
git commit -m "perf(desktop): cache the side panels; measure draw time with --perf-log"
```

---

### Task 14: The Module 2 check, parity and docs

**Files:**
- Modify: `crates/reco-desktop/PARITY.md`, `DESIGN.md`, `FRICTION.md`

- [ ] **Step 1: Run the whole check**

Run: `cargo build --profile desktop -p reco-desktop && /usr/bin/python3 crates/reco-desktop/tools/check_m2.py`
Expected: every line `ok`; "Module 2 check passed". Fix in one batch whatever fails, then run it once more.

- [ ] **Step 2: Look at the screenshots**

View, in `target/desktop-checks/m2/`:
- `step-a.png`, `icon-playing.png`
- `lanes-chained.png` (two blocks per camera and a gap at 1:00)
- `export-range.png` (the tint)
- the toast grabs (from `check_toasts`, add `app.grab(os.path.join(OUT, "toasts.png"))` after the four-card check)

Fix visual problems in one batch (card spacing, the badge's alignment, the tint's strength) and confirm once.

- [ ] **Step 3: PARITY.md**

Tick each Module 2 item with its evidence, in the Module 1 style (check line names, unit test names, screenshots). Replace the export line with two:

```
- [x] Show in folder after a recording. Evidence: "record: Show in folder
      appears"; `finder_selects_the_file`.
- [ ] Export progress, status text and Cancel; Show in folder after an
      export. → Module 6 (the export job). The card exists since Module 0.
```

"Status text … version, Report bug" is evidenced by:
- the status line checks ("Ready", the fps reading, "Paused", "Finished", "Recording ·");
- error toasts ("a failed open raises an error toast");
- version and "Report a bug…" in the app menu (Module 0; the dialog arrives in Module 7).

- [ ] **Step 4: DESIGN.md and FRICTION.md**

DESIGN.md, under the preview bridge:
- **Recording:** its own renderer at 1080 rows, NV12 readback, an encoder thread; one frame per source frame shown, flushed on stop; quitting waits up to 3 s.
- **Toasts:** four slots in the viewer, fed by `reco_app::toasts`.
- **Settings:** in `desktop.json`.
- **The ruler:** seeks on release and holds its target until the seek lands.

Under "Makepad behaviour the rules work around":
- Dragging is captured automatically from a press. Emit several actions per pass with `filter_widget_actions_cast`.
- A button's icon is swapped by assigning `draw_icon.svg`.
- `new_batch: true` caches a View's subtree; a changing widget's own list does not stop the window walk.
- The built-in `Toaster` doesn't hit-test, ignores its rect and has fixed lifetimes.
- `Event::Shutdown` comes before quitting.

FRICTION.md entries:
1. The encoder ignores timestamps (constant rate): recordings are one frame per source frame.
2. `StitchPipeline::resize` keeps the first render target: the recorder has its own renderer.
3. `total_frames()` ignores the sync offset, and a failed seek looks like the end of the stream: lengths come from probing the files, and seek failures are reported.
4. File durations are private and there is no frame count: one `VideoDecoder::open` per file, on a probe thread.

- [ ] **Step 5: Full verification and commit**

Run: `cargo test -p reco-app -- --test-threads=1 && cargo test -p reco-desktop && cargo fmt --check -p reco-app -p reco-desktop && cargo clippy -p reco-app -p reco-desktop --no-deps --all-targets -- -D warnings && /usr/bin/python3 crates/reco-desktop/tools/test_drive.py && /usr/bin/python3 crates/reco-desktop/tools/check_m2.py && /usr/bin/python3 crates/reco-desktop/tools/check_m1.py && /usr/bin/python3 crates/reco-desktop/tools/check_m0.py`
Expected: all pass.

```bash
git add crates/reco-desktop/PARITY.md crates/reco-desktop/DESIGN.md crates/reco-desktop/FRICTION.md crates/reco-desktop/tools
git commit -m "test(desktop): Module 2 check for the time panel and status; record parity"
```

---

## Self-review notes

- **Spec coverage (PARITY Module 2, DESIGN row 2):**
  - step/play/pause, enabled when loaded: Task 9 (gating from Module 0's `apply_shell`);
  - current and total time: Tasks 9 and 10 (the total from the probed files);
  - timeline, seek on release, export-range tint: Task 10;
  - record/stop with state colours, quality hidden while recording: Tasks 6, 7 and 12;
  - preview aspect persisted: Task 8;
  - status text (error channel), fps: Task 9, with errors also as toasts in Task 11; version and Report bug: the Module 0 app menu;
  - Show in folder after a recording: Task 12; export progress and Show in folder after an export: Module 6;
  - toasts: Tasks 3 and 11;
  - lanes from the real files: Task 5 and Task 10.
- **Module 1 deferrals folded in:**
  - the play icon swap, the status line when paused or finished, and the fps meter reset on pause (Task 9);
  - key repeats not toggling (Task 9);
  - draw time measured, and the side panels cached (Task 13);
  - worker panics reported (Task 4).
- **Still deferred, with owners:**
  - readback backpressure and buffer reuse (before Module 3's dialogs);
  - wheel-notch scale and layout-dependent keys (Module 5/7);
  - command-queue consistency;
  - the first frame at 1280×720;
  - a resize-while-playing test.
- **Rule 4:** probing, seeking, rendering and encoding run on worker threads. The UI thread writes settings and spawns `open -R`, and waits only at quit while recording.
- **Rule 8:** about 0% CPU while paused (Module 1's check still runs); under 4 ms draw time (Task 13's check).
