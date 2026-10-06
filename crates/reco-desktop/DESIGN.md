# Reco Desktop: design, rules and plan

Status: approved 2026-10-04 (framework, rules, module order).
Branch: `feat/makepad-desktop-ui`.

## Goal

A new desktop app for Reco that looks modern, is easy to use and is fast,
built with Makepad 2. It replaced the Slint app (`crates/reco-gui`): the
owner signed off the parity sweep on 2026-10-06 and the Slint app was
removed in one commit. Code comments that say "as the Slint app" point at
it: `git log --diff-filter=D -- crates/reco-gui/ui/main.slint` finds the commit, and its parent has the app.

It is finished when:

- every item in the parity checklist ([PARITY.md](PARITY.md)) works in the
  new app;
- the preview runs at the display rate with no CPU copies on macOS;
- every screen has been checked in the running app, with screenshots;
- the owner has signed off the look and the parity sweep.

## Decisions

| Decision | Choice | Why |
|---|---|---|
| UI framework | Makepad 2, git dependency pinned to `dev` @ `62691a290eb58f7d5234960524429aaada3e572c` (2026-10-03) | Probe (2026-10-04): wgpu 28 and Makepad share one `MTLDevice` on macOS, so a wgpu-rendered 1080p texture adopted with `Texture::adopt_metal_bgra` ran at 120 fps and ~12% CPU with no engine changes. egui 0.36 and iced `main` need Reco on wgpu 30, which touches 34 files and the Metal interop in three. |
| Platform order | macOS first | The owner's platform. Other desktops must compile and run, using a portable preview path (GPU readback into a Makepad texture) until each gets a zero-copy path. |
| Engine | unchanged | `reco-core`, `reco-io`, `reco-calibrate`, `reco-autocam`, `reco-control` and `reco-detect` keep their APIs. A gap gets a `FRICTION.md` entry and is raised, not hacked around (Reco rule). |
| Slint app | removed (2026-10-06) | Stayed buildable as the behavioural reference until the owner's parity sign-off, then was removed in one commit. |
| Look | the Rerun viewer (`re_ui`) as the design reference (owner, 2026-10-05) | A dense, flat, dark tool UI built around a timeline and side panels, close to Reco's job. Reco keeps pitch green in Rerun's accent role and takes Rerun's density whole: hints live in tooltips. See Look below. |
| Accessibility | known gap | Makepad 2 has no screen-reader bridge on macOS (`CxOsOp::AccessibilityUpdate` is ignored). Full keyboard access is still required (Rule 9). |

## Architecture

Two new crates:

| Crate | Kind | Owns |
|---|---|---|
| `reco-app` | library, no UI framework | `AppState`, the `Action` enum, the session (pipeline, playback, pose), background jobs (calibrate, export, record, update check, ROI image prep, lens search), settings, toasts, bug report, telemetry. Ported from `reco-gui/src` with the Slint types removed. Unit-tested. Created in Module 1. |
| `reco-desktop` | binary, Makepad | Theme, window shell, screens, custom widgets, the preview bridge. It reads state and sends actions; it owns no other logic. |

Data flow:

1. A UI event becomes an `Action`.
2. `reco_app` applies it: the state changes and, if needed, a job starts on a
   worker.
3. Job results come back over a channel. The UI thread drains it with
   `try_recv` on each frame (or on a `SignalToUI`).
4. The UI refreshes the widgets whose state changed.

Preview bridge (Module 1). The render thread lives in `reco-app`
(`src/preview/worker.rs`); the `RecoPreview` widget
(`reco-desktop/src/ui/preview.rs`) shows its frames.

- The worker makes its own wgpu 28 device. Zero-copy needs it to be
  Makepad's `MTLDevice` (`cx.metal_device()`, compared by pointer);
  otherwise, or with `--preview-readback`, the worker reads frames back.
- Decoding is `FfmpegFileSource`: CPU YUV planes, seekable. The zero-copy
  decoder cannot seek yet ([FRICTION.md](FRICTION.md)).
- Zero-copy: a ring of six `Bgra8Unorm` textures. The worker sends their raw
  `MTLTexture` pointers (`PreviewEvent::Ring`), keeps them alive until the
  widget answers `Adopted`, and holds back any new ring (a resize) until
  then.
- Makepad and the worker use different Metal queues with no fence between
  them. So the worker waits for its own GPU work before it announces a
  frame; the widget hands a slot back only three display beats and at least
  50 ms after it stopped showing it (`Retirement`: beats keep coming while
  Makepad skips paints with three frames in flight); and the worker renders
  only into slots it got back (`SlotRing`).
- Readback: the frame is copied to a buffer, mapped, and sent as BGRA `u32`
  pixels into a `VecBGRAu8_32` texture. Slower, but it works on every
  desktop; the Module 1 check runs both paths.
- The worker sleeps until the next frame while playing and blocks while
  paused and still, so a paused preview costs about 0% CPU. The pose eases
  by time (reco-gui's 0.25 per 60 Hz frame) whatever the render rate, and
  eased renders come at most every 8 ms.
- A pair that gives no first frame, or videos of different sizes, fail the
  open (`PreviewEvent::Failed`; [FRICTION.md](FRICTION.md)). Anything that
  fails once open (a render, a seek, a decode) pauses playback and is
  reported once (`PreviewEvent::Stopped`); the picture and the panels stay.
  Space after the end plays again from the start.

Time panel and status (Module 2):

- Seeks go by frame index (`SeekTo`). Seeks in one batch of commands
  collapse into one, and `SeekBy` counts from a pending target, so holding
  `]` costs one seek per batch. A seek the videos cannot make keeps the
  frame on screen and says so.
- The ruler seeks on release and holds its target until the reported
  playhead lands within 0.5 s of it (or 1.5 s pass), so the playhead never
  jumps back while the worker seeks.
- Lanes and the length come from the files: a probe thread opens each file
  once for its duration, lays them end to end per camera, shifts them by the
  sync offset, and the pair plays only while both cameras have video.
- Recording: its own renderer at 1080 rows and the preview aspect (the
  preview's renderer keeps its first render target), NV12 readback, and an
  encoder thread behind a queue of eight frames that the render thread waits
  on rather than dropping a frame. One frame per source frame shown, nothing
  while paused, the last frames flushed on stop. Quitting while recording
  is the one time the UI thread waits: up to 3 s for the file to close.
- Toasts: four card slots in a layer over the viewer's canvas, fed by
  `reco_app::toasts` (a pure model with expiry times) and one timer.
  They never write the status line.
- Settings live in `desktop.json` in reco-io's settings folder (namespace
  `desktop`; `RECO_CONFIG_DIR` overrides it, and every check launch gets
  its own). The preview aspect and the recording quality are stored by
  name; anything missing or unknown reads as its default.
- A worker panic is reported as `Stopped("The preview stopped
  unexpectedly: …")` instead of a silent dead preview.

Files and calibration (Module 3):

- `reco_app::project` holds each camera's files and the calibration; the
  stage (no videos, one camera, both, ready) decides the next-step card,
  and `sync_live` opens, reopens or closes the preview from it. The render
  thread stays for the app's life: `Close` drops the videos but keeps the
  texture ring, and a reopen retires the shown ring until the UI adopts
  the next one.
- File lengths come from `reco_app::durations`, measured on short-lived
  threads and remembered.
- `reco_app::calibrate` runs a calibration as a cancellable job, reports
  its seven steps, and saves `{first left stem}_calibration.json` beside
  the first left file (atomically). That file loads by itself the next time
  the pair is set up.
- System file dialogs answer as actions. On macOS the open panel runs
  `runModal`, queued onto the main queue: the one time the UI thread waits
  on the user (the render thread keeps going). Checks answer dialogs from
  `RECO_DESKTOP_DIALOG_ANSWERS` through the same path.
- The Recent menu keeps eight sessions (both cameras and the calibration)
  in `desktop.json`.

Stitching and calibration (Module 4):

- The render thread owns the live calibration. `Tune` sets the blend,
  colour match, tilt, roll or layout; `SetSyncOffset` reopens playback at
  the same frame and measures the lanes again; `SetFieldRoi` sets the
  outline; `SaveCalibration` writes it (atomically) with the live values
  folded in. After each, `Calibration(values)` reports them with `dirty`,
  and the Adjust panel and its title row's Save follow it.
- Straight ahead, the render pitch cancels the rig tilt
  (`rig_correction::render_pitch`); the tilt levels the horizon as the
  view turns.
- The field outline's editor (the Slint app's page) is written on a job
  thread into `$XDG_CACHE_HOME/reco/roi` (a sandboxed browser can't read
  `/tmp`) and opened in the browser; checks set `RECO_DESKTOP_NO_BROWSER`.

Export (Module 6):

- `reco_app::export` runs Reco's `StitchJob` on its own thread with a
  cancel flag. It reports progress (at most ten times a second, the rate
  counted from the first frame), finishing, and done, failed or
  cancelled. The output is checked first: a missing folder, a folder
  itself, a file that isn't MP4, MOV or MKV (a calibration would be
  overwritten), or one of the input videos (an export over its own source
  would destroy it) is refused with the reason.
- The export uses the live tuning: the worker answers `Snapshot` with the
  calibration as tuned and the colour match, and the job starts from it.
- The preview pauses for an export and stays where it was. Closing it
  would drop the Adjust panel's unsaved changes, and a paused preview
  neither decodes nor renders. Playback (buttons, keys and the ruler) is
  locked until the export ends; the ruler keeps showing the playhead.
- The encoders are probed once at startup on a thread; the codec list
  offers what this machine can encode (H.264 until the probe answers).
- The sheet is Makepad's `Modal`: it dims the window, keeps the keyboard,
  and closes on Escape or a press outside. Its range sliders run over the
  match in whole seconds (Makepad's slider has no runtime range); the
  time fields take "90", "1:30" or "1:02:03", and count when Export is
  pressed even if not entered (buttons take no focus). The range belongs
  to the match: a new sync offset keeps it in place. Size, codec, quality,
  replay and events are remembered in `desktop.json` for the next export.
- Cancel keeps the part written (StitchJob closes the file properly) and
  the notice names it.

AI tracking (Module 6b):

- reco-app takes `reco-autocam` behind its `ai` feature (on by default;
  `coreml` passes through), as the Slint app's default build: ONNX Runtime
  on the CPU. `reco_app::ai` holds the sheet's choices and builds the
  engine's config: the style preset is the base and the knobs shown go
  over it, as the Slint app built it. A ball-only mode raises the
  confidence floor.
- Whether the machine can run the detector is asked once at startup on a
  thread (ONNX Runtime's engines can take a moment to load). Until it
  answers, and when it can't, "Follow the play" is dimmed and off: the
  sheet puts back a click (FRICTION.md), and the status line says why.
- The sheet's rows scroll (`ScrollYView`, capped at the window's height
  less the band, the buttons and a margin: `reco_sheet_rows_max`). The AI
  rows show while tracking is on; the panner's finer knobs are a closed
  Advanced tier (Rule 9).
- The model is the last one used: Choose… keeps it at once, and a usable
  model typed in is kept at Export (only the sheet shows it since the UI
  pass). Tracking
  without a usable model (an .onnx file that exists, Preferences' rule)
  says why under the model, and Export waits; Sweep needs none.
- A style preset sets the knobs it covers. On open, the Advanced tier
  follows the saved preset; the mode, interval, preset, framing, tilt
  lock and lookahead are remembered.
- The lookahead's zones come from the GPU's free and total memory at the
  preview's open, and the source's size and rate (the engine's budget, as
  the Slint app). A saved lookahead past the ceiling opens at the safe
  value. The track (`RecoZones` under a knob-only slider) shows the zones
  dimmed and fills up to the knob in its zone's colour; without a reading
  it is a plain slider.
- The job adds the lookahead, then sets up the detector, trackers and
  panner on the session as it opens (`on_session` → `setup_autocam`, with
  the calibration's field outline). It says whether tracking started (the
  card's line; the notice adds "tracked with AI") and sends the engine's
  AI figures to Stats once measured.

One home per setting (UI pass, owner-approved 2026-10-06):

- Preferences holds the app-wide settings only: the recording codec and
  folder, and usage data. The rest live where they are used: the export
  sheet remembers its codec, quality and AI model; Record's menu has the
  recording quality; Adjust has the seam blend. There is no separate
  seam blend default: a recalibration keeps its blend, a first one starts
  at 0.05, and an old saved `default_blend` is ignored.
- The view bar reads `Aspect [Auto] … [Record][▾]`. Record keeps one
  width whether it says "Record" or shows the time with a stop square
  (`reco_record_width`), and its menu is off rather than hidden while
  recording, so nothing in the bar moves. The menu is a `RecoMenu` with
  `MenuEntry::Choice` rows: the ✓ sits at the content edge, so no row
  keeps an empty mark column. Its last row, Codec and folder…, opens
  Preferences.
- Save lives in the Adjust panel's title row (`Unsaved [Save]`), for
  edits from Stitch, Lens or the field outline. ⌘S saves through the
  macOS menu bar (File → Save Calibration) and through the window's key
  (`keys::is_save_shortcut`: injected keys and systems without a menu
  bar, where it is Ctrl+S). The row goes as soon as a save is sent, and
  a failed save brings it back with the next values.

Camera and lens (Module 5):

- The field of view and "stay inside" are view commands (`SetFov`,
  `StayInside`): they don't change the calibration, so they don't mark it
  unsaved. The worker answers each view change, and each open, with the
  field of view it is heading to (`Fov`), and the slider follows the
  wheel, the keys and the picture's limit (staying inside narrows it).
- Lens changes are `Tune` changes on a small `Lens` value (fx, fy, cx, cy,
  k1–k4) for the left camera, the right or both; Reset lens returns to the
  file's lenses. Lens correction is saved with the calibration.
- The fine-tune sliders run 0 to 1 over the Slint app's ranges (±15% of the
  focal length, ±10% of the picture for the centre), centred on the lens
  loaded or picked; each k term is ±0.3 around its own value.
- Lens names come from the videos' telemetry (a short thread when the
  preview opens) or from the calibration run that made the file; a picked
  profile or a file names its cameras "(picked)" or "(file)".
- The single-camera view uses Reco's `LensPreviewRenderer`, which draws at
  the input's size; the worker fits that picture into the preview's frame
  with a small blit (letterboxed) on both the zero-copy and readback paths.
- The lens picker searches on a short thread; only the newest query's
  results are kept. A profile is scaled to the calibration's lens size.
- Stats: the worker times each shown frame's wait for decoded frames and
  its render, and reports once a second from the first frame (reset on
  Play), so the first figure after Play is not diluted by the pause.
- Folds: `RecoFold` wraps Makepad's FoldHeader so a closed body takes no
  pointer events outside the fold (FRICTION).

Parity sweep (Module 8):

- The preview renders from the frames already on the GPU when only the view
  changed (`Playback::frame_serial` against the frame last sent; reco-core's
  additive `render_uploaded_to_view`, owner's OK): a 5.3K pan costs about
  3 ms instead of 20.
- Sheets hold every shortcut: `run_shortcut` returns while one is open, and
  the App hands the preview `SheetOpen` in the scope's props with each key.
- Quitting (⌘Q, the menu, the window's close) with unsaved calibration
  edits asks first (`QuitRequested` handled, `accept_close` false, then
  `cx.quit()` once answered or saved); a termination signal still quits.
- The log file is reco-app's `log_file`: the `log` crate's lines straight
  in, the app's (Makepad's log ring) copied on each signal and at
  shutdown, panics written by the hook. Checks name their own
  (`RECO_DESKTOP_LOG_FILE`).
- Keys: by place for Space, the arrows and F11; by the typed character for
  R, F, + = - _ [ ] (macOS asks the layout, `UCKeyTranslate`; Windows and
  Linux take their text events), so they follow the keyboard's layout.
- The detector features are the Slint app's (reco-app's `ort`, `load-dynamic`,
  `cuda`, `tensorrt`, `directml` on for Windows, `tensorrt-native`, `ncnn`),
  and `automation` builds the benchmark hooks (automation.rs).
- Linux: built and run in a container (Ubuntu 24.04, Xvfb, Mesa); it needs
  X11, GLX, xkbcommon, PulseAudio, ALSA, gbm and drm.
- Packaging (`tools/package.py`): a release carries Makepad's fonts and
  icons. macOS: `cargo makepad desktop bundle` (cargo-makepad from the
  pinned revision; `[package.metadata.makepad.desktop]` names it "Reco",
  `org.reco-project.reco`) makes a signed `Reco.app`, extra files (ONNX
  Runtime) go into `Contents/MacOS` and it is signed again. Windows and
  Linux: a `MAKEPAD_PACKAGE_DIR=resources` build in its own target folder,
  beside it `resources/<crate>/resources`, `resources/<crate>/fonts` (the
  fonts the binary's manifest names) and `<exe>.makepad-package-paths`.
  Fonts the theme uses are declared in `app_main!` (`font_assets`).

Threading, adopted from Makepad's own rules:

- The UI thread never blocks. It shares no `Mutex` or `RwLock` with workers,
  never waits on a channel, and never opens or seeks a decoder or does file I/O
  beyond small settings writes.
- Workers are long-lived and fed by bounded channels. Large payloads travel
  as `Arc`.

## Look

After the Rerun viewer, with more room: Rerun's 12 px text and 24 pt rows
read too small on a 1x monitor, so the owner chose a roomier design
everywhere (2026-10-05). Every value is a token in `src/theme.rs`.

- **Surfaces**, Rerun's neutral grey scale: viewport `#000000`, panels and
  the title bar `#0d0d0d`, view bars `#171717`, section bands and floating
  panels (menus, the export card) `#212121`, control faces `#2c2b2b`,
  separators `#272626`.
- **Text**: Inter Medium at 13 px for everything (Makepad's bundled
  variable Inter at weight 500; macOS's own interface size), 12 px for
  small print, one 22 px SemiBold title for the viewer's next step. Colour does the hierarchy: white for
  names and titles, `#cfcfcf` for values, `#939090` for labels and meta.
- **Accent**: pitch green where Rerun uses blue, and nowhere else: the
  primary action (white on `#007541`), focus rings, slider and progress
  fills, the lit seam, camera lanes (`#34d399` marks, `#0f4a30` blocks).
- **Geometry**: 28 pt rows and bands, 32 pt title rows and view bar (a
  26 pt control keeps 3 pt above and below); a 14 pt content edge in
  every panel; 10 pt between items, 5 pt between an icon and its text;
  26 pt buttons and 18 pt icons (a row's end icon: 16 pt on a 22 pt hover
  face; the top bar's controls 22 pt, 5 pt above and below; the step
  icons 16 pt, under play's 18); 4 pt control corners, 6 pt floating
  corners. Menus (the app menu, Recent, every dropdown's list) share the
  floating panel and the row height; a dropdown's list opens below it.
- **Structure**: a title bar with the "Reco" app menu (shortcuts,
  preferences, bug report, version), the project, Export and three panel
  toggles. Setup (left) and Adjust (right) are flat panels: a title row,
  then collapsible section bands over rows. Adjust rows are property rows
  (label, control, value) whose labels explain themselves in tooltips. The
  viewer has a view bar (name, Aspect, Record and its menu) over a black
  canvas.
- **Panels slide**: a toggle (button, ⌘1/⌘2/⌘3, View menu) slides Setup,
  Adjust or the time panel's lanes out past the window's edge, or back in,
  in 0.2 s easing out; the panel's rows keep their width (`RecoPanelBox`
  holds them) and the picture gives way smoothly. A window resize ends a
  slide at once and folds as before. Plan: `plans/2026-10-06-panel-motion.md`.
- **Value fields**: every slider's number is a field (`RecoValueField`), a
  quiet box with the digits on the right: a shade lighter on hover, a green
  ring while typing. A click selects the digits; Return or a click away
  applies the value through the slider (which keeps it in range) and gives
  the keyboard back to the preview; Escape puts the value back (and holds
  Escape, so a sheet stays open); ↑/↓ step the last digit shown (⇧: ten).
  The unit is optional and a decimal comma reads (`value_text::Reading`).
  A field typed in keeps its text while the value moves under it; a field
  left as it was changes nothing. Plan: `plans/2026-10-06-value-fields.md`.
  The time panel spans the bottom and shows only what exists: a control
  row (step, play, and a status line; the time once there is a stitch to
  play), then, once a camera has video, a ruler and one lane per camera: its
  L or R badge and each file as a slim block, a gap at every file boundary.
  The lanes fold away (⌘3), leaving the control row.

## Rules

1. **Side by side.** The new app lived in `crates/reco-desktop` next to the
   Slint app, which stayed untouched and working until the new app passed
   every parity check; then it was removed in one commit (2026-10-06).
2. **Engine untouched.** No changes to the engine crates except small
   additive hooks the UI needs, each called out in its commit and in the
   crate's `FRICTION.md`.
3. **Makepad pinned.** One exact commit (see Decisions). Upgrading is its
   own step: bump, rebuild, rerun every module check, commit.
4. **Logic outside the UI.** State, settings and jobs live in `reco-app`,
   which has no Makepad types and has unit tests. `reco-desktop` only draws
   and sends actions. Nothing slow runs on the UI thread.
5. **One theme file.** Every colour, spacing step, radius, font size and
   animation timing comes from `reco-desktop/src/theme.rs`. Screens use
   theme names, never raw values. The `screens_use_theme_values_only` unit
   test enforces this for colours, sizes, spacing and type sizes.
6. **Checklist per screen.** Each module has a checklist in
   [PARITY.md](PARITY.md) built from [the Slint inventory](docs/slint-inventory.md):
   every control, state and shortcut. Known Slint bugs (listed below) are
   fixed, not copied.
7. **Done means proven in the running app.**
   - Build in release and launch with `--remote`.
   - Click or type every checklist item through `tools/drive.py`.
   - Take screenshots of each state: empty, loaded, busy and error.
   - The app log must be free of errors, and `cargo test` and `clippy` must
     pass.
   - The owner gets the screenshots at the end of each module.
8. **Speed targets.**
   - The preview runs at display rate with no CPU copies (macOS).
   - UI work stays under 4 ms per frame.
   - CPU use is about 0% when paused.
   - The first window frame appears in under 1 s, excluding pipeline set-up.
9. **Ease of use.**
   - One primary action per area, and advanced settings collapsed by default.
   - Every control has a label or tooltip.
   - Every long task shows progress and has Cancel.
   - Esc closes dialogs, and shortcuts never fire behind a dialog.
   - Every control can be reached and operated from the keyboard.
   - Nothing typed or expanded is lost when a panel or dialog closes.
10. **Reco code standards.** rustfmt; `clippy -D warnings`; `///` on public
    items; `//!` on modules; tests in each module; conventional commit
    messages; one commit per finished step.
11. **Containers own every gap.** Text roles (`RecoText` and its family)
    carry no padding or margin, and Reco controls carry no outer margin;
    insets live on the enclosing view. Everything in a panel starts on its
    14 pt content edge, and a status line's detail starts under its words.
    A trailing icon (`RecoRowIcon`) reaches past the row's padding by its
    own inset, so its ink, not its box, ends on the right content edge.
    `check_m0.py` measures where the ink starts and ends, not just widget
    boxes.

### Makepad behaviour the rules work around

- `Label` applies its padding and margin twice (once to its box, once to
  its text), so an inset label drifts off its edge. Hence Rule 11.
- `Slider` and `SliderMinimal` sit 4 pt in and 8 pt down from their row,
  and `SliderMinimal` draws its track at the bottom of its box (room for a
  label). `RecoSlider` drops the margin and draws its own centred track and
  knob; a property row shows the value in its own field.
- `Splitter` keeps an open pane at its floor and has no setter for the
  floors; a script apply (`script_apply_eval!`) can set them, but puts the
  widget's other runtime values back to its DSL. A slide relaxes the floors,
  then sets them back from the theme and sets the bar and the fold again
  (panel_motion.rs). FRICTION.md.
- `TextInput` lays a single line out at its natural width, so neither
  `label_align` nor a fixed width's `align` moves it: `RecoValueField` is
  Fit, held to one width by `min` and `max`, and its layout aligns the text
  right. A press inside a selection keeps it, then drops it as the press
  ends, so the field clears its selection when typing ends (the next click
  would leave the caret where it landed instead of the digits selected).
- `Slider` sets the grab hand over itself and the closed hand while pressed,
  in code, and its number field shows the text cursor over the track and
  takes the keyboard on a press even when hidden. `RecoSlider` sizes the
  field away, and the App puts the arrow back over sliders after each
  pointer event (`slider_arrow`; the sliders come from the widget tree,
  since `find_widgets_from_point` from the root finds nothing). FRICTION.md.
- `FoldHeader` draws a body it has never measured whole, whatever its
  state, so a closed fold first drawn in view showed open until the next
  redraw. `RecoFold` clips a closed fold's first draw to its header and
  draws again on the next frame. FRICTION.md.
- Theme values derived from others (`font_body_m`, `font_title_l` and the
  like) are computed from the base theme, so they stay IBM Plex. The menu
  layer reads them, so `MenuLayer` is styled where it is placed. The theme
  script needs `use mod.text.*` and `use mod.res.*` to name fonts.
- A `use mod.widgets.*` is a snapshot: a widget registered in the same
  `script_mod!` block is named by its full path (`mod.widgets.X`).
- The remote snapshot reports a widget's `area()`: the field marked
  `#[area]`, else the first `#[redraw]` field. A custom widget makes its
  whole box the redraw area. Redrawing a draw quad that has never been
  drawn does nothing, so `RecoPreview` redraws through its box and only
  reports its frame quad (`#[area]`).
- A script can set `visible:` on a custom widget only if the widget has a
  `#[visible] #[live(true)] visible: bool` field; an unknown property is a
  runtime script error, not a build error.
- `scroll.y` is the macOS wheel delta negated (scrolling away is negative),
  the opposite of Slint's `delta-y`.
- The `log` crate has no backend in the app: whatever the remote log ring
  (and so a check) must see is logged with Makepad's `log!`.
- On macOS `maximize()` toggles fullscreen; `fullscreen()` does nothing.
- Buttons take key focus on a click by default (`grab_key_focus`), and a
  focused button eats Space, so after any click the preview's shortcuts
  went dead and Space re-clicked the button. Reco's buttons turn it off
  (Tab still focuses them). A dropdown always takes focus on a click and
  steps its choice on arrow keys, so a mouse pick hands focus back and a
  keyboard pick keeps it. Checkboxes and sliders also take focus on a
  click; they get the same hand-back when Modules 4 and 5 wire them.
- Negative margins work, and are how trailing icons line up (Rule 11).
- An SVG icon is scaled by its drawn content, not its viewBox, so a small
  drawing is blown up to fill the icon box. Every icon pins its whole
  viewBox with an invisible rect (the `every_icon_pins_its_viewbox` test).
  Icons are [Lucide](https://lucide.dev)'s (owner's choice, 2026-10-05),
  fetched and converted by `tools/lucide_icons.py`: painted in `#000` for the
  widget to tint, the box pinned, Record and Stop filled and drawn at 9 pt.
  A new icon is a line in that script's table.
- Buttons and dropdowns add 4 pt above and below themselves; Reco's
  versions set `margin: 0`.
- A plain `View` with `show_bg` draws nothing; a filled box is a
  `SolidView` (or `RoundedView`).
- A disabled button's icon keeps its colour: `RecoIconButton` and
  `RecoButton` fade it, `RecoPrimaryButton` greys it.
- Disabled buttons still take Tab focus, and Tab follows draw-list nesting
  rather than reading order (`nav.rs`). Known gaps until Module 2.
- A press captures the pointer by itself: `FingerMove` and `FingerUp` keep
  coming while dragging off the widget (`FingerUp` has `cancelled`). A
  widget that emits several actions in one pass is read with
  `filter_widget_actions_cast`, not `find_widget_action`.
- A button's icon is swapped by assigning `draw_icon.svg` (a
  `ScriptHandleRef` from a `#[live]` field); Makepad reloads the SVG when the
  handle changes.
- `new_batch: true` gives a View its own draw list, skipped while clean. A
  widget that redraws every frame still makes the window walk its tree, so
  the static side panels are cached and the draw time is measured
  (`--perf-log`: about 0.1 to 0.3 ms a frame while playing).
- Makepad's `Toaster` does not hit-test (clicks fall through), places itself
  against the whole window and has fixed lifetimes, so Reco draws its own
  toast cards.
- `DropDown` has no `visible` field: hiding one hides its wrapper instead.
- `#[derive(Script)]` rejects path types on fields (`perf::DrawStats`):
  import the type and use the bare name.
- `Event::Shutdown` arrives before quitting; it is where a recording is
  finished.
- A list of rows from a template follows FlatList: collect the named child
  in `on_after_apply`, make rows with `WidgetRef::script_from_value`, and
  register them with `widget_tree_insert_child` so snapshots see them
  (`ui::file_list`).
- Makepad's `MenuLayer` fixes its geometry in code (22 pt rows, an empty
  24 pt mark column), so Reco's menus are a `Popover` (`ui::menu_list`'s
  `RecoMenu`) holding a `RecoMenuList`; `open_at` opens one from anywhere
  (the next-step card's "Recent files…").
- A popup drawn over the title bar loses real presses: the window's drag
  query answers "caption" for any point in the bar not over a control
  hung there, so the OS drags the window instead. A dropdown's list opens
  below it (`BelowInput`) so no row lies there.
- A window accepts dropped files by answering `Event::Drag` with
  `DragResponse::Copy`.
- `ids!(...)` in an array of tuples is `&[LiveId; 1]`; pass `*id` where a
  slice is wanted.
- A cached panel (`new_batch`) keeps its own draw list, and a redraw of the
  window doesn't reach it: only a redraw of one of its own areas does.
  Nor does a change of its parent's size: a toast card stayed drawn where it
  was when a panel opened over it, while `/snap` reported it moved (checks
  of such widgets look at pixels). A redraw asked for while drawing is
  dropped, and a cached view's `area()` lies in its parent's list, so
  `RecoToasts` notes its room as it draws and redraws its cards on the next
  frame when the room changes.
  Showing or hiding a widget asks for no redraw at all, so `App::set_visible`
  redraws the side panels when visibility changes.
- A fold (`FoldHeader`) opens from its chevron only, not from its title.
- A hit's `fe.rect` is the area's visible part (scrolled and clipped), so a
  list in a scroll view finds the row under the pointer from each row's
  own `clipped_rect` (`ui::pick_list`), not from the list's top.
- `CheckBox` starts its text 13 pt in, counting on padding Reco's rows
  don't have: `RecoCheckBox` puts it after the 15 pt box and an 8 pt gap.
- `Window::resize` sets the outer frame and may leave the displays;
  `reposition` fits the window to them, so a restored size is followed by
  a reposition where the window is.
- Makepad covers the app's network, clipboard and log needs without a
  crate: `cx.http_request` (answers in `Event::NetworkResponses`),
  `cx.copy_to_clipboard`, and `log_ring::read_since` (always on, not only
  with `--remote`).
- `FileDialogAction::FolderSelected` carries no dialog id.
- Text is exact curve coverage with no hinting or smoothing: fine on Retina,
  thin on a 1x display. A shader's default is replaced by re-registering a
  fresh `#(DrawText::script_shader(vm))` that splats `..mod.draw.DrawText`
  (deriving the old object loses the shader's `vertex` and `fragment`), and
  a script that writes shader code needs `use mod.pod.*`, `use mod.math.*`
  and `use mod.shader.*`; a fuller coverage curve made that way looked
  worse to the owner than bigger type.
- `--dpi N` draws at a chosen density on any display
  (`cx.set_window_dpi_override`); `--window-size` stays in the display's
  points.

## Modules

| # | Module | Done when |
|---|---|---|
| 0 | Shell and look | The crate builds in the workspace on Rust 1.92. Theme tokens are in place. The window shell (title bar with the app menu, Setup panel, viewer with its view bar and next-step states, Adjust panel, time panel with camera lanes) works, with three panel toggles. The minimum, default and large window sizes look right. `tools/drive.py` exists and the Module 0 check passes. The owner approves the look. |
| 1 | Preview | `reco-app` exists with the session. Reco's renderer draws into the viewer through the zero-copy bridge. Pan, zoom, reset, keyboard nudges, preview aspect and playback ticking work. The fallback path builds. |
| 2 | Time panel and status | Play, pause, step and scrubbing on the ruler, with the export-range tint and time display. Lanes from the real files (chapter boundaries, each camera's start and end). Record with quality. The status line and toasts. |
| 3 | Files | Left and right segment lists (add, remove, reorder). Calibration: auto-calibrate with progress and Cancel, load and clear. Recent files. Drop videos onto the window. |
| 4 | Stitching and calibration | Seam blend, colour match, rig tilt and roll, sync offset, intersect, axis offset, `x_ty`, save and reset calibration, field ROI (paste and show). |
| 5 | Camera and lens | FOV, constrained look, reset view, lens info, lens correction, lens preview with side, fine-tune sliders, reset lens, lens picker dialog, stats. |
| 6 | Export | Output path, resolution, codec and quality, processing range, replay and debug options, AI tracking (model, mode, interval, preset, framing, pitch lock, lookahead with VRAM risk, advanced panner), progress and Cancel, show in folder. |
| 7 | Preferences and help | The app menu's commands: preferences dialog, keyboard shortcuts dialog, bug report dialog, version. Update check, settings persistence (including window size and panel state). |
| 8 | Parity sweep | Every PARITY.md item is ticked with evidence, plus a polish and performance pass. Then the Slint app is retired, with the owner's sign-off. |

## Verification tooling

- `tools/drive.py` (Python standard library only) starts the release binary
  with `--remote`, waits for the port and offers `snap`, `click`, `type`,
  `key`, `grab`, `log` and `quit`. It fails on any `[E]` log line.
- Each module's check is a script `tools/check_m<N>.py` that walks its
  PARITY.md items and saves screenshots to
  `target/desktop-checks/m<N>/<state>.png`.
- Test windows are started hidden (`MAKEPAD_HIDE_WINDOWS=1`) unless the owner
  is watching, and always closed with `/gq`.
- Checks never reach outside: `drive.launch_env` sets
  `RECO_DESKTOP_NO_NETWORK`, `RECO_DESKTOP_NO_BROWSER` and
  `RECO_DESKTOP_NO_CLIPBOARD`, which turn each request, link and copy into a
  log line (a switch naming a folder also keeps the payload there for the
  check to read), `RECO_DESKTOP_FAKE_RELEASE` answers the update check,
  and `RECO_DESKTOP_FAKE_AI` stands in for a machine that can't run the
  detector. `RECO_DESKTOP_LOG_CURSOR` logs each change of the mouse
  cursor, for the checks that hold sliders to the arrow.
  `RECO_DESKTOP_LAUNCHED_AT` (set by `drive.launch`) makes the first frame
  log how long after the launch it came, and `RECO_DESKTOP_LOG_FILE` gives
  each launch its own log file.
  Dialogs are answered through `RECO_DESKTOP_DIALOG_ANSWERS`, and settings go
  to a fresh `RECO_CONFIG_DIR`. Even a hidden window goes full screen, so no
  check starts one full screen.

## Slint issues to fix, not copy

From [the inventory](docs/slint-inventory.md), section 9:

- Toasts do not render reliably, and they overwrite the status line.
- Shortcuts fire behind dialogs. Std widgets keep focus, so Space re-clicks
  the last button. Esc does not close dialogs.
- Calibration cannot be cancelled, and its Advanced sliders stay live while
  it runs.
- Synchronous decoder reopen and seek, ROI image preparation and lens search
  run on the UI thread.
- Panels and dialogs lose expanded state, typed text and scroll position
  when closed.
- Window size is saved but never restored. Panel widths are not saved.
- The theme is partial: about 116 hard-coded colours, and std widgets ignore
  the dark-mode toggle.
- ROI dots misalign when the preview aspect differs from the render target.
  The render target is not DPI-aware.
- The export codec list maps fixed indices onto a probed list.
- The stats panel only shows export telemetry. Dropped-frame and calibration
  stats are never filled in.
- The update check opens the browser by itself. Bug reports are sent without
  telemetry opt-in and silently copied to the clipboard.
- The shortcuts dialog promises arrow-key frame stepping that does not exist.
