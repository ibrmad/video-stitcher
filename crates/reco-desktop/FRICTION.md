# Friction

Gaps between what Reco Desktop needs and what its dependencies offer. The
engine stays unchanged (DESIGN.md, Rule 2); each gap is recorded here with
what the app does instead, and what would remove the workaround ("The ask").
Entries marked Done are fixed and kept for the reason.

## Engine

### Zero-copy decode cannot seek

`SmartFileSource` decodes straight into GPU textures on macOS, but it has no
`seek`. The preview needs seeking for step back, `[`/`]` and scrubbing, so it
decodes through `FfmpegFileSource`: CPU YUV planes, uploaded on every render.
The 5.3K match pair still plays at the source rate (about 30 fps, the
source's 29.97, on an M1 Pro: `tools/check_preview.py`). The ask: `seek(frame)`
on the zero-copy source, then the preview can switch to it.

### A file that is not a video opens anyway

`FfmpegFileSource::open_from_inputs` only warns when the right file fails to
probe, and decoding errors end the stream (`next_frame` turns an error into
`Ok(None)`). So a pair with a junk right file opens and plays nothing, and a
right video of another size opens and then fails every render.
`PreviewSession::open` refuses both (no first frame; planes of different
sizes), so they read as a failed open in plain words. The ask: fail the open
when the right file does not probe, and report decoding errors as errors.

### The encoder ignores timestamps

`FfmpegFileEncoder` writes at a constant rate and ignores `pts_us`, so a
recording cannot keep the display's timing. Recordings are one frame per
source frame shown, which keeps the file at the source rate whatever the
display does. The ask: honour `pts_us`, or say in the trait that it is
ignored.

### Resizing the pipeline keeps the first render target

`StitchPipeline::resize` changes the viewport but not the internal render
target, so the preview's renderer cannot also record at a fixed size. The
recorder builds its own `StitchRenderer` at the recording size. The ask:
recreate the target on resize.

### Lengths ignore the sync offset; failed seeks look like the end

`FfmpegFileSource::total_frames()` ignores the sync offset (a positive offset
skips right frames, so the pair ends earlier), and a seek past the real end
returns `Ok` and then reads as the end of the stream. The app measures the
length from the files (see below) and treats a seek that decodes no frame
as a failed seek: the frame on screen stays and the user is told. The ask:
an offset-aware frame count, and a seek that fails as an error.

### File durations are private; there is no frame count

reco-io's per-file duration helpers are private, and nothing reports a
pair's frame count. The app opens each file once with
`VideoDecoder::open(path)?.duration_secs()` on a short-lived probe thread
and lays the files out itself (`reco_app::preview::lanes`). The ask: a
public probe of a chained input's file durations.

### Calibration takes single files

`calibrate_videos` takes one left and one right file, so the app
calibrates each camera's first file. A recalibration starts at the
preview's time only while it falls inside both first files. The ask:
calibrate a chained input at a stitched-timeline time.

### A cancelled stitch job reports success

`StitchJob::run` returns `Ok` with the frames written when its interrupt
flag is set: `session.run` stops early and the file is finished normally.
The app reads its own cancel flag to tell a cancelled export from a
finished one. The ask: an `Interrupted { frames }` outcome (or a flag on
`StitchResult`).

### StitchJob is silent before the first frame

`StitchJob` reports nothing between its start and the first frame (probing,
opening, seeking). The export card says where the export starts instead.

### The lens preview allocates a full-size texture a frame and shows no raw view

`LensPreviewRenderer::render_yuv` creates a new texture at the input's size
for every frame (63 MB at 5.3K), so the single-camera view allocates that
much per frame; the app then fits it into the preview's frame with its own
blit. Neither correction mode shows the raw frame: both re-project the
camera, so the field outline (drawn on raw frames) can't be overlaid where
it belongs. The asks: render into a caller's texture (any size), and a raw
mode.

### The detector's Metal preprocessing leaves an encoder open

Debug builds turn on wgpu's validation, and with it Metal's API
validation. A tracked export there stops with Metal's `Command encoder
released without endEncoding`. reco-detect's Metal preprocessing
(`metal_compute.rs`) opens its compute encoder before steps that can
return early with `?` (fetching the planes' Metal textures), so such a
return drops the encoder open. Optimized builds (the app's) export with
tracking fine. reco-app's two tracked export tests run in optimized builds
only (`cargo test --profile desktop -p reco-app export::tests`). The
ask: open the encoder after the steps that can fail, or end it on every
path.

### A lookahead's detections reach no telemetry

With a lookahead (the default, 2.5 s), detection runs in the session's
buffered produce phase (`detect_and_track_only`). That phase records
neither the detections nor the detection time, and the frame loop then
skips detection, so the telemetry's AI figures (detection time,
detections a frame, tracks, ball presence) stay zero for the whole export.
Reco sends AI figures only once the engine has measured something, so
Stats shows them for exports without a lookahead. The ask: record the
produce phase's detections and detection time.

### Without ONNX Runtime there is no execution-provider probe

reco-detect's `probe_execution_providers` exists only with ORT: a native
backend build (ncnn, tensorrt-native) takes AI tracking as ready, and an
export that can't start the detector says so.

### A view change re-sent both frames to the GPU (Done)

`StitchPipeline::render_to_view` uploads both cameras' planes before every
render, so a pan, a zoom or a tuning drag while paused sent the same two
frames again each time. On the 5.3K match pair that is about 44 MB a
picture: pans ran at about 52 pictures a second, 19.7 ms each on average
and 37.9 ms at the slowest, above a 60 Hz display's 16.7 ms (DESIGN.md
Rule 8). Measured on the pair at 1100×620: 20.5 ms a render with the
upload, 2.2 ms from the frames already on the GPU, pixel for pixel the same
picture. The engine's no-upload path (`render_to_target_gpu`) draws into an
internal target fixed at the size the renderer was built with
(`StitchPipeline::resize` only changes the viewport), so it is wrong after
the viewer's first resize, and the renderer's own `render_to_view` is
private behind the pipeline.

Done: reco-core has `StitchPipeline::render_uploaded_to_view`, an additive
hook that `render_to_view` and `render_nv12_to_view` now end with
(`render_uploaded_to_view_draws_the_last_upload`, a GPU test run with
`--ignored`). The preview session sends a frame only when playback has
taken a new one (`Playback::frame_serial`): 5.3K pans redraw about 82 times
a second, 3.4 ms each and 6.1 ms at the slowest, and 5.3K playback dropped
from 87% to 76% CPU (`check_app.py perf`).

### Clippy on the engine fails under Rust 1.92 on macOS

`cargo clippy -- -D warnings` lints path dependencies too, and
`reco-core/src/gpu/mod.rs:477` (macOS only) trips `clippy::unnecessary_cast`
on the pinned toolchain. The app's checks run clippy with `--no-deps` so its
own crates are linted in full. The ask: drop the cast in `reco-core`.

## Makepad

### A closed fold's hidden rows take the pointer

Makepad's `FoldHeader` draws a closed body scrolled out of sight in a
zero-height clip, but still hands it every event, and its hidden widgets
win presses over the visible rows above the fold. In the Adjust panel the
top-level sliders (Field of view, Seam blend) took no drag while the
Advanced tier under them was closed; opening it made them work, closing it
broke them again. The app's `RecoFold` (ui/fold.rs) wraps `FoldHeader` and,
while closed, passes pointer events only inside the fold's own area (its
header). Hidden rows overlapping the header strip itself still get events
there; none of them holds a control where a header has one. The ask: a
closed fold gives its body no events (and draws none of it).

### A closed fold first drawn shows open

`FoldHeader` draws its body whole until it has measured it, whatever its
state, so a closed fold whose first draw is in view shows open for that
frame and stays so until something redraws it: the export sheet's AI
Advanced tier showed open on opening the sheet and closed on the first
scroll. `RecoFold` clips a closed fold's first draw to its header and asks
for another draw, which is measured and closed. The ask: a closed fold
measures its body without showing it.

### A slider shows a hand and a text cursor

`Slider` sets `MouseCursor::Grab` on hover and `Grabbing` on a press in its
event handler, with no property to change it, and its number field (the
readout typed values go into) shows the text cursor over the track and
takes the keyboard on a press even while hidden. Reco wants the arrow, as
macOS sliders have, and its own value fields. `RecoSlider` sizes the
readout away (0 by 0, read-only), and the App puts the arrow back after
each pointer event over a slider (`App::slider_arrow`). It finds the
sliders through the widget tree: `find_widgets_from_point` from the root
returns nothing. The ask: a `cursor` property on `Slider`, a readout that
can be turned off, and a hit search that works from the root.

### A hit's rect is the area's visible part

`event.hits(cx, area)` reports `fe.rect` as `area.clipped_rect`: shifted by
the scroll view and clipped to it. A list that finds the row under the
pointer from `fe.abs.y - fe.rect.pos.y` counts from the visible top, so in
the lens picker, scrolled two rows down, the highlight sat two rows above
the pointer and the last two rows could never be picked. `RecoPickList` now
asks each row's own area (`clipped_rect`, which knows the scroll) whether it
holds the pointer, and arrow keys ask the App to scroll the highlighted row
into view. The ask: hits carry the content's rect (or a position relative
to it) as well as the visible one.

### A checkbox's text sits on its box without padding

`CheckBox` starts its label 13 pt in (`label_walk` margin), counting on
the default padding to clear the 15 pt mark box. With `padding: 0` (Reco's
rows own their insets) the text overlapped the box by 2 pt, in the export
sheet and in Preferences. `RecoCheckBox` sets the label's margin to the
box and an 8 pt gap (`reco_check_label`); `check_export.py` and
`check_prefs.py` measure the gap. The ask: the label offset follows the
mark's size.

### A disabled checkbox still takes clicks

`CheckBox::set_disabled` only plays the disabled look: its event handler
has no gate, unlike `Button`, which has `enabled`. The export sheet's
"Follow the play" looks disabled until the machine can run the detector,
and the sheet puts it back off when a click or Space flips it then. The
ask: a disabled widget ignores input, or `CheckBox` gets an `enabled` as
`Button` has.

### Resizing a window doesn't keep it on the displays

`Window::resize` sets the outer frame (`setFrame`) as asked, even larger
than every attached display, while `reposition` fits the window to the
displays. Restoring a size saved on a large external display would open a
window past the laptop's screen, so the app repositions it where it is
after resizing. The ask: `resize` fits like `reposition` (and says it sets
the outer size).

### A hidden window still goes full screen

With `MAKEPAD_HIDE_WINDOWS=1` (every check), `maximize()` (macOS:
`toggleFullScreen:`) still takes the window full screen on the display: a
one-off probe of "restore full screen" did. Checks therefore don't run a
full-screen start; `remember_window`'s test and that probe cover it. The
ask: hidden windows ignore full screen (or fake it).

### No full screen on Windows and Linux

`Window::fullscreen()` pushes an op the Windows and Linux backends don't
handle, and `maximize()` is `ShowWindow(SW_MAXIMIZE)` there (on macOS it is
`toggleFullScreen:`), so F maximized once and never came back. F now
maximizes and restores off macOS (`keys::fullscreen_step`). The ask: full
screen on every desktop backend.

### Text on a 1x display is thin and soft

On macOS every glyph is drawn by exact curve coverage (`sample_slug_pixel`):
no hinting, no font smoothing, baselines wherever layout puts them. On
Retina that looks right; on a 1x external monitor the text is thin and grey
next to macOS's own text, whose font smoothing makes strokes fuller (the
same word in CoreText carried about 45% more ink). A fuller coverage curve
below 2x (Makepad's own `sample_slug_pixel` re-registered in the theme with
`1 - (1 - a)^2`) was tried and looked worse than bigger type: the answer is
size, so the design gets more room (bigger type, icons and rows). The asks
stand: font smoothing for low-density displays, and baselines snapped to
device pixels.

### A popup over the title bar loses presses

The window answers each `WindowDragQuery` from its caption geometry: a
press in the caption strip is the window manager's drag unless it lands on
the chrome buttons or on a control hung in the caption bar. A popup's rows
drawn over the strip are neither, so on macOS a real click on them dragged
the window and the row was never chosen (press-and-drag worked, since the
popup already held the pointer). Injected clicks (the checks) never reach
the OS hit test, so no check could see it. Reco's dropdowns open their list
below themselves. The ask: an open popup's rects count as the client's in
the drag query.

### Makepad's menu fixes its geometry

`MenuLayer` (what `MenuButton` opens) takes colours and fonts from the
theme but fixes its geometry in code: 22 pt rows, 4 pt padding and a 24 pt
mark column reserved on every row, so text started 28 pt in. Reco's menus
are a `Popover` with a `RecoMenuList` (template rows: items, separators,
sections). The ask: row height, padding and the mark column as style
values.

### The menu bar's shortcuts didn't fire

⌘1, ⌘2, ⌘3 and ⌘, did nothing on the ABC layout on one Mac. Each is a menu
item with its key equivalent (Makepad's `WindowMenu` builds the `NSMenu` on
its first draw, and a fired item marks the press so the window doesn't see
it), and the path reads right, but the checks can't press a real key: the
remote injects keys as `Event::KeyDown`, past AppKit, and a `--remote` run
never takes focus. So no check could press them, and the cause there is not
known. The app also reads its menu shortcuts as keys (`keys::app_shortcut`),
as ⌘S already was; a press the menu takes never reaches the window, so
nothing runs twice. `check_prefs.py keys` presses each. The ask: a way to
drive menu key equivalents from the remote, so a check covers the AppKit
path.

### A splitter can't slide a pane past its floor

`Splitter` clamps an open pane to its floors, folds a pane only at once
(`set_collapse`), and has no setter for the floors, so a panel couldn't
slide closed: its width stopped at the floor. A script apply
(`script_apply_eval!`) sets the floors, but puts the widget's other runtime
values (its align and fold) back to its DSL. The app's slides relax the
floors for their 0.2 s, set them back from the theme after, and set the bar
and the fold again each time (`panel_motion.rs`). The ask: a floor setter,
or a collapse that animates.

### A cached view isn't redrawn when its room changes

A view with its own draw list (`new_batch`) is skipped while clean, and a
change of its parent's size doesn't mark it: after a calibration opened
the Adjust panel, the "Calibrated" toast stayed drawn where the wider
viewer had put it, under the panel, until a new toast redrew it. `/snap`
reported the card at its new place all along, so only pixels showed it. A
redraw asked for while drawing is dropped (`redraw_list` returns in a
draw), and the view's `area()` lies in its parent's list. `RecoToasts`
notes its room as it draws and redraws its cards on the next frame when
the room changes; `check_time.py toasts` looks at their pixels after a
slide, a drag and a resize. The ask: redraw a cached view whose turtle
moved, and let `/snap` report what was drawn.

The same at start: the step badges in the viewer's empty state each had a
cached draw list, drawn at the first frame's size; when the saved window
size was restored the words moved and the badges stayed. They now draw in
their parent's list (their digits still draw over the fill), and
`check_shell.py stepper` launches with a saved window size, since the
checks' `--window-size` skips the restore.

### The caption bar's filling label is 0 tall off macOS

The caption bar fits its content unless window buttons size it (macOS's
traffic lights do), and its stock label is `height: Fill`: off macOS the
two make a 0-tall bar, and the app's top bar inside it (the app menu,
Export, the panel toggles) vanished on Linux. The label now has the bar's
height (`main.rs`). The ask: a caption bar that holds app content at the
content's height everywhere.

### macOS sends text only to a text field

Key events carry only the key's place (`KeyCode`), and on macOS text
events come only while a text field has the input method (`ime_active`);
Windows (`WM_CHAR`) and X11 send what a key typed. So the preview's
character keys couldn't follow the keyboard's layout on macOS: `typed.rs`
asks the layout with `UCKeyTranslate` (Carbon's input sources, which abort
off the main thread). The ask: the typed character on `KeyEvent`.

### Resources load from the build machine's paths (Done)

`crate_resource("self:…")` resolves to the crate's source folder at build
time unless the build sets `MAKEPAD_PACKAGE_DIR` (then `package/crate/…`
beside the executable, or a `<exe>.makepad-package-paths` map), which
Makepad's own packager does. A plain `cargo build` binary moved to another
machine draws no text or icons ("is not packaged with this app, so its text
will show as boxes"). `MAKEPAD_PACKAGE_DIR` alone isn't enough: the default
font (Inter) is still looked for at the build machine's path, because fonts
are packaged through a manifest in the binary (`app_main!`'s `font_assets`)
that Makepad's packager reads. `cargo makepad desktop bundle` makes a
self-contained macOS `.app` (every crate's resources, the fonts the binary
uses, a package map); Windows and Linux have no such step.

Done: `tools/package.py` runs cargo-makepad's bundle on macOS and lays out
the same layout on Windows and Linux; it reads the font manifest from the
binary's text, as cargo-makepad reads it from ELF and Mach-O sections only
(a Windows binary keeps no long section names). The default International
manifest doesn't name the theme's Inter, so no package carried it and the
packaged app drew no text: `app_main!` declares `INTER_FONT_ASSET`, and
`every_font_the_app_names_is_packaged` holds the theme to the manifest. The
International set ships CJK and emoji fonts (about 48 MB of a package); a
Latin set would be far smaller and show other scripts as boxes.

### Smaller gaps

- A single-line `TextInput` uses ↑/↓ itself and doesn't pass them on
  (`KeyDownUnhandled`), so the lens picker's search can't hand the arrow
  keys to its results; the list is reached with the pointer.
- `FileDialogAction::FolderSelected` carries no dialog id: the app's one
  folder dialog (Preferences' recording folder) owns every answer.
- `KEYCODE_VARIANTS` isn't re-exported by `makepad-widgets`; the test that
  holds the shortcuts sheet to the key handler takes it from
  `makepad-key-code` (a dev-dependency at the pinned rev).
- Makepad logs `[E] PulseAudio: pa_context_connect failed` where no sound
  server runs (a container), before falling back to ALSA.
- Makepad's log has no hook: the app copies its log ring into the log file
  on each signal and at shutdown (`log_view.rs`); engine lines go there
  straight through the `log` crate.

### Seen once: labels blank over a new modal

On the first run of a freshly built binary, one capture taken while the
export sheet was open showed every `Label` blank (in the sheet and in the
panels behind it), while buttons, fields and dropdowns drew their text;
moving the pointer brought them back. It has not happened again in seven
runs, cold or warm. If it returns, suspect the first draw of the modal's
new draw list (shader or glyph work) leaving the frame without a
follow-up redraw.
