# Reco Desktop: design and rules

## Goal

Reco's desktop app: modern, easy to use and fast, built with Makepad 2. It
opens two cameras' videos and a calibration into a live stitched preview,
tunes the stitch and the lenses, records, and exports the match, with AI
tracking if wanted.

The app is right when:

- the preview runs at the display rate with no CPU copies on macOS;
- every control, state and shortcut has a check that drives the running app
  and saves screenshots (see Verification tooling);
- each rule below holds.

## Decisions

| Decision | Choice | Why |
|---|---|---|
| UI framework | Makepad 2, a git dependency pinned to one commit of `dev` (`62691a290eb58f7d5234960524429aaada3e572c`) | wgpu 28 and Makepad share one `MTLDevice` on macOS, so a wgpu-rendered 1080p texture adopted with `Texture::adopt_metal_bgra` runs at 120 fps and about 12% CPU with no engine changes. egui and iced need Reco on wgpu 30, which touches 34 files and the Metal interop in three. |
| Platforms | macOS first | Other desktops compile and run, with a portable preview path (GPU readback into a Makepad texture) until each gets a zero-copy path. |
| Engine | unchanged | `reco-core`, `reco-io`, `reco-calibrate`, `reco-autocam`, `reco-control` and `reco-detect` keep their APIs. A gap gets a `FRICTION.md` entry and is raised, not hacked around. |
| Look | the Rerun viewer (`re_ui`) as the design reference | A dense, flat, dark tool UI built around a timeline and side panels, close to Reco's job. Reco keeps pitch green in Rerun's accent role and takes Rerun's density whole: hints live in tooltips. See Look below. |
| Accessibility | known gap | Makepad 2 has no screen-reader bridge on macOS (`CxOsOp::AccessibilityUpdate` is ignored). Full keyboard access is still required (Rule 9). |

## Architecture

Two crates:

| Crate | Kind | Owns |
|---|---|---|
| `reco-app` | library, no UI framework | The preview worker (pipeline, playback, pose), the camera files and calibration (`project`), background jobs (calibrate, export, record, update check, field-outline editor, lens search), settings, toasts, bug report, telemetry, the log file. Unit-tested. |
| `reco-desktop` | binary, Makepad | Theme, window shell, screens, custom widgets, the preview widget. Each feature's behaviour is an `impl App` block in a `*_view.rs` file: it reads state, sends commands and draws. |

Data flow:

1. A UI event reaches an `App` method (a `*_view.rs` file).
2. The method changes `reco-app` state or sends a command (`PreviewCommand`)
   to the preview worker, or starts a job on its own thread.
3. Results come back over a channel; the worker or job wakes the UI with a
   `SignalToUI`, and the UI thread drains the channel with `try_recv`.
4. The UI refreshes the widgets whose state changed.

### Preview

The render thread lives in `reco-app` (`src/preview/worker.rs`); the
`RecoPreview` widget (`reco-desktop/src/ui/preview.rs`) shows its frames.

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
  desktop; the preview check runs both paths.
- The worker sleeps until the next frame while playing and blocks while
  paused and still, so a paused preview costs about 0% CPU. The pose eases
  by time (0.25 per 60 Hz frame) whatever the render rate, and eased renders
  come at most every 8 ms.
- When only the view changed, the preview renders from the frames already on
  the GPU (`Playback::frame_serial` against the frame last sent; reco-core's
  additive `render_uploaded_to_view`): a 5.3K pan costs about 3 ms instead
  of 20.
- A pair that gives no first frame, or videos of different sizes, fail the
  open (`PreviewEvent::Failed`; [FRICTION.md](FRICTION.md)). Anything that
  fails once open (a render, a seek, a decode) pauses playback and is
  reported once (`PreviewEvent::Stopped`); the picture and the panels stay.
  A worker panic is reported the same way ("The preview stopped
  unexpectedly: …"). Space after the end plays again from the start.

### Playback, the time panel and status

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
  `reco_app::toasts` (a pure model with expiry times) and one timer. They
  never write the status line.

### Files and calibration

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
  in `desktop.json`. Files dropped on the window go to the camera row they
  land on.

### Stitching and tuning

- The render thread owns the live calibration. `Tune` sets the blend,
  colour match, tilt, roll or layout; `SetSyncOffset` reopens playback at
  the same frame and measures the lanes again; `SetFieldRoi` sets the
  outline; `SaveCalibration` writes it (atomically) with the live values
  folded in. After each, `Calibration(values)` reports them with `dirty`,
  and the Adjust panel and its title row's Save follow it.
- Straight ahead, the render pitch cancels the rig tilt
  (`rig_correction::render_pitch`); the tilt levels the horizon as the
  view turns.
- The field outline's editor is a browser page, written on a job thread
  into `$XDG_CACHE_HOME/reco/roi` (a sandboxed browser can't read `/tmp`)
  and opened in the browser; checks set `RECO_DESKTOP_NO_BROWSER`.
- Every slider's number is typed into (see Look); values go through the
  slider, which keeps them in range.

### Camera and lens

- The field of view and "stay inside" are view commands (`SetFov`,
  `StayInside`): they don't change the calibration, so they don't mark it
  unsaved. The worker answers each view change, and each open, with the
  field of view it is heading to (`Fov`), and the slider follows the
  wheel, the keys and the picture's limit (staying inside narrows it).
- Lens changes are `Tune` changes on a small `Lens` value (fx, fy, cx, cy,
  k1–k4) for the left camera, the right or both; Reset lens returns to the
  file's lenses. Lens correction is saved with the calibration.
- The fine-tune sliders run 0 to 1 over each term's range (±15% of the
  focal length, ±10% of the picture for the centre), centred on the lens
  loaded or picked; each k term is ±0.3 around its own value.
- Lens names come from the videos' telemetry (a short thread when the
  preview opens) or from the calibration run that made the file; a picked
  profile or a file names its cameras "(picked)" or "(file)".
- The single-camera view uses Reco's `LensPreviewRenderer`, which draws at
  the input's size; the worker fits that picture into the preview's frame
  with a small blit (letterboxed) on both the zero-copy and readback paths.
  Showing one camera opens Fine-tune, where its lens is tuned.
- The lens picker searches on a short thread; only the newest query's
  results are kept. A profile is scaled to the calibration's lens size.
- Stats: the worker times each shown frame's wait for decoded frames and
  its render, and reports once a second from the first frame (reset on
  Play), so the first figure after Play is not diluted by the pause.
- Folds: `RecoFold` wraps Makepad's FoldHeader so a closed body takes no
  pointer events outside the fold (FRICTION.md).

### Export

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
  replay and events are remembered in `desktop.json` for the next export;
  choices made in the sheet and not exported are still there when it opens
  again.
- The View row picks Camera (the view that follows the play or holds a
  pose) or Whole field (180°): one fixed panorama of the pitch, goal line
  to goal line, trimmed to the field outline (`PanoramaLayout` in
  reco-core). Its Size row offers Half (the default: half the cameras'
  detail, rendered with 2×2 samples per pixel) and Full (every camera
  pixel), each labelled with the size the match's saved calibration gives.
  Whole field hides the camera's Size row and the follow rows, which keep
  their values for when Camera comes back. A frame wider than H.264
  encoders take (4096) is written as HEVC: the codec shows HEVC, dimmed,
  with the reason under it, and the codec chosen before comes back after;
  it is the one remembered. A machine without HEVC can't export it, and the
  sheet says why. The export writes `{output}.panorama.json` beside the
  video, mapping its pixels to Reco's yaw and pitch.
- Cancel keeps the part written (StitchJob closes the file properly) and
  the notice names it.
- The last export's speed and where its time goes show in Stats.

### AI tracking

- reco-app takes `reco-autocam` behind its `ai` feature (on by default;
  `coreml` passes through): ONNX Runtime on the CPU. `reco_app::ai` holds
  the sheet's choices and builds the engine's config: the style preset is
  the base and the knobs shown go over it. A ball-only mode raises the
  confidence floor.
- Whether the machine can run the detector is asked once at startup on a
  thread (ONNX Runtime's engines can take a moment to load). Until it
  answers, and when it can't, "Follow the play" is dimmed and off: the
  sheet puts back a click (FRICTION.md), and the status line says why.
- The sheet's rows scroll (`ScrollYView`, capped at the window's height
  less the band, the buttons and a margin: `reco_sheet_rows_max`). The AI
  rows show while tracking is on; the panner's finer knobs are a closed
  Advanced tier (Rule 9).
- The model is the last one used: Choose… keeps it at once. Tracking
  without a usable model (an .onnx file that exists) says why under the
  model, and Export waits; Sweep needs none.
- A style preset sets the knobs it covers. On open, the Advanced tier
  follows the saved preset; the mode, interval, preset, framing, tilt
  lock and lookahead are remembered.
- The lookahead's zones come from the GPU's free and total memory at the
  preview's open, and the source's size and rate (the engine's budget). A
  saved lookahead past the ceiling opens at the safe value. The track
  (`RecoZones` under a knob-only slider) shows the zones dimmed and fills
  up to the knob in its zone's colour; without a reading it is a plain
  slider.
- The job adds the lookahead, then sets up the detector, trackers and
  panner on the session as it opens (`on_session` → `setup_autocam`, with
  the calibration's field outline). It says whether tracking started (the
  card's line; the notice adds "tracked with AI") and sends the engine's
  AI figures to Stats once measured.

### Settings

- Settings live in `desktop.json` in reco-io's settings folder (namespace
  `desktop`; `RECO_CONFIG_DIR` overrides it, and every check launch gets
  its own). Values are stored by name; anything missing or unknown reads as
  its default.
- One home per setting. Preferences holds the app-wide settings only: the
  recording codec and folder, and usage data. The rest live where they are
  used: the export sheet remembers its codec, quality and AI model;
  Record's menu has the recording quality; Adjust has the seam blend.
  There is no separate seam blend default: a recalibration keeps its blend,
  a first one starts at 0.05, and an old saved `default_blend` is ignored.
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
- Quitting (⌘Q, the menu, the window's close) with unsaved calibration
  edits asks first (`QuitRequested` handled, `accept_close` false, then
  `cx.quit()` once answered or saved); a termination signal still quits.

### Keys and logging

- Sheets hold every shortcut: `run_shortcut` returns while one is open, and
  the App hands the preview `SheetOpen` in the scope's props with each key.
- Keys are read by place for Space, the arrows and F11, and by the typed
  character for R, F, + = - _ [ ] (macOS asks the layout, `UCKeyTranslate`;
  Windows and Linux take their text events), so they follow the keyboard's
  layout.
- The log file is reco-app's `log_file`: the `log` crate's lines straight
  in, the app's (Makepad's log ring) copied on each signal and at
  shutdown, panics written by the hook. Checks name their own
  (`RECO_DESKTOP_LOG_FILE`).

### Threading

Adopted from Makepad's own rules:

- The UI thread never blocks. It shares no `Mutex` or `RwLock` with workers,
  never waits on a channel, and never opens or seeks a decoder or does file I/O
  beyond small settings writes.
- Workers are long-lived and fed by bounded channels. Large payloads travel
  as `Arc`.

### Platforms, features and packaging

- The detector features are reco-app's (`ort`, `load-dynamic`, `cuda`,
  `tensorrt`, `directml` on for Windows, `tensorrt-native`, `ncnn`), and
  `automation` builds the benchmark hooks (`automation.rs`).
- Linux is built and run in a container (Ubuntu 24.04, Xvfb, Mesa); it needs
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

## Look

After the Rerun viewer, with more room: Rerun's 12 px text and 24 pt rows
read too small on a 1x monitor, so the design is roomier everywhere. Every
value is a token in `src/theme.rs`.

- **Surfaces**, Rerun's neutral grey scale: viewport `#000000`, panels and
  the title bar `#0d0d0d`, view bars `#171717`, section bands and floating
  panels (menus, the export card) `#212121`, control faces `#2c2b2b`,
  separators `#272626`.
- **Text**: Inter Medium at 13 px for everything (Makepad's bundled
  variable Inter at weight 500; macOS's own interface size), 12 px for
  small print, one 22 px SemiBold title for the viewer's next step. Colour
  does the hierarchy: white for names and titles, `#cfcfcf` for values,
  `#939090` for labels and meta.
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
  slide at once and folds as before (`panel_motion.rs`).
- **Value fields**: every slider's number is a field (`RecoValueField`), a
  quiet box with the digits on the right: a shade lighter on hover, a green
  ring while typing. A click selects the digits; Return or a click away
  applies the value through the slider (which keeps it in range) and gives
  the keyboard back to the preview; Escape puts the value back (and holds
  Escape, so a sheet stays open); ↑/↓ step the last digit shown (⇧: ten).
  The unit is optional and a decimal comma reads (`value_text::Reading`).
  A field typed in keeps its text while the value moves under it; a field
  left as it was changes nothing.
- **Time panel**: it spans the bottom and shows only what exists: a control
  row (step, play, and a status line; the time once there is a stitch to
  play), then, once a camera has video, a ruler and one lane per camera: its
  L or R badge and each file as a slim block, a gap at every file boundary.
  The lanes fold away (⌘3), leaving the control row.

## Rules

1. **One app, every desktop.** macOS has the zero-copy preview; Windows and
   Linux read frames back. All three build, and the Linux build runs in a
   container.
2. **Engine untouched.** No changes to the engine crates except small
   additive hooks the UI needs, each called out in its commit and in the
   crate's `FRICTION.md`.
3. **Makepad pinned.** One exact commit (see Decisions). Upgrading is its
   own step: bump, rebuild, rerun every check, commit.
4. **Logic outside the UI.** State, settings and jobs live in `reco-app`,
   which has no Makepad types and has unit tests. `reco-desktop` only draws
   and sends commands. Nothing slow runs on the UI thread.
5. **One theme file.** Every colour, spacing step, radius, font size and
   animation timing comes from `reco-desktop/src/theme.rs`. Screens use
   theme names, never raw values. The `screens_use_theme_values_only` unit
   test enforces this for colours, sizes, spacing and type sizes.
6. **Every screen has a check.** Each control, state and shortcut is
   exercised by a script in `tools/` (see Verification tooling), which
   drives the running app and saves screenshots.
7. **Done means proven in the running app.**
   - Build in release and launch with `--remote`.
   - Click or type every control through `tools/drive.py`.
   - Take screenshots of each state: empty, loaded, busy and error.
   - The app log must be free of errors, and `cargo test` and `clippy` must
     pass.
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
    `check_shell.py` measures where the ink starts and ends, not just widget
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
  (`panel_motion.rs`). FRICTION.md.
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
- `scroll.y` is the macOS wheel delta negated (scrolling away is negative).
- The `log` crate has no backend in the app: whatever the remote log ring
  (and so a check) must see is logged with Makepad's `log!`.
- On macOS `maximize()` toggles fullscreen; `fullscreen()` does nothing, and
  it is unhandled on Windows and Linux (F maximizes and restores there).
- Buttons take key focus on a click by default (`grab_key_focus`), and a
  focused button eats Space, so after any click the preview's shortcuts
  went dead and Space re-clicked the button. Reco's buttons turn it off
  (Tab still focuses them). A dropdown always takes focus on a click and
  steps its choice on arrow keys, so a mouse pick hands focus back and a
  keyboard pick keeps it. Checkboxes and sliders also take focus on a
  click.
- Negative margins work, and are how trailing icons line up (Rule 11).
- An SVG icon is scaled by its drawn content, not its viewBox, so a small
  drawing is blown up to fill the icon box. Every icon pins its whole
  viewBox with an invisible rect (the `every_icon_pins_its_viewbox` test).
  Icons are [Lucide](https://lucide.dev)'s, fetched and converted by
  `tools/lucide_icons.py`: painted in `#000` for the widget to tint, the
  box pinned, Record and Stop filled and drawn at 9 pt. A new icon is a
  line in that script's table.
- Buttons and dropdowns add 4 pt above and below themselves; Reco's
  versions set `margin: 0`.
- A plain `View` with `show_bg` draws nothing; a filled box is a
  `SolidView` (or `RoundedView`).
- A disabled button's icon keeps its colour: `RecoIconButton` and
  `RecoButton` fade it, `RecoPrimaryButton` greys it.
- Disabled buttons still take Tab focus, and Tab follows draw-list nesting
  rather than reading order.
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
  below it (`BelowInput`) so no row lies there. Off macOS the caption bar
  fits its content and the stock caption label is `height: Fill`, so it is
  0 tall: the label has `reco_bar_height`.
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
  frame when the room changes. Showing or hiding a widget asks for no redraw
  at all, so `App::set_visible` redraws the side panels when visibility
  changes. A cached view drawn at the first frame stays put when a restored
  window size moves its layout (the stepper's badges): keep `new_batch` off
  small widgets in moving layouts.
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
  worse than bigger type.
- `--dpi N` draws at a chosen density on any display
  (`cx.set_window_dpi_override`); `--window-size` stays in the display's
  points.
- Resources resolve to the source tree they were built from unless the
  build sets `MAKEPAD_PACKAGE_DIR`; fonts ship only if the binary's font
  manifest names them, and Makepad's default set lacks the theme's Inter, so
  `app_main!` declares it (the `every_font_the_app_names_is_packaged` test).

## Verification tooling

- `tools/drive.py` (Python standard library only) starts the release binary
  with `--remote`, waits for the port and offers `snap`, `click`, `type`,
  `key`, `grab`, `log` and `quit`. It fails on any `[E]` log line.
- Each area has a check script: `check_shell.py` (every `--look-preview`
  state), `check_preview.py`, `check_time.py`, `check_files.py`,
  `check_stitch.py`, `check_lens.py`, `check_export.py`, `check_prefs.py`
  and `check_app.py` (startup and performance, sheets, quitting, the log
  file), plus `check_theme.py`. Each walks its area's controls and saves
  screenshots to `target/desktop-checks/<area>/<state>.png`; named checks
  run alone (`check_export.py kept`).
- `test_drive.py`, `test_package.py` and `test_lucide.py` test the tools
  themselves; CI runs the first two.
- Test windows are started hidden (`MAKEPAD_HIDE_WINDOWS=1`) unless someone
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
