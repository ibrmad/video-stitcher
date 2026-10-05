# Parity checklist

Every behaviour of the Slint app (`crates/reco-gui`) that the new app must
have, grouped by module. Sources are the section numbers in
[the Slint inventory](docs/slint-inventory.md). An item is ticked only with
evidence: a `tools/check_m<N>.py` step and/or a screenshot path under
`target/desktop-checks/`. "New" marks behaviour the Slint app lacks or gets
wrong (see the issues list in [DESIGN.md](DESIGN.md)).

## Module 0: shell and look (§1, §4)

The look follows the Rerun viewer (DESIGN.md, Look). Evidence for this
module: `tools/check_m0.py` (837 checks over every `--look-preview` state:
start, one camera, both cameras, calibrating, calibration failed, ready,
exporting; at 720×600, 1280×820 and 1920×1200), the unit tests
(`cargo test -p reco-desktop`), and screenshots in
`target/desktop-checks/m0/`.

- [x] Window: title "Reco", default 1280×820. It stays usable down to
      720×600 by folding panels, because Makepad has no minimum-size API.
      Evidence: "window title is Reco", "window size … matches",
      "ready-720x600: Adjust panel closed", "fold hint shown".
- [x] Title bar: the "Reco" app menu (keyboard shortcuts, preferences,
      Report a bug, the version), the project, Export (primary; disabled
      until there is a stitched preview), and the Setup, time panel and
      Adjust toggles. Evidence: "`app_menu` … is on screen", "menu:
      `app_menu` opens a menu", "Export face is green / grey", "disabled
      Export has no green ink"; `ready-app_menu.png`. Every icon file
      exists: `every_self_resource_exists`.
- [x] Setup panel on the left: default width 260, minimum 200, resizable,
      open at startup. Cameras (the linked pair: Add, then Change; file
      count and length) and Calibration (status, detail, Auto-calibrate or
      Recalibrate, Load file) in section bands; recent files in a menu.
      Evidence: "Setup panel open", "`<id>` shown / hidden" per state,
      "camera link is lit", "dragging the bar widens Setup", "Setup stops
      at its 200 pt minimum", "menu: `recent_menu` opens a menu".
- [x] Viewer: a view bar (Preview, aspect, Record) over a black canvas; the
      next step with the panorama frame and stepper until a stitch exists;
      calibration progress in that column; the export card with Cancel; a
      calibration-failed state with Try again. Evidence: "viewport is
      #000000", "empty state shown", "stepper not covered while
      calibrating", "`export_card` shown", "`cal_dot_error` shown".
- [x] Adjust panel on the right: default width 280, minimum 200,
      resizable. View and Stitch sections of property rows with tooltips,
      Advanced tiers closed. Evidence: "Adjust panel open / closed",
      "Adjust stops at its 200 pt minimum", "Adjust panel stays closed
      before a stitch", `adjust-1280x820.png`.
- [x] Time panel across the bottom, showing only what exists: step, play
      (the largest of the three) and a status line; the time once there is
      a stitch; once a camera has video, a ruler and a lane per camera (its
      badge, its files as slim blocks); the playhead over a stitched
      preview; folds to its control row. Controls take no input before a
      stitch. Evidence: "step icons smaller than play", "`lanes` /
      `time_display` shown / hidden", "lane 1 / 2 shows files / no video",
      "playhead shown / hidden", "`step_back` / `play_pause` /
      `step_forward` / `timeline` / `aspect` / `record_button` takes /
      ignores input" (the `/snap` input flag), "time panel folds to its
      control row"; `ready-time-folded.png`.
- [x] All colours, spacing, radii and type sizes come from `src/theme.rs`.
      Dark only; light tokens arrive with the dark-mode preference
      (Module 7). Evidence: the `screens_use_theme_values_only` unit test
      (no raw colours, sizes, spacing or type sizes in `src/ui/`),
      `tools/check_theme.py`, and "Setup panel is #0d0d0d", "section band
      is #212121".
- [x] One content edge per panel, trailing icons on the right edge, one line
      per control row (DESIGN.md Rule 11). Evidence: "`<id>` text starts on
      the edge", "`<id>` starts on the edge", "… ends on the content edge"
      (Recent files, help, Change and band icons, Adjust values), "time
      panel's first icon starts on the edge 12", "`<id>` centred on the
      control row".
- [x] Looks right at 720×600, 1280×820 and 1920×1200, in every state.
      Evidence: the screenshots, reviewed in two batched rounds after the
      Rerun restyle (fixes: the app menu button's border, empty camera
      rows, trailing icon alignment, the checkbox column, the time panel's
      edge, dark menus).
- [x] Owner approved the look (2026-10-05: "good enough for now", after the
      Rerun restyle and the time panel tidy-up).

## Module 1: preview (§2.3, §6)

- [ ] Stitched preview in the viewer, fitted with `contain`.
- [ ] Preview aspect: auto, 16:9, 4:3, 21:9.
- [ ] Drag pans the view (X inverted, as in Slint); the wheel zooms
      (FOV change = -dy/40 degrees).
- [ ] Keys: arrows pan 20 px; `+`/`=` zoom in 5°; `-`/`_` zoom out 5°; R
      resets the view; F/F11 toggle fullscreen; Space plays or pauses;
      `[`/`]` seek ∓5 s.
- [ ] Playback advances on vsync through the zero-copy bridge.
- [ ] Render target follows the viewer size in physical pixels (New: DPI-aware).
- [ ] The portable readback path compiles and renders when forced.
- [ ] Autoload for checks: left, right and calibration paths from the
      command line (replaces the Slint `automation` feature's RECO_AUTOLOAD).

## Module 2: transport and status (§2.5, §2.6, §2.13)

- [ ] Step back, play/pause, step forward; enabled when files are loaded.
- [ ] Current and total time.
- [ ] Timeline over all frames; seek on release; export-range tint.
- [ ] Record / Stop with recording state colours; quality (fast, balanced,
      high) hidden while recording.
- [ ] Preview aspect selector, persisted.
- [ ] Status text (also the error channel), fps readout, version, Report bug.
- [ ] Export progress, status text and Cancel in the status bar; Show in
      folder after an export or recording.
- [ ] Toasts: info, warn and error with TTL 4/7/10 s (plus custom), at most
      4, dismissable, clear of the Inspector. (New: rendered reliably, and
      they do not overwrite the status line.)

## Module 3: files (§2.2, §2.7)

- [ ] Left: add videos (multi-select mp4/mov/avi/mkv, appended as
      segments), Clear, segment list with remove and drag-to-reorder,
      "No video selected".
- [ ] Right: the same.
- [ ] Calibration: Auto Calibrate / Re-calibrate / Calibrating… with step
      text; Load…; calibration file chip with remove. (New: Cancel, and
      Advanced locked while calibrating.)
- [ ] Calibration Advanced: frames (2/4/6/8), IMU seeds, AKAZE threshold,
      Detect Y min and max, skip end.
- [ ] Recent files dialog: left videos, right videos, calibrations; click
      loads; Clear all; Close.
- [ ] (New) Drop videos onto the window.

## Module 4: stitching and calibration (§2.2, §2.4, §2.14)

- [ ] Seam blend 0–0.3; Match colours; rig tilt −30..30°; rig roll
      −15..15°; sync offset in frames with Apply.
- [ ] Save Calibration shown when calibration or lens is dirty.
- [ ] Intersect −1..1, camera axis offset −0.6..0.6, `x_ty` −0.1..0.1,
      Reset.
- [ ] Field ROI status; Set/Edit ROI (browser editor); Paste ROI from the
      clipboard or a JSON field. (New: image preparation off the UI thread.)
- [ ] ROI points drawn over the lens preview. (New: aligned to the image.)

## Module 5: camera and lens (§2.4, §2.12)

- [ ] FOV 20–150°, constrained look, Reset View.
- [ ] Lens info for L and R, or "Auto-calibrate to detect lens".
- [ ] Browse profiles…, lens correction, lens preview (single camera) with
      Left/Right.
- [ ] Fine-tune: Left/Right/Both; fx, fy, cx, cy, k1–k4; Reset Lens.
- [ ] Lens picker: apply to Both/Left/Right, search the profile database,
      pick a result, Load from file…, Close. (New: search off the UI thread.)
- [ ] Stats: fps, frame times, bottleneck, GPU, dropped frames, AI and
      calibration figures. (New: fed during preview too.)

## Module 6: export (§2.11, §7)

- [ ] Output path with Save to… (adds `.mp4` when there is no extension).
- [ ] Resolution (1080p, 720p, 2K, 4K) with the size shown; codec (from the
      probed list; New: no fixed indices); quality.
- [ ] Processing range: start and end as sliders and text, duration, and an
      empty-range warning.
- [ ] Record replay; save AI debug events.
- [ ] AI tracking: Enable (when available) and status; model picker with a
      missing-model warning; tracking mode; detect every N frames; style
      preset; framing; pitch lock; lookahead with VRAM risk zones; advanced
      panner (cluster mode, ball weight, cluster bandwidth, dead zone, FOV
      tight/default/wide).
- [ ] Error text, Cancel, Start Export with its enable rules.
- [ ] During an export: preview paused overlay, progress, Cancel. Afterwards
      the preview is rebuilt. (New: the playback position is kept.)

## Module 7: preferences and help (§2.8–§2.10, §8)

- [ ] Preferences: default codec, quality and seam blend; AI model path with
      Browse; recording codec, quality and folder with Browse; dark mode;
      telemetry opt-in; Save. (New: Cancel reverts everything.)
- [ ] Keyboard shortcuts dialog, matching what is implemented; Website;
      Forum.
- [ ] Report a bug: message, contact, include logs, Send. (New: honours the
      telemetry opt-in; clipboard copy is explicit.)
- [ ] Update check shows a toast with a link. (New: it no longer opens the
      browser by itself.)
- [ ] Settings persistence: recent files, defaults, recording, preview
      aspect, telemetry, dark mode. (New: window size, maximized state and
      panel widths are restored.)

## Module 8: parity sweep

- [ ] Every item above ticked with evidence.
- [ ] Performance pass against the DESIGN.md speed targets.
- [ ] Owner sign-off; Slint app removed.
