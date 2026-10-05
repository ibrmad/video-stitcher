# Reco Desktop: design, rules and plan

Status: approved 2026-10-04 (framework, rules, module order).
Branch: `feat/makepad-desktop-ui`.

## Goal

A new desktop app for Reco that looks modern, is easy to use and is fast,
built with Makepad 2. It replaces the Slint app (`crates/reco-gui`) once it
does everything that app does.

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
| Slint app | frozen | Stays buildable and is the behavioural reference until parity sign-off, then is removed in one commit. |
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

Threading, adopted from Makepad's own rules:

- The UI thread never blocks. It shares no `Mutex` or `RwLock` with workers,
  never waits on a channel, and never opens or seeks a decoder or does file I/O
  beyond small settings writes.
- Workers are long-lived and fed by bounded channels. Large payloads travel
  as `Arc`.

## Look

After the Rerun viewer. Every value is a token in `src/theme.rs`.

- **Surfaces**, Rerun's neutral grey scale: viewport `#000000`, panels and
  the title bar `#0d0d0d`, view bars `#171717`, section bands and floating
  panels (menus, the export card) `#212121`, control faces `#2c2b2b`,
  separators `#272626`.
- **Text**: Inter Medium at 12 px for everything (Makepad's bundled
  variable Inter at weight 500), 11 px for small print, one 20 px SemiBold
  title for the viewer's next step. Colour does the hierarchy: white for
  names and titles, `#cfcfcf` for values, `#939090` for labels and meta.
- **Accent**: pitch green where Rerun uses blue, and nowhere else: the
  primary action (white on `#007541`), focus rings, slider and progress
  fills, the lit seam, camera lanes (`#34d399` marks, `#0f4a30` blocks).
- **Geometry**: 24 pt rows, title rows and bands; a 12 pt content edge in
  every panel; 8 pt between items, 4 pt between an icon and its text;
  22 pt buttons; 4 pt control corners, 6 pt floating corners.
- **Structure**: a title bar with the "Reco" app menu (shortcuts,
  preferences, bug report, version), the project, Export and three panel
  toggles. Setup (left) and Adjust (right) are flat panels: a title row,
  then collapsible section bands over rows. Adjust rows are property rows
  (label, control, value) whose labels explain themselves in tooltips. The
  viewer has a view bar (name, preview aspect, Record) over a black canvas.
  The time panel spans the bottom and shows only what exists: a control
  row (step, play, and a status line; the time once there is a stitch to
  play), then, once a camera has video, a ruler and one lane per camera: its
  L or R badge and each file as a slim block, a gap at every file boundary.
  The lanes fold away (⌘3), leaving the control row.

## Rules

1. **Side by side.** The new app lives in `crates/reco-desktop` next to the
   Slint app. The Slint app stays untouched and working until the new app
   passes every parity check; then it is removed in one commit.
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
    12 pt content edge, and a status line's detail starts under its words.
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
  knob; a property row shows the value in its own label.
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
