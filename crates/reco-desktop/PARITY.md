# Parity checklist

Every behaviour of the Slint app (`crates/reco-gui`) that the new app must
have, grouped by module. Sources are the section numbers in
[the Slint inventory](docs/slint-inventory.md). An item is ticked only with
evidence: a `tools/check_m<N>.py` step and/or a screenshot path under
`target/desktop-checks/`. "New" marks behaviour the Slint app lacks or gets
wrong (see the issues list in [DESIGN.md](DESIGN.md)).

## Module 0: shell and look (§1, §4)

Evidence for this module: `tools/check_m0.py` (658 checks over every
`--look-preview` state: start, one camera, both cameras, calibrating, ready,
exporting), the unit tests (`cargo test -p reco-desktop`), and screenshots
in `target/desktop-checks/m0/`.

- [x] Window: title "Reco", default 1280×820. It stays usable down to
      720×600 by folding panels, because Makepad has no minimum-size API.
      Evidence: "window title is Reco", "window size … matches",
      "loaded-720x600: Inspector folded"; `loaded-720x600.png`.
- [x] Top bar: Media panel toggle, title, Help (shortcuts), Preferences,
      Export (primary; disabled until files are loaded), Inspector toggle.
      Evidence: "`toggle_media` / `export_button` / `toggle_inspector` is
      on screen", "Export is disabled" / "Export is enabled (accent)".
      Every icon file exists: `every_self_resource_exists`.
- [x] Media sidebar on the left: default width 260, minimum 180, resizable,
      opens at startup when nothing is loaded. Evidence: "Media panel open",
      "dragging the bar widens Media", "Media stops at its 180 pt minimum";
      `loaded-media-widened.png`.
- [x] Viewer with empty state: title, the three steps, status text.
      Evidence: "empty state shown"; `empty-1280x820.png`.
- [x] Inspector on the right: default width 280, minimum 200, resizable.
      Evidence: "Inspector open", "Inspector stops at its 200 pt minimum",
      "Inspector closed before load".
- [x] Transport bar placeholder, disabled until files are loaded.
      Evidence: "`step_back` / `play_pause` / `step_forward` /
      `record_button` / `timeline` / `aspect` disabled" (the `/snap` input
      flag, in every state), and `empty-1280x820.png` (dimmed).
- [x] Status bar: status text, version, Report bug link. Evidence:
      "`status_text` / `version_text` / `report_bug` is on screen".
- [x] All colours, spacing, radii and type sizes come from `src/theme.rs`.
      Dark only; light tokens arrive with the dark-mode preference
      (Module 7). Evidence: the `screens_use_theme_values_only` unit test
      (no raw colours, sizes, spacing or type sizes in `src/ui/`),
      `tools/check_theme.py`, and the background pixel checks.
- [x] Looks right at 720×600, 1280×820 and 1920×1200. Evidence: all 12
      screenshots reviewed. Fixes from that review: the Media panel
      scrolls, disabled icons fade, the timeline value is hidden, and the
      settings icon is clearer.
- [x] Critique pass (contrast, type, information architecture, copy,
      states, match character): Setup and Adjust panels, the next-step
      viewer with the panorama frame and stepper, calibration progress in
      the viewer column, the export card, one primary per screen.
      Evidence: "`<id>` shown / hidden" and "takes / ignores input" for
      every state, "Setup panel's Auto-calibrate is secondary", "camera
      link is lit", "disabled Export has no green ink", "stepper not
      covered while calibrating", "next step clear of the viewer's right
      edge" (720×600).
- [x] One edge per card, one line per row (DESIGN.md Rule 11). Evidence:
      "`<id>` text starts on the content edge" for card titles, hints,
      status words and details, panel headers and Adjust controls;
      "`<id>` centred on the transport row" and "timeline track centred on
      the transport row".
- [ ] Owner approved the look.

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
