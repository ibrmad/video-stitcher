# reco-desktop

Reco's desktop app, built with [Makepad 2](https://github.com/makepad/makepad).
Open two cameras' videos and a calibration, watch the stitched panorama, tune
the stitch and the lenses, record, and export the match with optional AI
tracking. The app's logic lives in [`reco-app`](../reco-app) (no UI
framework, unit-tested); this crate draws it.

- [DESIGN.md](DESIGN.md): architecture, the look, the rules, and the Makepad
  behaviour the code works around.
- [FRICTION.md](FRICTION.md): gaps in the engine and in Makepad, and what the
  app does about each.

## Build and run

```bash
cargo build --profile desktop -p reco-desktop
target/desktop/reco-desktop
target/desktop/reco-desktop --left a.mp4 --right b.mp4 --calibration match.json
```

`--left` and `--right` take a camera's files in order, joined with `;`.
Other flags: `--window-size WxH`, `--preview-readback` (copy frames instead of
sharing textures), `--dpi N`, `--perf-log`, `--look-preview[=STATE]` (the
window with sample content, for design review) and `--remote` (Makepad's
control port, used by the checks).

AI tracking is on by default (ONNX Runtime on the CPU). Other detector
backends are features: `coreml`, `cuda`, `tensorrt`, `directml`,
`tensorrt-native`, `ncnn`; `load-dynamic` loads ONNX Runtime at run time, as
the releases do. `automation` adds benchmark hooks (`RECO_AUTOLOAD`,
`RECO_AUTOEXPORT`, `RECO_VRAM_BUDGET_GB`) and stays off in releases.

On Linux the build needs the development packages for X11, Xcursor, GLX,
xkbcommon, PulseAudio, ALSA, gbm, drm and OpenSSL besides FFmpeg's.

## Test

```bash
cargo test -p reco-app -p reco-desktop
cargo test --profile desktop -p reco-app export::tests   # tracked exports
python3 crates/reco-desktop/tools/test_drive.py
python3 crates/reco-desktop/tools/test_package.py
```

The tracked-export tests need an optimized build: Metal's validation trips
in debug builds (FRICTION.md).

The checks drive the running app through its `--remote` port and save
screenshots to `target/desktop-checks/<area>/`. Build the `desktop` profile
first, then:

```bash
python3 crates/reco-desktop/tools/check_<area>.py [NAME...]
```

with `<area>` one of `shell`, `preview`, `time`, `files`, `stitch`, `lens`,
`export`, `prefs` or `app` (and `check_theme.py`); `NAME...` runs only the
named checks. Set `RECO_FIXTURE_LEFT`, `RECO_FIXTURE_RIGHT` and
`RECO_FIXTURE_CAL` to a short camera pair and its calibration. Each launch
gets its own settings folder, and no check touches the network, the
browser or the clipboard.

## Package

`tools/package.py` builds a release that runs without the source tree, with
Makepad's fonts and icons beside it:

```bash
python3 crates/reco-desktop/tools/package.py --out dist --features load-dynamic
```

On macOS it makes a signed `Reco.app` with cargo-makepad (install it from the
revision `Cargo.toml` pins: `cargo install --git
https://github.com/makepad/makepad --rev <rev> cargo-makepad --locked`),
ad hoc unless `--cert` names an identity. On Windows and Linux it makes a
folder (`--name`, `--target`). `--extra FILE` ships a file beside the binary,
such as the ONNX Runtime library. The release workflow runs it for each
platform.

## Settings and logs

Settings are `desktop.json` in Reco's settings folder (`RECO_CONFIG_DIR`
overrides the folder). The log is `reco-desktop.log` in `~/Library/Logs` on
macOS, in `$XDG_STATE_HOME/reco` (else `~/.local/state/reco`) on Linux and
beside the executable on Windows (`RECO_DESKTOP_LOG_FILE` names a file).
