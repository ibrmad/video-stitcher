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
- [x] New (owner, 2026-10-06): the panels slide open and closed in 0.2 s,
      their rows unchanged, the picture giving way smoothly; the menu
      shortcuts ⌘1/⌘2/⌘3/⌘, also work as keys. Evidence: check_m0 `motion`
      ("motion: the picture's edge slides as Setup closes", "… as Setup
      opens", "the picture's right edge slides as Adjust closes", "the time
      panel's top slides as the lanes fold", "Setup slides out, its rows
      unchanged: the title leaves with it", "a toggle mid-slide turns it
      around", "a dragged width comes back"); check_m7 `keys`; the `motion`
      unit tests.
- [x] Setup panel on the left: default width 290, minimum 220, resizable,
      open at startup. Cameras (the linked pair: Add, then Change; file
      count and length) and Calibration (status, detail, Auto-calibrate or
      Recalibrate, Load file) in section bands; recent files in a menu.
      Evidence: "Setup panel open", "`<id>` shown / hidden" per state,
      "camera link is lit", "dragging the bar widens Setup", "Setup stops
      at its 220 pt minimum", "menu: `recent_menu` opens a menu".
- [x] Viewer: a view bar (Preview, Aspect, Record and its menu) over a black canvas; the
      next step with the panorama frame and stepper until a stitch exists;
      calibration progress in that column; the export card with Cancel; a
      calibration-failed state with Try again. Evidence: "viewport is
      #000000", "empty state shown", "stepper not covered while
      calibrating", "`export_card` shown", "`cal_dot_error` shown".
- [x] Adjust panel on the right: default width 310, minimum 220,
      resizable. View and Stitch sections of property rows with tooltips,
      Advanced tiers closed. Evidence: "Adjust panel open / closed",
      "Adjust stops at its 220 pt minimum", "Adjust panel stays closed
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
      Dark only: no light theme is offered (ruling, Module 7). Evidence: the `screens_use_theme_values_only` unit test
      (no raw colours, sizes, spacing or type sizes in `src/ui/`),
      `tools/check_theme.py`, and "Setup panel is #0d0d0d", "section band
      is #212121".
- [x] One content edge per panel, trailing icons on the right edge, one line
      per control row (DESIGN.md Rule 11). Evidence: "`<id>` text starts on
      the edge", "`<id>` starts on the edge", "… ends on the content edge"
      (Recent files, help, Change and band icons, Adjust values), "time
      panel's first icon starts on the edge 12", "`<id>` centred on the
      control row".
- [x] Looks right at 720×600, 1280×820 and 1920×1200, in every state, on
      Retina and on a 1x monitor. The design has more room than Rerun's
      (the owner found 12 px text and 24 pt rows too small on a 1x
      monitor, 2026-10-05): 13 px text, 28 pt rows, a 14 pt content edge,
      26 pt buttons, 18 pt icons; `check_m0.py` measures the new edges.
      Menus (app, Recent, every dropdown's list) share one panel and row
      height, and a dropdown's list opens below it ("dropdown: the click
      on row 1 picks Fast", "menus: the text starts on the content
      edge").
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
      high) hidden while recording. Changed (UI pass, owner-approved
      2026-10-06): Record is one button, which shows the time and a stop
      square while recording at the same width, with a ▾ menu beside it:
      the quality (a ✓ on the one in use) and Codec and folder… (Preferences),
      off while recording, so nothing in the bar moves. Evidence: "record:
      the button says Record", "record: the menu has the qualities and Codec
      and folder…", "record: ✓ on the current quality", "record: the quality
      is remembered", "record: the ✓ moves to it", "record: Codec and
      folder… opens Preferences", "record: the toast names the file, not its
      path", "record: while recording the button shows the time", "record:
      the menu is off while recording", "record: nothing in the view bar
      moves when recording starts", "record: the button's time counts",
      "record: 1920x1080 for Auto",
      "record: about 3 s at 30 fps, one frame per frame played", "record:
      quitting while recording leaves a file", "record: and it plays"
      (`recording.png`); `recording_has_one_frame_per_source_frame`,
      `recording_keeps_its_size_when_the_preview_resizes`,
      `pausing_adds_no_frames`, `quitting_while_recording_finishes_the_file`,
      `the_record_menu_marks_the_quality_in_use`,
      `a_picked_row_names_its_quality`,
      `a_choice_is_picked_like_an_item_and_marks_the_chosen_one`.
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
      the app menu (Module 0; the dialog is Module 7's).
- [x] Show in folder after a recording. Evidence: "record: Show in folder
      appears"; `finder_selects_the_file`.
- [x] Export progress, status text and Cancel; Show in folder after an
      export. Done in Module 6 (see "During an export" there). Evidence:
      "export: the card shows over the picture", "cancel: the export runs",
      "cancel: it stops", "cancel: a notice says so", "export: Show in
      folder offers the file"; `progress_reads_in_frames_and_time`,
      `cancelling_an_export_stops_it`.
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
      −15..15°; sync offset in frames (a value field since the owner's
      2026-10-06 ask: Return applies, no Apply button). Evidence: "tune: the seam
      blend shows the calibration's value", "tune: the tilt follows the
      slider", "tune: tilting changes the picture", "sync: 30 frames apart,
      the pair plays a second less", "sync: an offset as long as the videos
      is refused", "sync: text that isn't a number is put back", "sync: ↑
      steps a frame";
      `tuning_reaches_the_renderer_and_marks_it_changed`,
      `changes_stay_inside_their_sliders`,
      `the_sync_offset_moves_the_cameras_within_the_videos`,
      `a_new_sync_offset_brings_new_lanes`. Straight ahead the render pitch
      cancels the tilt (`rig_correction::render_pitch`), so the tilt shows
      once the view turns (`tilt_and_layout_change_the_picture`). (New: the
      sliders show the calibration's own values from the start.)
- [x] New (owner, 2026-10-06): every slider's value can be typed. A click
      selects the digits; Return or a click away applies the value through
      the slider (kept in its range) and gives the keyboard back; Escape
      puts it back; the unit is optional and a decimal comma reads; ↑/↓
      step the last digit (⇧: ten). The Adjust panel's 15, Setup's
      calibration options, the export sheet's AI rows and Start/End alike.
      Evidence: check_m4 `values` ("values: Field of view takes a typed 25",
      "values: past the end, the slider's end", "values: Escape puts the
      value back", "values: ↑ steps the typed number's last digit", "values:
      ⇧↑ takes ten steps", "values: after Return, Space plays", "values: a
      field clicked and left changes nothing", "values: the typed tilt is
      saved", "values: the digits are on the right of the box"); "lens: a
      typed focal length", "lens: past the range, its end"; "ai: a typed off
      turns the lookahead off", "ai: Escape in a field puts its value back",
      "ai: and leaves the sheet open"; "calibrate: a typed time to skip",
      "calibrate: their value fields too"; the `value_text` unit tests.
- [x] Save Calibration shown when calibration or lens is dirty. Evidence:
      "tune: nothing to save yet", "tune: Save appears once something
      changed", "tune: Save goes once saved", "tune: the file has the new
      tilt", "tune: the tilt comes back after a reopen";
      `a_saved_calibration_reloads_with_the_tuned_values`,
      `saving_writes_the_file_and_clears_the_change`. Lens edits arrive
      with Module 5 and take the same path. Changed (UI pass): Save sits in
      the Adjust panel's title row (`Unsaved [Save]`), not in Setup, beside
      the sliders that make the edits, and ⌘S saves (File → Save
      Calibration). Evidence: "tune: in the Adjust panel's title row,
      beside the tuning", "tune: the title row says Unsaved", "tune: ⌘S
      saves it, and a toast says so"; `command_keys_are_the_menu_shortcuts`.
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
- [x] ROI points drawn over the lens preview. Not done, by ruling (Module
      5; the owner accepted it, 2026-10-06): the outline is drawn on the raw frames (the browser editor), and
      Reco's lens preview never shows a raw frame (both of its modes
      re-project the picture), so the points would sit in the wrong places,
      as they did in the Slint app. The browser editor shows the outline on
      the pictures it was drawn on. A raw single-camera view would make the
      overlay possible.

## Module 5: camera and lens (§2.4, §2.12)

Evidence for this module: `tools/check_m5.py` (checks `view`, `lens`,
`preview`, `picker` and `stats`, on the fast pair with a temporary copy of
its calibration; the lens file through `RECO_DESKTOP_DIALOG_ANSWERS`); the
unit tests (`cargo test -p reco-app -p reco-desktop`, plus `-- --ignored`
for the real-pair calibration); and screenshots in
`target/desktop-checks/m5/`.

- [x] FOV 20–150°, constrained look, Reset View. Evidence: "view: the
      opening field of view is what the picture allows", "view: the slider
      narrows the view", "view: the picture follows", "view: the zoom keys
      move the slider", "view: staying inside caps the field of view",
      "view: switched off, the whole range is free", "view: Reset view goes
      back"; `the_field_of_view_follows_its_slider`,
      `an_unconstrained_look_may_leave_the_picture`,
      `staying_inside_still_zooms_in`, `the_view_reports_its_field_of_view`.
      (New: the slider follows the wheel, the keys and the picture's limit.)
- [x] Lens info for L and R, or "Auto-calibrate to detect lens". Evidence:
      "lens: the left lens is named", "lens: the right lens is named";
      `a_gopro_video_names_its_camera`, `lens_info_reads_like_the_panel`,
      `the_detection_job_reports_both_cameras`,
      `lens_rows_say_where_a_lens_came_from`. (New: looked up from the
      videos for a loaded calibration too; a generic lens says so.)
- [x] Browse profiles…, lens correction, lens preview (single camera) with
      Left/Right. Evidence: "lens: switching correction off changes the
      picture", "lens: the file keeps correction off", "preview: Left camera
      shows one camera", "preview: the camera keeps its shape, black at the
      sides", "preview: Right camera shows the other one", "preview:
      Stitched picture goes back"; `a_camera_shows_flat_and_each_side_differs`,
      `a_camera_sits_inside_the_frame`, `lens_correction_is_saved_with_the_calibration`,
      `a_picture_fits_inside_and_centres`. The preview's Show list (Stitched
      picture, Left camera, Right camera) replaces the checkbox and side
      buttons.
- [x] Fine-tune: Left/Right/Both; fx, fy, cx, cy, k1–k4; Reset Lens.
      Evidence: "lens: fine-tune shows the file's focal length", "lens: the
      focal length follows its slider", "lens: the picture follows", "lens:
      Reset lens can go back", "lens: Save appears", "lens: Reset lens
      restores the file's lens"; `lens_changes_reach_the_renderer_and_reset_restores_them`,
      `a_lens_change_changes_the_picture`, `fine_tune_ranges_are_the_slint_apps`,
      `slider_places_and_values_meet`. Each k range is ±0.3 around its own
      value (the Slint app's ±0.3 was absolute and couldn't show a k1 of
      0.333).
- [x] Lens picker: apply to Both/Left/Right, search the profile database,
      pick a result, Load from file…, Close. (New: search off the UI thread.)
      Evidence: "picker: a search lists matching profiles", "picker: and
      says how many", "picker: picking applies it", "picker: both cameras
      are named by it", "picker: a file loads for the left camera",
      "picker: the right camera keeps its lens"; `the_newest_search_wins`,
      `a_profile_gives_its_lens_at_the_calibration_size`,
      `a_profile_file_gives_its_lens`, `the_hint_says_what_the_list_holds`.
- [x] Stats: fps, frame times, bottleneck, GPU, dropped frames, AI and
      calibration figures. (New: fed during preview too.) Evidence: "stats:
      the GPU is named", "stats: the first second of playing reads near the
      source's 30 fps", "stats: the frame time and the slowest", "stats:
      decode and render times"; `a_second_of_frames_is_reported`,
      `a_second_starts_at_its_first_frame`, `stats_arrive_while_playing`,
      `figures_read_plainly`. Shown: frame rate, frame time, the slowest
      1%, decode wait, render, GPU, and the last calibration's confidence and
      matches, and (Module 6b) a tracked export's detection time, finds a
      frame, tracks and ball presence: "figures: Stats has the detector's
      time and finds", "figures: and the tracks and the ball";
      `a_tracked_export_reports_the_detectors_figures`,
      `figures_come_only_once_the_detector_has_run`. The engine measures
      those only without a lookahead (FRICTION.md); Stats shows them once
      measured. Not shown: a bottleneck line and dropped frames (the
      preview has no drop count yet). Module 8: the last export's speed
      and stages, its slowest stage included, are shown (see there);
      dropped frames are a ruling.

## Module 6: export (§2.11, §7)

Evidence for this module: `tools/check_m6.py` (checks `export`, `cancel`
and `rules`, on the fast pair linked into a temporary folder, the Save
dialog answered through `RECO_DESKTOP_DIALOG_ANSWERS`, the written file
read back with ffprobe); the unit tests (`cargo test -p reco-app -p
reco-desktop`); and screenshots in `target/desktop-checks/m6/`. AI
tracking (Module 6b) adds the checks `ai`, `ai_short`, `ai_unavailable`
and `ai_figures`: the fixture model (`yolo26n.onnx`, or
`RECO_FIXTURE_MODEL`) is chosen through the `model` answer, and
`RECO_DESKTOP_FAKE_AI` stands in for a machine that can't run the
detector. reco-app's tracked exports run in optimized builds only
(`cargo test --profile desktop -p reco-app`; FRICTION.md).

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
      `a_range_keeps_its_place_when_the_length_changes`,
      `typed_times_read_as_seconds`, `an_empty_range_fails_at_once`. (New: a
      time typed but not entered still counts when Export is pressed.)
- [x] Record replay; save AI debug events. Evidence:
      `the_replay_and_events_files_go_beside_the_export` (the sheet's two
      boxes pass them to the job).
- [x] AI tracking: Enable (when available) and status; model picker with a
      missing-model warning; tracking mode; detect every N frames; style
      preset; framing; pitch lock; lookahead with VRAM risk zones; advanced
      panner (cluster mode, ball weight, cluster bandwidth, dead zone, FOV
      tight/default/wide). Evidence: "ai: the status says where tracking
      runs", "ai: Enable takes input once the machine answers", "ai: off,
      the tracking rows are hidden", "ai: on, the rows show", "ai: no model
      says so", "ai: no model, no Export", "ai: Choose… sets the model",
      "ai: with the model, Export is enabled", "ai: Broadcast, every 15
      frames to start", "ai: Broadcast's knobs", "ai: Action narrows the
      dead zone", "ai: Frame all frames everyone", "ai: Broadcast puts its
      knobs back", "ai: the lookahead fits this machine", "ai: the track's
      safe zone is green", "ai: dragging the lookahead shows its value",
      "ai: the export says tracking is active", "ai: the events file has
      the detections", "unavailable: the status says why", "unavailable:
      Enable is dimmed", "unavailable: a click leaves it off",
      "unavailable: the rows stay hidden"; `an_export_tracks_with_the_model`,
      `tracking_without_its_model_fails_before_writing`,
      `the_config_is_the_preset_with_the_knobs_over_it`,
      `presets_set_the_knobs_from_the_engine`,
      `broadcast_is_the_slint_apps_starting_point`,
      `a_model_is_needed_except_for_sweep`,
      `zones_come_from_the_gpu_and_the_source`,
      `a_lookahead_that_does_not_fit_starts_at_the_safe_value`,
      `an_open_reports_the_gpus_memory`, `the_probe_answers`,
      `the_status_says_where_tracking_runs_or_why_not`,
      `the_note_says_whether_the_lookahead_fits`, `a_lookahead_sits_in_one_zone`,
      `the_zones_end_where_the_track_says`. Labels in plain words ("Follow":
      Players and ball, Ball only, Sweep (no AI); "Tilt": hold it level).
      (New: Sweep needs no model, "ai: Sweep needs no model"; the model
      is an .onnx file that exists, and the last one used (chosen, or
      typed in and exported) stays for the next export; every choice but the Advanced tier is
      remembered, "ai: the choices are remembered, the model as the
      default", `tracking_choices_default_as_the_slint_app`; the lookahead
      fills in its zone's colour, with a line saying whether it fits; the
      rows scroll in a short window, "short: Export stays inside the
      window", "short: the sheet scrolls to its last row"; the card and
      the notice say whether tracking started, "ai: so does the card", "ai:
      the notice says it tracked", `the_card_says_whether_tracking_started`,
      `the_notice_says_whether_the_export_tracked`.)
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

Evidence for this module: `tools/check_m7.py` (checks `prefs`, `blend`,
`shortcuts`, `bug`, `usage`, `update` and `persist`, on fresh settings
folders, without network, browser or clipboard: each request, link and copy
is a log line, and the payloads are kept for the check to read), the unit
tests (`cargo test -p reco-app -p reco-desktop`), and screenshots in
`target/desktop-checks/m7/`.

- [x] Preferences (the app menu, and ⌘, in the macOS menu bar). Changed
      (UI pass, owner-approved 2026-10-06): app-wide settings only, the
      recording codec and folder with Choose…, and the usage-data opt-in.
      The Slint app's other defaults live where they are used: export codec
      and quality (the export sheet remembers its own), recording quality
      (Record's menu), the AI model (the export sheet). The seam blend
      default is dropped: a first calibration starts at 0.05, a
      recalibration keeps its blend, and an old saved default is ignored.
      Save keeps and applies them. New: Cancel, Escape or a press outside
      keeps nothing; a missing folder is refused with the reason. Evidence:
      "prefs: the app menu opens Preferences", "prefs: only app-wide
      settings, the rest live where they are used", "the recording codec
      shows H.264", "Cancel keeps nothing", "Escape keeps nothing", "a
      missing folder is refused (That recording folder doesn't exist.)",
      "Choose… sets the folder", "the recording codec is kept", "the folder
      is kept", "a new launch shows the codec and the folder"; "blend:
      Auto-calibrate starts at 0.05, whatever an old saved default said";
      `the_recording_folder_must_exist`,
      `a_file_from_before_loads_and_drops_the_blend_default`,
      `a_new_calibration_takes_the_default_blend` (a recalibration keeps
      the seam its calibration had), `recording_uses_the_chosen_codec`.
      Ruling: dark mode is not offered; the app has one look, the Rerun
      dark look the owner chose (a light theme is a design project of its
      own). The ⌘, menu item has no check (menu-bar commands don't reach
      the remote).
- [x] Keyboard shortcuts, listing exactly the keys the app answers (the
      sheet and the key handler share one table), with Website and Forum.
      Evidence: "shortcuts: the keys and what they do", "every key, the
      pointer and the menu keys (12 rows, ⌘S among them)", "Website opens the project's
      page", "Forum opens the forum", "Close closes it", "Escape closes
      it"; `the_sheet_lists_every_key_the_preview_handles` (every key
      Makepad knows: handled ⇔ listed),
      `the_sheet_lists_the_menu_keys_only_with_the_menu_bar`.
- [x] Report a bug: what went wrong, a contact, "Include system info and
      logs" (the version with its commit, OS, GPU, the open files' names
      only, the preview's figures, the last calibration run and the log's
      newest 200 lines, the home folder shown as `~`). New: Send needs the
      usage-data opt-in (a hint offers Preferences) and says whether it
      went; Copy report puts it on the clipboard only when clicked.
      Evidence: "bug: Send waits", "with usage data off, a hint offers
      Preferences", "Copy report copies it", "the report starts with the
      words and the contact", "with the version and the log", "the home
      folder reads as ~", "without details, only the words and the
      contact", "Send sends it", "a notice says it was sent", "with the
      files' names only", "with the GPU"; the `bug_report` and
      `telemetry` tests (the report fitted under 16 KB, oldest log lines
      first).
- [x] Usage data (opt-in): the Slint app's events and JSON: app_open, the
      system's context, source info per opened match, export and
      calibration outcomes, bug reports; none with it off. Evidence:
      "usage: an opened match sends its source info", "and its outcome
      ({'frames': 30, 'duration_sec': 1.0, 'codec': 'h264'})", "a failed
      calibration sends its error", "with it off, nothing is sent";
      `a_batch_is_the_slint_apps_json`, `each_event_has_its_name_and_figures`.
      The real service was not posted to from a check or a probe (that
      sends data out; the owner's call).
- [x] Update check: GitHub's latest release at start, and a notice with
      Download for a newer one. New: the browser opens only on the click.
      Evidence: "update: a newer release shows a notice", "with Download",
      "the browser waits for a click", "Download opens the release page",
      "'v0.5.4' → 'update check: up to date'", "'v1 & calc' → 'update
      check: no release in the answer'", "offline, GitHub isn't asked";
      a one-off real request: "update check: up to date (0.5.4; the latest
      is v0.5.4)"; `versions_compare_by_number`, `the_tag_comes_from_the_release`.
      The app's version is the product's, 0.5.4 (it was 0.1.0, which every
      release would have beaten), shown in the app menu with its commit.
- [x] Settings persistence: recent sessions (Module 3), export and
      recording defaults, the preview aspect (Module 2), usage data and its
      id. New: the window's size and full screen and the side panels'
      widths, saved a quiet second after a change (or on quit) and
      restored at start, fitted to the attached displays (`--window-size`
      still wins). Evidence: "persist: dragging the edges widens both
      panels", "saved a quiet second later", "the window is remembered",
      "at its size ((1440, 900))", "with the panels' widths", "--window-size
      wins"; `the_window_is_remembered_windowed_and_full_screen`,
      `a_restored_window_is_never_below_the_minimum`. Full screen at start
      was seen once in a probe (the window came back 3440×1440); no check
      runs it, as even a hidden window takes over the display.

## Module 8: parity sweep

Evidence for this module: `tools/check_m8.py` (`start`, `perf`, `sheets`,
`unsaved`, `logfile`), new parts in check_m1, m4, m5, m6 and m7, the unit
tests, a Linux build and run (OrbStack: Ubuntu 24.04 with CI's packages,
Xvfb), and screenshots in `target/desktop-checks/m8/`.

The sweep walked the whole inventory (about 450 entries: §1–§9, the 61
callbacks, the 38 fragile points) and DESIGN.md's issues list against this
checklist and the app; each finding was checked by hand. What it found
and what became of it:

- [x] No shortcut fires behind a sheet, and Escape closes every sheet
      (DESIGN.md Rule 9: ⌘1–⌘3, ⌘S and ⌘, ran behind any sheet, ⌘,
      stacked Preferences on the export sheet, and the preview took Space
      behind Report a bug). Evidence: check_m8 `sheets` for Export,
      Keyboard shortcuts, Report a bug, the lens picker and Preferences
      ("⌘1 does nothing behind the … sheet", "Space doesn't play behind
      the … sheet", "⌘, doesn't open Preferences over the … sheet",
      "Escape closes the … sheet").
- [x] The export sheet keeps choices made and not exported (Rule 9; it
      refilled them from the settings at each opening, as Slint did).
      Evidence: check_m6 `kept` ("the choices are still there").
- [x] New (owner, 2026-10-06): closing or ⌘Q with unsaved calibration
      edits asks first: Save saves and quits, Don't Save quits, Cancel
      stays (Slint saved a pasted outline at once and lost other edits).
      A termination signal still quits. Evidence: check_m8 `unsaved`
      ("quitting asks first", "Cancel keeps the app and the edit", "Save
      wrote the edit", "Don't Save quits", "and leaves the file as it
      was", "with nothing unsaved, quitting quits at once").
- [x] Recalibrate keeps the lenses in use, a picked profile or fine-tuning
      not yet saved (Slint kept its in-memory calibration; this app read
      the file's). Evidence: `a_recalibration_keeps_the_lenses_in_use`.
- [x] A log file, as Slint kept: the engine's lines and the app's,
      timestamped, kept across runs and started afresh over 2 MB,
      `RUST_LOG` filtering them (default `info,ort=warn`), panics written
      in; in Slint's places (`~/Library/Logs/reco-desktop.log`, the XDG
      state folder, beside the .exe); the bug report attaches its tail.
      Evidence: check_m8 `logfile` ("the engine's lines are in it", "the
      app's own lines are in it", "a second run adds to it", "RUST_LOG=warn
      leaves the engine's info lines out"); `the_filter_reads_like_rust_log`,
      `the_file_lives_where_each_system_keeps_logs`,
      `the_file_keeps_lines_across_runs_and_starts_afresh_when_large`.
- [x] Usage data and the bug report say where AI tracking runs and name
      the GPU's backend (the context sent Slint's no-AI line). Evidence:
      check_m7 "usage: the context says where AI tracking runs", "and the
      GPU with its backend", "bug: and its backend", "bug: and where AI
      tracking runs"; `the_usage_line_says_where_tracking_runs`.
- [x] Reset layout only when the layout changed (Slint's `cal-dirty`).
      Evidence: check_m4 "tune: Reset layout waits while the layout is
      the file's", "a changed overlap enables Reset layout", "and waits
      again"; `the_layout_differs_by_its_three_values`.
- [x] Showing one camera opens Fine-tune, as Slint. Evidence: check_m5
      `fine_tune_opens`.
- [x] An export says where it starts while it seeks ("Starting at
      0:30…"; Slint said "Seeking to …"; the engine reports no phases).
      Evidence: check_m6 `starting`; `the_card_says_where_the_export_starts`.
- [x] F/F11 on Windows and Linux maximizes and restores (Makepad has no
      full screen there: FRICTION.md); a Windows release build opens no
      console window. Evidence: `f_toggles_full_screen_or_the_maximized_window`;
      check_m1 "F toggles fullscreen" (macOS).
- [x] The detector backends as reco-gui offers them: ort, load-dynamic
      (the release builds), cuda, tensorrt, directml (on for Windows),
      tensorrt-native, ncnn, profiling. Evidence: built here: the default,
      load-dynamic, coreml, profiling, ncnn without ORT and no default
      features (tensorrt-native needs the TensorRT SDK).
- [x] Owner's choice: Stats shows the last export's speed and stages, as
      Slint's Stats did (fps lately and overall, frame time and slowest 1%,
      decode and stitch, read-back and encode, the slowest stage).
      Evidence: check_m6 `export_figures`; `an_export_reports_its_figures`,
      `export_figures_read_plainly`.
- [x] Owner's choice: the preview's character keys follow the keyboard's
      layout: R, F, + = - _ [ ] are matched by what they type, as Slint
      matched the typed text (on a German keyboard + is the US ] key);
      Space, the arrows and F11 by place. macOS asks the layout
      (`UCKeyTranslate`); Windows and Linux use their text events.
      Evidence: check_m1's key checks; `keys_that_type_follow_the_layout`,
      `the_sheet_lists_every_key_the_preview_handles`, and
      `the_layouts_are_read_on_the_main_thread` (the US and German layouts
      by name).
- [x] Owner's choice: the benchmark auto-export, in automation builds
      (`RECO_AUTOLOAD`, `RECO_AUTOEXPORT` with `_MODEL`, `_LOOKAHEAD` and
      `_REPEAT`, `RECO_VRAM_BUDGET_GB`). Evidence:
      `the_export_reads_as_slint_read_it`, `autoload_becomes_the_command_line`,
      `runs_number_their_files`, `the_budget_is_in_gigabytes`,
      `a_budget_gives_the_zones_its_memory_would`; a run exported twice and
      quit, and once with AI tracking.
- [x] Found on the way: the top bar was missing on Linux (the caption bar
      came out 0 tall); macOS unchanged. Evidence: the Linux run's
      screenshot; check_m0.
- [x] Found on the way: a view change sent both frames to the GPU again
      (5.3K pans redrew about 52 times a second, 38 ms at the slowest);
      reco-core's additive `render_uploaded_to_view` (owner's OK) draws
      from the frames already there. Evidence: check_m8 `perf`;
      `render_uploaded_to_view_draws_the_last_upload`,
      `a_pan_draws_what_sending_the_frame_again_would`,
      `a_new_frame_is_sent_before_it_is_drawn`.
- [x] The decoder's reopen and seek run off the UI thread (a Slint issue):
      the render worker owns the session. Evidence: the worker's tests
      (`seek_burst_lands_on_the_sum`); check_m1 "] seeks 5 s".

Rulings for the sign-off (kept as they are, and why):

- Screen readers: Makepad 2 has no accessibility bridge on macOS
  (DESIGN.md, Decisions); Slint exposed its std widgets to VoiceOver.
  Every control is reached by the keyboard (Rule 9).
- The field outline over the lens preview: not drawn (above); the owner
  accepted it, 2026-10-06.
- Slint's settings (`gui.json`) aren't read at first start: the owner
  declined it, 2026-10-06.
- Match colours is on at every open, not remembered, as in Slint.
- Usage data's `decoder` says how frames reach the picture (zero-copy or
  readback), not Slint's fixed "D3D11VA/NVDEC/VT".
- The paused preview stays loaded during an export (Slint released it), so
  the position and unsaved tuning stay; the lookahead's zones are measured
  with it loaded, so they agree.
- Dropped frames aren't counted (the preview has no drop count); an
  export's slowest stage is shown.
- Windows is built only in CI, with the switch: DirectML and the
  console-less build are unrun here.

- [x] Performance pass against DESIGN.md Rule 8 (check_m8 `start` and
      `perf`; an M1 Pro, the alfheim pair and the 5.3K match pair):
      - the first window frame about 300 ms after launch (a fresh build's
        first launch 0.45–1.7 s: macOS's first look at a new binary);
      - the zero-copy preview, no CPU copies, on both pairs;
      - playing: the source rate (30.4 and 30.8 fps), 22% and 76% CPU;
      - panning: about 84 and 82 redraws a second, 3.2 and 3.4 ms each, 5.6
        and 6.1 ms at the slowest, inside a 60 Hz frame;
      - UI draws 0.2–0.4 ms a frame on average;
      - paused: about 0% CPU.
- [x] Linux: built, linted and tested in Ubuntu 24.04 (CI's packages plus
      X11, GLX, xkbcommon, PulseAudio, ALSA, gbm and drm), and run under
      Xvfb, the preview by readback on Mesa's llvmpipe.
- [x] Packaging (owner's choice: cargo-makepad on macOS, a script
      elsewhere): `tools/package.py` makes a signed `Reco.app` and the
      Windows and Linux folders, each carrying its fonts and icons.
      Evidence: the Linux package, alone in a clean container (no source
      tree, no Makepad checkout), draws its text and icons and plays the
      videos, with no resource warning (`linux-package.png`); `Reco.app`
      runs from its bundle, Inter inside it;
      `every_font_the_app_names_is_packaged`; `tools/test_package.py`
      (the manifest read from a binary, the crates linked, the layout).
- [ ] Owner sign-off; Slint app removed, with the CI and release workflows
      moved over (packaging with `tools/package.py`) and Windows proven in
      CI (the other session's `main.slint` edits go with it, owner,
      2026-10-06).
