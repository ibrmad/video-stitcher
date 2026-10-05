# Engine friction

Gaps between what Reco Desktop needs and what the engine crates offer. The
engine stays unchanged (DESIGN.md, Rule 2); each gap is recorded here with
what the app does instead, and raised with the engine.

## Zero-copy decode cannot seek (Module 1)

`SmartFileSource` decodes straight into GPU textures on macOS, but it has no
`seek`. The preview needs seeking for step back, `[`/`]` and (Module 2)
scrubbing, so Module 1 decodes through `FfmpegFileSource`: CPU YUV planes,
uploaded on every render. The 5.3K match pair still plays at the source rate
(about 30 fps, the source's 29.97, on an M1 Pro: `tools/check_m1.py`). The ask: `seek(frame)`
on the zero-copy source, then the preview can switch to it.

## A file that is not a video opens anyway (Module 1)

`FfmpegFileSource::open_from_inputs` only warns when the right file fails to
probe, and decoding errors end the stream (`next_frame` turns an error into
`Ok(None)`). So a pair with a junk right file opens and plays nothing, and a
right video of another size opens and then fails every render.
`PreviewSession::open` refuses both (no first frame; planes of different
sizes), so they read as a failed open in plain words. The ask: fail the open
when the right file does not probe, and report decoding errors as errors.

## Clippy on the engine fails under Rust 1.92

`cargo clippy -- -D warnings` lints path dependencies too, and
`reco-core/src/gpu/mod.rs:477` trips `clippy::unnecessary_cast` on the pinned
toolchain. The app's checks run clippy with `--no-deps` so its own crates are
linted in full. The ask: drop the cast in `reco-core`.

## The encoder ignores timestamps (Module 2)

`FfmpegFileEncoder` writes at a constant rate and ignores `pts_us`, so a
recording cannot keep the display's timing. Recordings are one frame per
source frame shown, which keeps the file at the source rate whatever the
display does. The ask: honour `pts_us`, or say in the trait that it is
ignored.

## Resizing the pipeline keeps the first render target (Module 2)

`StitchPipeline::resize` changes the viewport but not the internal render
target, so the preview's renderer cannot also record at a fixed size. The
recorder builds its own `StitchRenderer` at the recording size. The ask:
recreate the target on resize.

## Lengths ignore the sync offset; failed seeks look like the end (Module 2)

`FfmpegFileSource::total_frames()` ignores the sync offset (a positive offset
skips right frames, so the pair ends earlier), and a seek past the real end
returns `Ok` and then reads as the end of the stream. The app measures the
length from the files (see below) and treats a seek that decodes no frame
as a failed seek: the frame on screen stays and the user is told. The ask:
an offset-aware frame count, and a seek that fails as an error.

## File durations are private; there is no frame count (Module 2)

reco-io's per-file duration helpers are private, and nothing reports a
pair's frame count. The app opens each file once with
`VideoDecoder::open(path)?.duration_secs()` on a short-lived probe thread
and lays the files out itself (`reco_app::preview::lanes`). The ask: a
public probe of a chained input's file durations.

## Calibration takes single files (Module 3)

`calibrate_videos` takes one left and one right file, so the app
calibrates each camera's first file, as the Slint app did. A recalibration
starts at the preview's time only while it falls inside both first files.
The ask: calibrate a chained input at a stitched-timeline time.

## A cancelled stitch job reports success (Module 6)

`StitchJob::run` returns `Ok` with the frames written when its interrupt
flag is set: `session.run` stops early and the file is finished normally.
The Slint app therefore announced a cancelled export as complete. The app
reads its own cancel flag to tell the two apart. The ask: an
`Interrupted { frames }` outcome (or a flag on `StitchResult`).

## Labels blank once over a new modal (Module 6)

On the first run of a freshly built binary, one capture taken while the
export sheet was open showed every `Label` blank (in the sheet and in the
panels behind it), while buttons, fields and dropdowns drew their text;
moving the pointer brought them back. It has not happened again in seven
runs, cold or warm. If it returns, suspect the first draw of the modal's
new draw list (shader or glyph work) leaving the frame without a
follow-up redraw.

## A closed fold's hidden rows take the pointer (Module 5, Makepad)

Makepad's `FoldHeader` draws a closed body scrolled out of sight in a
zero-height clip, but still hands it every event, and its hidden widgets
win presses over the visible rows above the fold. In the Adjust panel the
top-level sliders (Field of view, Seam blend) took no drag while the
Advanced tier under them was closed; opening it made them work, closing it
broke them again. Every fold since Module 0 had it; no check dragged a
top-level slider. The app's `RecoFold` (ui/fold.rs) wraps `FoldHeader` and,
while closed, passes pointer events only inside the fold's own area (its
header). Hidden rows overlapping the header strip itself still get events
there; none of them holds a control where a header has one. The ask: a
closed fold gives its body no events (and draws none of it).

## The lens preview allocates a full-size texture a frame and shows no raw view (Module 5)

`LensPreviewRenderer::render_yuv` creates a new texture at the input's size
for every frame (63 MB at 5.3K), so the single-camera view allocates that
much per frame; the app then fits it into the preview's frame with its own
blit. Neither correction mode shows the raw frame: both re-project the
camera, so the field outline (drawn on raw frames) can't be overlaid where
it belongs. The asks: render into a caller's texture (any size), and a raw
mode.


## A hit's rect is the area's visible part (Module 7, Makepad)

`event.hits(cx, area)` reports `fe.rect` as `area.clipped_rect`: shifted by
the scroll view and clipped to it. A list that finds the row under the
pointer from `fe.abs.y - fe.rect.pos.y` counts from the visible top, so in
the lens picker, scrolled two rows down, the highlight sat two rows above
the pointer and the last two rows could never be picked (the owner found
it). `RecoPickList` now asks each row's own area (`clipped_rect`, which
knows the scroll) whether it holds the pointer, and arrow keys ask the
App to scroll the highlighted row into view. The ask: hits carry the
content's rect (or a position relative to it) as well as the visible one.

## A checkbox's text sits on its box without padding (Module 7, Makepad)

`CheckBox` starts its label 13 pt in (`label_walk` margin), counting on
the default padding to clear the 15 pt mark box. With `padding: 0` (Reco's
rows own their insets) the text overlapped the box by 2 pt, in the export
sheet and in Preferences (the owner found it). `RecoCheckBox` sets the
label's margin to the box and an 8 pt gap (`reco_check_label`); check_m6
and check_m7 measure the gap. The ask: the label offset follows the mark's
size.

## Resizing a window doesn't keep it on the displays (Module 7, Makepad)

`Window::resize` sets the outer frame (`setFrame`) as asked, even larger
than every attached display, while `reposition` fits the window to the
displays. Restoring a size saved on a large external display would open a
window past the laptop's screen, so the app repositions it where it is
after resizing. The ask: `resize` fits like `reposition` (and says it sets
the outer size).

## A hidden window still goes full screen (Module 7, Makepad)

With `MAKEPAD_HIDE_WINDOWS=1` (every check), `maximize()` (macOS:
`toggleFullScreen:`) still takes the window full screen on the owner's
display: a one-off probe of "restore full screen" did. Checks therefore
don't run a full-screen start; `remember_window`'s test and that probe
cover it. The ask: hidden windows ignore full screen (or fake it).

## Small gaps met in Module 7 (Makepad)

- A single-line `TextInput` uses ↑/↓ itself and doesn't pass them on
  (`KeyDownUnhandled`), so the lens picker's search can't hand the arrow
  keys to its results; the list is reached with the pointer, as in the
  Slint app.
- `FileDialogAction::FolderSelected` carries no dialog id: the app's one
  folder dialog (Preferences' recording folder) owns every answer.
- `KEYCODE_VARIANTS` isn't re-exported by `makepad-widgets`; the test that
  holds the shortcuts sheet to the key handler takes it from
  `makepad-key-code` (a dev-dependency at the pinned rev).

## Text on a 1x display is thin and soft (Module 7, Makepad)

On macOS every glyph is drawn by exact curve coverage (`sample_slug_pixel`):
no hinting, no font smoothing, baselines wherever layout puts them. On
Retina that looks right; on a 1x external monitor the owner found the text
"not really great": thin and grey next to macOS's own text, whose font
smoothing makes strokes fuller (the same word in CoreText carried about
45% more ink). A fuller coverage curve below 2x (Makepad's own
`sample_slug_pixel` re-registered in the theme with `1 - (1 - a)^2`) was
tried and the owner found it "much worse": the answer is size, so the
design gets more room (bigger type, icons and rows). The asks stand: font
smoothing for low-density displays, and baselines snapped to device pixels.
