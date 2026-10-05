# Engine friction

Gaps between what Reco Desktop needs and what the engine crates offer. The
engine stays unchanged (DESIGN.md, Rule 2); each gap is recorded here with
what the app does instead, and raised with the engine.

## Zero-copy decode cannot seek (Module 1)

`SmartFileSource` decodes straight into GPU textures on macOS, but it has no
`seek`. The preview needs seeking for step back, `[`/`]` and (Module 2)
scrubbing, so Module 1 decodes through `FfmpegFileSource`: CPU YUV planes,
uploaded on every render. The 5.3K match pair still plays at the source rate
(30.0 of 29.97 fps on an M1 Pro, `tools/check_m1.py`). The ask: `seek(frame)`
on the zero-copy source, then the preview can switch to it.

## Clippy on the engine fails under Rust 1.92

`cargo clippy -- -D warnings` lints path dependencies too, and
`reco-core/src/gpu/mod.rs:477` trips `clippy::unnecessary_cast` on the pinned
toolchain. The app's checks run clippy with `--no-deps` so its own crates are
linted in full. The ask: drop the cast in `reco-core`.
