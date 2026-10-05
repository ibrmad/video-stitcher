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

Evidence for this module: `tools/check_m1.py`, which opens the alfheim pair
zero-copy and again with `--preview-readback`, opens a missing file, and
plays the 5.3K match pair; the unit tests (`cargo test -p reco-app -p
reco-desktop`); and screenshots in `target/desktop-checks/m1/`. Each check
line below is quoted from the zero-copy run; the readback run has the same
lines.

- [x] Stitched preview in the viewer, fitted with `contain`. Evidence: "the
      preview appears", "the empty state is gone", "a real picture";
      `ready-zero-copy.png`, `real-5k.png`. A missing file says so instead:
      "bad file: the viewer says it couldn't open the videos", "no preview
      is drawn"; `bad-file.png`. So does a right file that is not a video
      ("bad video: it says no frame could be decoded", "Play stays
      disabled"; `bad-video.png`) and a pair of different sizes
      (`videos_of_different_sizes_fail_to_open`).
- [x] Preview aspect: auto, 16:9, 4:3, 21:9. Evidence: "the preview
      letterboxes to 4:3 (734x550)", `aspect-4x3-zero-copy.png`;
      `aspects_follow_the_dropdown_order`, `fit_letterboxes_and_pillarboxes`.
- [x] Drag pans the view (X inverted, as in Slint); the wheel zooms
      (FOV change = -dy/40 degrees). Evidence: "dragging pans", "the wheel
      zooms"; scrolling away zooms in (Makepad's `scroll.y` is the negated
      macOS delta, DESIGN.md).
- [x] Keys: arrows pan 20 px; `+`/`=` zoom in 5°; `-`/`_` zoom out 5°; R
      resets the view; F/F11 toggle fullscreen; Space plays or pauses;
      `[`/`]` seek ∓5 s. Evidence: "Space plays", "arrow keys pan", "= zooms
      in", "R resets the view", "F toggles fullscreen", "] seeks 5 s"; keys
      survive mouse use: "keys still reach the preview after clicking Play",
      "keys reach the preview after a mouse pick of the aspect", "Space
      plays after clicking a panel toggle", while "Space on a Tab-focused control leaves the preview
      alone"; Space after the end plays again (`space_after_the_end_restarts`);
      the mapping of every key, with ⌘/Ctrl/Option ignored:
      `maps_the_slint_shortcuts`, `shift_still_counts`,
      `modified_keys_are_ignored`.
- [x] Playback advances on vsync through the zero-copy bridge. Frames are
      paced by the worker's clock at the source rate and shown on Makepad's
      next frame. Evidence: "renders zero-copy" (the log line `preview:
      1280x960 input, zero-copy on Apple M1 Pro`), "the picture moves while
      playing", "paused frames stay still", "about 0% CPU while paused"; the
      5.3K pair decodes and shows about 30 frames a second (source 29.97).
      The ring's hand-over is unit-tested: `ring_waits_for_adoption`,
      `a_replaced_slot_retires_after_its_beats`,
      `a_slot_also_waits_its_minimum_time`; a different GPU falls back to
      readback: `another_gpu_falls_back_to_readback`.
- [x] Render target follows the viewer size in physical pixels (New:
      DPI-aware). Evidence: `render_size_counts_physical_pixels`,
      `render_size_caps_at_4k_keeping_the_aspect`; the 4:3 run re-renders
      at the letterboxed size.
- [x] The portable readback path compiles and renders when forced.
      Evidence: every check above passes again with `--preview-readback`
      ("renders readback"); `readback_renders_and_plays`.
- [x] Autoload for checks: left, right and calibration paths from the
      command line (replaces the Slint `automation` feature's RECO_AUTOLOAD).
      Evidence: the whole check launches through `--left/--right/
      --calibration`; `parses_the_three_files`,
      `chains_files_split_by_semicolons`, `files_must_come_as_a_set`.

## Module 2: transport and status (§2.5, §2.6, §2.13)

Evidence for this module: `tools/check_m2.py` (checks `persist`,
`transport`, `ruler`, `toasts`, `record` and `perf` on the alfheim pair, the
chained pair `cam0.mp4;cam0.mp4`, and the 5.3K match pair for `perf`); the
unit tests (`cargo test -p reco-app -p reco-desktop`); and screenshots in
`target/desktop-checks/m2/`. Check lines are quoted as they print.

- [x] Step back, play/pause, step forward; enabled when files are loaded.
      Evidence: "transport: step forward shows the next frame", "transport:
      step back shows the frame before again", "transport: the play button
      shows pause while playing" (`icon-paused.png`, `icon-playing.png`),
      "transport: Space after the end plays from the start"; enabling is
      Module 0's `apply_shell` gate. Holding a key repeats moves and seeks,
      never play (`toggles_do_not_repeat`); a burst of seeks costs one seek
      (`relative_seeks_accumulate`, `seek_burst_lands_on_the_sum`).
- [x] Current and total time. Evidence: "ruler: the clock follows a scrub",
      "ruler: the length comes from the files" (2:00 for the chained pair);
      `length_is_where_both_cameras_have_video`, `lanes_arrive_after_open`.
- [x] Timeline over all frames; seek on release; export-range tint.
      Evidence: "ruler: releasing seeks there", "ruler: a gap between the two
      files" (`lanes-chained.png`), "ruler: the export range is tinted"
      (`export-range.png`); seeks go by frame index (`seek_to_frame_shows_that_frame`),
      and one past the real end fails and keeps the frame
      (`seek_past_the_end_is_an_error`).
- [x] Record / Stop with recording state colours; quality (fast, balanced,
      high) hidden while recording. Evidence: "record: the quality shows
      before recording", "record: the badge shows while recording", "record:
      the quality hides while recording", "record: 1920x1080 for Auto",
      "record: about 3 s at 30 fps, one frame per frame played", "record:
      quitting while recording leaves a file", "record: and it plays"
      (`recording.png`); `recording_has_one_frame_per_source_frame`,
      `recording_keeps_its_size_when_the_preview_resizes`,
      `pausing_adds_no_frames`, `quitting_while_recording_finishes_the_file`.
      (New: the whole picture at 1080 rows and the preview aspect, one frame
      per source frame; the Slint app cropped a 1080p render and recorded per
      render.)
- [x] Preview aspect selector, persisted. Evidence: "persist: the aspect is
      saved (4:3)", "persist: the aspect comes back after a restart",
      "persist: a malformed settings file still opens"; `missing_fields_take_defaults`,
      `unknown_values_fall_back`. The recording quality is saved the same way.
- [x] Status text (also the error channel), fps readout, version, Report bug.
      Evidence: "transport: Ready after opening", "transport: the status line
      shows the frame rate while playing", "transport: Paused", "transport:
      playback finishes at the end", "record: the status line says
      recording"; errors also raise a toast ("toasts: a failed open raises an
      error toast"); `status_says_what_playback_does`,
      `recording_and_problems_come_first`. Version and "Report a bug…" sit in
      the app menu (Module 0; the dialog arrives in Module 7).
- [x] Show in folder after a recording. Evidence: "record: Show in folder
      appears"; `finder_selects_the_file`.
- [ ] Export progress, status text and Cancel; Show in folder after an
      export. → Module 6 (the export job). The card exists since Module 0.
- [x] Toasts: info, warn and error with TTL 4/7/10 s (plus custom), at most
      4, dismissable, clear of the Inspector. (New: rendered reliably, and
      they do not overwrite the status line.) Evidence: "toasts: four show",
      "toasts: the oldest of five left first", "toasts: the newest is at the
      bottom", "toasts: inside the viewer, clear of the Adjust panel and the
      time panel", "toasts: the status line keeps its own text", "toasts: a
      close button dismisses its toast", "toasts: an info toast leaves after
      about four seconds", "toasts: a warning stays longer", "record: more
      notices than fit show four" (`toasts.png`); `ttls_follow_the_severity`,
      `at_most_four_newest_last`, `a_repeat_refreshes_instead_of_stacking`.

## Module 3: files (§2.2, §2.7)

Evidence for this module: `tools/check_m3.py` (checks `files`, `list`,
`calibrate` and `recent`; dialogs answered through
`RECO_DESKTOP_DIALOG_ANSWERS`, videos linked into temporary folders, and the
real 5.3K pair calibrated in `calibrate`); the unit tests (`cargo test -p
reco-app -p reco-desktop`, plus `-- --ignored` for the two full
calibrations); and screenshots in `target/desktop-checks/m3/`.

- [x] Left: add videos (multi-select mp4/mov/avi/mkv, appended as
      segments), Clear, segment list with remove and drag-to-reorder,
      "No video selected". Evidence: "files: the left camera shows its file
      and length", "list: the left camera's files show in recording order",
      "list: a row's remove button removes its file", "list: dragging a row
      moves its file", "list: Alt+Up moves the selected file back", "list:
      Remove all empties the camera" (`list-open.png`);
      `gopro_files_go_in_recording_then_chapter_order`,
      `only_videos_are_added_once`, `moves_and_removes_stay_in_bounds`,
      `a_drop_beside_itself_moves_nothing`, `keys_select_remove_and_move`. An
      empty camera shows its Add… button (Module 0's design) instead of "No
      video selected". (New: GoPro chapters go in recording order; the
      keyboard can select, remove and move files.)
- [x] Right: the same. Evidence: "files: the next-step card adds the right
      camera"; the same list widget and model.
- [x] Calibration: Auto Calibrate / Re-calibrate / Calibrating… with step
      text; Load…; calibration file chip with remove. (New: Cancel, and
      Advanced locked while calibrating.) Evidence: "calibrate: the steps
      show", "calibrate: the card says so", "calibrate: the status line
      counts the steps", "calibrate: Cancel stops it", "calibrate: footage
      with no matches fails plainly", "calibrate: Load calibration file opens
      the preview", "calibrate: removing it closes the preview", "calibrate:
      the real pair calibrates" (12 s on the 5.3K pair), "calibrate: the
      calibration is saved beside the left video" (`calibrating.png`,
      `calibration-failed.png`, `calibrated-real.png`);
      `cancelling_stops_the_job`, `footage_with_no_matches_fails_plainly`,
      `calibrating_saves_beside_the_left_file`; Cancel from the card and the
      Setup panel, and the lock: "calibrate: the card's Cancel stops it",
      "calibrate: the Advanced options lock while it runs", "calibrate: and
      unlocks them". (New: a
      calibration saved beside the left video loads by itself: "files: a
      calibration saved beside the left video loads".)
- [x] Calibration Advanced: frames (2/4/6/8), IMU seeds, AKAZE threshold,
      Detect Y min and max, skip end. Evidence: `options_map_onto_the_config`;
      the Frames dropdown starts at the calibration's default (4), found when
      a run on the fast pair calibrated with the dropdown's first item.
- [x] Recent files dialog: left videos, right videos, calibrations; click
      loads; Clear all; Close. Evidence: "recent: the session is
      remembered", "recent: Recent files opens the menu", "recent: picking
      the session opens it again", "recent: Clear recent files forgets them";
      `recent_sessions_are_newest_first_without_repeats`. (New, per Module
      0's design: a Recent menu of sessions, each restoring both cameras and
      the calibration in one click, in place of three separate lists.)
- [x] (New) Drop videos onto the window. Evidence:
      `drops_go_to_the_row_or_the_first_empty_camera` (routing). The drop
      itself is wired to Makepad's `Event::Drag`/`Event::Drop` but not
      machine-checked: the remote cannot drop files.

## Module 4: stitching and calibration (§2.2, §2.4, §2.14)

Evidence for this module: `tools/check_m4.py` (checks `tune`, `sync` and
`roi`, on the fast pair with a temporary copy of its calibration); the
unit tests (`cargo test -p reco-app -p reco-desktop`); and screenshots in
`target/desktop-checks/m4/`.

- [x] Seam blend 0–0.3; Match colours; rig tilt −30..30°; rig roll
      −15..15°; sync offset in frames with Apply. Evidence: "tune: the seam
      blend shows the calibration's value", "tune: the tilt follows the
      slider", "tune: tilting changes the picture", "sync: 30 frames apart,
      the pair plays a second less", "sync: an offset as long as the videos
      is refused", "sync: text that is not a number says so";
      `tuning_reaches_the_renderer_and_marks_it_changed`,
      `changes_stay_inside_their_sliders`,
      `the_sync_offset_moves_the_cameras_within_the_videos`,
      `a_new_sync_offset_brings_new_lanes`. Straight ahead the render pitch
      cancels the tilt (`rig_correction::render_pitch`), so the tilt shows
      once the view turns (`tilt_and_layout_change_the_picture`). (New: the
      sliders show the calibration's own values from the start.)
- [x] Save Calibration shown when calibration or lens is dirty. Evidence:
      "tune: nothing to save yet", "tune: Save appears once something
      changed", "tune: Save goes once saved", "tune: the file has the new
      tilt", "tune: the tilt comes back after a reopen";
      `a_saved_calibration_reloads_with_the_tuned_values`,
      `saving_writes_the_file_and_clears_the_change`. Lens edits arrive
      with Module 5 and take the same path.
- [x] Intersect −1..1, camera axis offset −0.6..0.6, `x_ty` −0.1..0.1,
      Reset. Evidence: "tune: the overlap follows its slider", "tune: Reset
      layout restores the file's overlap"; `reset_restores_the_loaded_layout`.
      Named in plain words (Overlap, Camera depth, Vertical shift), with
      the engine's terms in the tooltips' sense.
- [x] Field ROI status; Set/Edit ROI (browser editor); Paste ROI from the
      clipboard or a JSON field. (New: image preparation off the UI thread.)
      Evidence: "roi: Edit in browser writes the editor", "roi: the editor
      carries both cameras' frames", "roi: a pasted outline is used", "roi:
      bad text says why", "roi: the outline is saved with the calibration",
      "roi: Remove outline clears it"; `an_outline_needs_three_points_inside_the_picture`,
      `the_editor_carries_both_frames_and_the_calibration`,
      `the_editor_job_reports_its_page`. The outline is pasted into the
      field (⌘V) rather than read from the clipboard by a button.
- [ ] ROI points drawn over the lens preview. (New: aligned to the image.)
      → Module 5, with the lens preview.

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

Evidence for this module: `tools/check_m6.py` (checks `export`, `cancel`
and `rules`, on the fast pair linked into a temporary folder, the Save
dialog answered through `RECO_DESKTOP_DIALOG_ANSWERS`, the written file
read back with ffprobe); the unit tests (`cargo test -p reco-app -p
reco-desktop`); and screenshots in `target/desktop-checks/m6/`. AI
tracking is Module 6b: `reco-autocam` is being changed in another branch.

- [x] Output path with Save to… (adds `.mp4` when there is no extension).
      Evidence: "export: the file defaults to beside the left video",
      "export: Save to… sets the file, with .mp4 added";
      `outputs_get_an_mp4_extension`, `outputs_are_checked_before_exporting`.
      (New: a missing folder, a folder, a file that isn't MP4, MOV or MKV,
      or one of the input videos is refused with the reason: "rules: an
      input video is refused".)
- [x] Resolution (1080p, 720p, 2K, 4K) with the size shown; codec (from the
      probed list; New: no fixed indices); quality. Evidence: "export:
      1080p by default", "export: the choices are remembered";
      `this_machine_encodes_h264`, `sizes_and_codecs_have_names`,
      `export_defaults_match_the_slint_app`.
- [x] Processing range: start and end as sliders and text, duration, and an
      empty-range warning. Evidence: "export: the range comes from
      --export-range", "export: the time fields show the range", "export:
      the typed end counts, about three seconds at 30 fps", "rules: both
      times apply together", "rules: the start stops at the end", "rules: an
      empty range says so", "rules: an empty range can't be exported",
      "rules: a time that doesn't read is put back";
      `the_range_stays_inside_the_videos`, `the_start_never_passes_the_end`,
      `both_ends_move_at_once`, `a_range_keeps_its_place_when_the_length_changes`,
      `typed_times_read_as_seconds`, `an_empty_range_fails_at_once`. (New: a
      time typed but not entered still counts when Export is pressed.)
- [x] Record replay; save AI debug events. Evidence:
      `the_replay_and_events_files_go_beside_the_export` (the sheet's two
      boxes pass them to the job).
- [ ] AI tracking: Enable (when available) and status; model picker with a
      missing-model warning; tracking mode; detect every N frames; style
      preset; framing; pitch lock; lookahead with VRAM risk zones; advanced
      panner (cluster mode, ball weight, cluster bandwidth, dead zone, FOV
      tight/default/wide).
- [x] Error text, Cancel, Start Export with its enable rules. Evidence:
      "rules: an input video is refused", "rules: the sheet stays open to
      fix it", "rules: no file, no Export", "rules: editing the file clears
      the reason", "rules: Escape closes the sheet".
- [x] During an export: preview paused overlay, progress, Cancel. Afterwards
      the preview is rebuilt. (New: the playback position is kept.)
      Evidence: "export: the card shows over the picture", "export:
      playback and the ruler are locked while exporting", "export: it
      finishes", "export: a notice says so", "export: the preview stays where
      it was", "export: playback and the ruler come back", "export: Show in
      folder offers the file", "cancel: it stops", "cancel: a notice says
      so", "cancel: playback comes back";
      `exporting_two_seconds_writes_a_playable_file`,
      `cancelling_an_export_stops_it`, `a_snapshot_carries_the_tuning`,
      `progress_reads_in_frames_and_time`. The preview is paused rather
      than rebuilt, so unsaved tuning and the position both stay.

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
