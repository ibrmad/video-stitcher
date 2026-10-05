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
