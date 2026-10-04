# Module 0: Shell and Look — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A `reco-desktop` binary that opens the Reco window shell in the new look. The shell has a top bar in the title bar, the Media sidebar, the viewer with its empty state, the Inspector, the transport bar and the status bar. Panels fold by hand and on narrow windows, and everything is verified through Makepad's `--remote` control.

**Architecture:**
- A new workspace crate `crates/reco-desktop` depends on Makepad 2, pinned by git rev.
- `src/theme.rs` derives `mod.themes.reco_dark` from Makepad's dark theme, between `theme_mod` and `widgets_mod`, so stock widgets pick up the Reco palette.
- `src/ui/*.rs` register the shell's styled widgets.
- `src/shell_state.rs` and `src/cli.rs` hold the only logic. Both are pure and unit-tested.
- `tools/drive.py` drives the release binary over HTTP for the checks.

**Tech Stack:** Rust 1.92 (pinned toolchain), Makepad 2 (`makepad-widgets`, git), Python 3 standard library for tools.

**Spec:** `crates/reco-desktop/DESIGN.md` (rules, decisions, modules) and `crates/reco-desktop/PARITY.md` (Module 0 items).

## Global Constraints

- **Toolchain:** Rust `1.92.0` from `rust-toolchain.toml`; do not bump it.
- **Edition:** `reco-desktop` uses edition 2021 (Makepad's macros target 2021). The rest of the workspace stays on 2024.
- **Makepad pin:**
  `makepad-widgets = { git = "https://github.com/makepad/makepad", rev = "62691a290eb58f7d5234960524429aaada3e572c", default-features = false }`.
- **Engine crates:** none of `reco-core`, `reco-io`, `reco-calibrate`, `reco-autocam`, `reco-control`, `reco-detect` or `reco-gui` changes.
- **Theme:** every colour, radius, spacing step and type size comes from `src/theme.rs` (DESIGN Rule 5). Colours are always written `#x…`.
- **Window:**
  - Title "Reco", default size 1280×820.
  - Usable down to 720×600. Makepad has no minimum-size API, so panels fold below 960 (Inspector) and 700 (Media) points.
- **Code standards:**
  - `cargo fmt`, and `cargo clippy -p reco-desktop --all-targets -- -D warnings` must be clean.
  - `///` on public items, `//!` on modules, tests in each logic module.
  - Conventional commits.
- **Checks:**
  - `tools/*.py` use the standard library only.
  - Screenshots go to `target/desktop-checks/m0/`.
  - Test instances run hidden (`MAKEPAD_HIDE_WINDOWS=1`) and are closed with `/gq`.
- **Build profile for checks:** `--profile desktop` (release without LTO, incremental). It is defined additively in the workspace `Cargo.toml`.

## Review Focus

1. **Narrow windows (720×600):** the Inspector folds by itself, nothing overlaps, and the Media panel stays open. Owner: Task 3 (unit tests) and Task 6 (720×600 screenshot plus rect assertions).
2. **Hand toggles versus the width rule:** reopening a panel by hand on a narrow window must not be undone by the next resize event. Owner: Task 3 (`hand_reopen_survives_further_narrow_resizes`).
3. **A typo in an icon path or theme key** is reported only as a log line. Owner: Task 2 (`App.errors()`), and Task 6 fails on any `[E]` line.
4. **Inspector toggle before files are loaded** must do nothing and look disabled. Owner: Task 3 (`inspector_toggle_refused_before_load`) and Task 6 (clicking it before load leaves the Inspector closed).
5. **Bad `--window-size` values** (`wide`, `100x100`, missing value) must give a clear error and the default window, not a crash. Owner: Task 3 (`rejects_bad_sizes`), and Task 5 logs the error and continues.

---

### Task 1: Crate skeleton with a pinned Makepad window

**Files:**
- Modify: `Cargo.toml` (workspace members; add `[profile.desktop]`)
- Create: `crates/reco-desktop/Cargo.toml`
- Create: `crates/reco-desktop/src/main.rs`

**Interfaces:**
- Produces: binary `target/desktop/reco-desktop`; window titled "Reco"; script module order `theme_mod → widgets_mod → app`.

- [ ] **Step 1: Add the crate to the workspace and the desktop profile**

In `Cargo.toml`, add `"crates/reco-desktop",` after `"crates/reco-gui",` in `members`, and append:

```toml
# Fast optimised builds for the Makepad desktop app's checks
# (crates/reco-desktop/DESIGN.md, "Verification tooling"): release code
# without LTO, incremental. Shipped builds keep using `release`.
[profile.desktop]
inherits = "release"
lto = false
incremental = true
codegen-units = 16
```

- [ ] **Step 2: Write `crates/reco-desktop/Cargo.toml`**

```toml
[package]
publish = false
name = "reco-desktop"
version = "0.1.0"
# Makepad's macros (app_main!, script_mod!) are written for edition 2021.
edition = "2021"
rust-version.workspace = true
license.workspace = true
repository.workspace = true
description = "Makepad 2 desktop app for Reco (replaces reco-gui; see DESIGN.md)"

[[bin]]
name = "reco-desktop"
path = "src/main.rs"

[dependencies]
# Pinned per DESIGN.md Rule 3. The widget families are opt-in; the core
# has every widget Module 0 uses.
makepad-widgets = { git = "https://github.com/makepad/makepad", rev = "62691a290eb58f7d5234960524429aaada3e572c", default-features = false }
```

- [ ] **Step 3: Write a minimal `src/main.rs`**

```rust
//! Reco Desktop: the Makepad 2 desktop app for Reco.
//!
//! See `DESIGN.md` for the plan, rules and module order.

pub use makepad_widgets;
use makepad_widgets::*;

app_main!(App);

script_mod! {
    use mod.prelude.widgets.*

    startup() do #(App::script_component(vm)){
        ui: Root{
            main_window := Window{
                window.title: "Reco"
                window.inner_size: vec2(1280, 820)
                body +: {
                    Label{text: "Reco"}
                }
            }
        }
    }
}

/// The application: the widget tree.
#[derive(Script, ScriptHook)]
pub struct App {
    #[live]
    ui: WidgetRef,
}

impl MatchEvent for App {}

impl AppMain for App {
    fn script_mod(vm: &mut ScriptVm) -> ScriptValue {
        makepad_widgets::theme_mod(vm);
        makepad_widgets::widgets_mod(vm);
        self::script_mod(vm)
    }

    fn handle_event(&mut self, cx: &mut Cx, event: &Event) {
        self.match_event(cx, event);
        self.ui.handle_event(cx, event, &mut Scope::empty());
    }
}
```

- [ ] **Step 4: Build**

Run: `cargo build --profile desktop -p reco-desktop`
Expected: it finishes. The first build fetches Makepad (about 140 MB) and compiles for a few minutes.

- [ ] **Step 5: Launch and check the title**

Run:
```bash
MAKEPAD_HIDE_WINDOWS=1 ./target/desktop/reco-desktop --remote > /tmp/reco-desktop.log 2>&1 &
```
Read the port from the `[makepad-remote] listening on 127.0.0.1:PORT` line, then:

```bash
curl -s "http://127.0.0.1:$PORT/s"
curl -s "http://127.0.0.1:$PORT/gq"
```

Expected: `/s` reports a window titled `Reco [remote]`, and `/gq` answers `"quit":1`.

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml Cargo.lock crates/reco-desktop/Cargo.toml crates/reco-desktop/src/main.rs
git commit -m "feat: add reco-desktop crate with a pinned Makepad 2 window"
```

---

### Task 2: Remote driver for checks

**Files:**
- Create: `crates/reco-desktop/tools/drive.py`
- Create: `crates/reco-desktop/tools/test_drive.py`

**Interfaces:**
- Produces (Python):
  - `parse_listen_line(text) -> (port, pid) | None`
  - `read_png(path) -> Png(width, height, rgba: bytes)`
  - `Png.pixel(x, y) -> (r, g, b, a)`
  - `App.launch(binary, args=(), hidden=True, timeout=60) -> App`, also usable as a context manager
  - `App.get(route, **params) -> dict`
  - `App.snap(q) -> list[dict]`
  - `App.rect(widget_id) -> (x, y, w, h) | None`
  - `App.click_id(widget_id)`
  - `App.key(code, **mods)`
  - `App.grab(dest, scale=1.0) -> Png`
  - `App.errors() -> list[str]`
  - `App.quit()`
  - `DriveError`

- [ ] **Step 1: Write the failing tests**

```python
# crates/reco-desktop/tools/test_drive.py
"""Unit tests for drive.py's parsing helpers (no app needed)."""
import os
import struct
import tempfile
import unittest
import zlib

import drive


def write_png(path, width, height, rgba_rows, filter_type=0):
    """Write a minimal 8-bit RGBA PNG with the given rows."""
    raw = b"".join(bytes([filter_type]) + row for row in rgba_rows)

    def chunk(kind, data):
        body = kind + data
        return struct.pack(">I", len(data)) + body + struct.pack(">I", zlib.crc32(body))

    ihdr = struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0)
    with open(path, "wb") as f:
        f.write(b"\x89PNG\r\n\x1a\n")
        f.write(chunk(b"IHDR", ihdr))
        f.write(chunk(b"IDAT", zlib.compress(raw)))
        f.write(chunk(b"IEND", b""))


class ParseListenLine(unittest.TestCase):
    def test_finds_port_and_pid(self):
        text = "noise\n[makepad-remote] listening on 127.0.0.1:53412 pid=9931 app=reco-desktop grabs=/tmp/x\n"
        self.assertEqual(drive.parse_listen_line(text), (53412, 9931))

    def test_none_when_missing(self):
        self.assertIsNone(drive.parse_listen_line("[I] starting\n"))


class ReadPng(unittest.TestCase):
    def test_reads_unfiltered_pixels(self):
        with tempfile.TemporaryDirectory() as d:
            path = os.path.join(d, "a.png")
            write_png(path, 2, 1, [bytes([15, 17, 21, 255, 52, 211, 153, 255])])
            png = drive.read_png(path)
            self.assertEqual((png.width, png.height), (2, 1))
            self.assertEqual(png.pixel(0, 0), (15, 17, 21, 255))
            self.assertEqual(png.pixel(1, 0), (52, 211, 153, 255))

    def test_reads_up_filtered_rows(self):
        # Row 2 uses filter 2 (Up): stored bytes are differences to row 1.
        with tempfile.TemporaryDirectory() as d:
            path = os.path.join(d, "b.png")
            row1 = bytes([10, 20, 30, 255])
            diff = bytes([5, 5, 5, 0])
            raw = b"\x00" + row1 + b"\x02" + diff

            def chunk(kind, data):
                body = kind + data
                return struct.pack(">I", len(data)) + body + struct.pack(">I", zlib.crc32(body))

            with open(path, "wb") as f:
                f.write(b"\x89PNG\r\n\x1a\n")
                f.write(chunk(b"IHDR", struct.pack(">IIBBBBB", 1, 2, 8, 6, 0, 0, 0)))
                f.write(chunk(b"IDAT", zlib.compress(raw)))
                f.write(chunk(b"IEND", b""))
            png = drive.read_png(path)
            self.assertEqual(png.pixel(0, 1), (15, 25, 35, 255))


class ColourClose(unittest.TestCase):
    def test_tolerance(self):
        self.assertTrue(drive.close_to((15, 17, 21, 255), "#0f1115", tol=2))
        self.assertFalse(drive.close_to((40, 17, 21, 255), "#0f1115", tol=2))


if __name__ == "__main__":
    unittest.main()
```

- [ ] **Step 2: Run them and see them fail**

Run: `cd crates/reco-desktop/tools && python3 -m unittest test_drive -v`
Expected: `ModuleNotFoundError: No module named 'drive'`.

- [ ] **Step 3: Write `drive.py`**

```python
#!/usr/bin/env python3
"""Drive a Reco Desktop (Makepad 2) instance through its --remote HTTP API.

Used by the module checks (tools/check_m<N>.py). Standard library only.

    with App.launch(BIN, ["--window-size", "1280x820"]) as app:
        app.click_id("toggle_media")
        png = app.grab("target/desktop-checks/m0/media-closed.png")

Input routes are sent with wait=1, so the next request sees the frame that
followed the input; no sleeps are needed between steps.
"""
import json
import os
import re
import shutil
import struct
import subprocess
import tempfile
import time
import urllib.parse
import urllib.request
import zlib

LISTEN_RE = re.compile(r"\[makepad-remote\] listening on 127\.0\.0\.1:(\d+) pid=(\d+)")


class DriveError(Exception):
    """A remote request failed or the app did not behave as required."""


def parse_listen_line(text):
    """Return (port, pid) from the app's startup output, or None."""
    m = LISTEN_RE.search(text)
    return (int(m.group(1)), int(m.group(2))) if m else None


def hex_rgb(hex_colour):
    """'#0f1115' -> (15, 17, 21)."""
    h = hex_colour.lstrip("#")
    return tuple(int(h[i:i + 2], 16) for i in (0, 2, 4))


def close_to(rgba, hex_colour, tol=3):
    """True when rgba's RGB is within tol of the hex colour on every channel."""
    return all(abs(a - b) <= tol for a, b in zip(rgba[:3], hex_rgb(hex_colour)))


class Png:
    """A decoded 8-bit RGBA image."""

    def __init__(self, width, height, rgba):
        self.width, self.height, self.rgba = width, height, rgba

    def pixel(self, x, y):
        """RGBA tuple at integer pixel (x, y)."""
        i = (y * self.width + x) * 4
        return tuple(self.rgba[i:i + 4])


def _paeth(a, b, c):
    p = a + b - c
    pa, pb, pc = abs(p - a), abs(p - b), abs(p - c)
    if pa <= pb and pa <= pc:
        return a
    return b if pb <= pc else c


def read_png(path):
    """Decode an 8-bit RGB or RGBA, non-interlaced PNG."""
    with open(path, "rb") as f:
        data = f.read()
    if data[:8] != b"\x89PNG\r\n\x1a\n":
        raise DriveError(f"{path}: not a PNG")
    pos, idat = 8, b""
    width = height = colour_type = None
    while pos < len(data):
        length, kind = struct.unpack(">I4s", data[pos:pos + 8])
        body = data[pos + 8:pos + 8 + length]
        if kind == b"IHDR":
            width, height, depth, colour_type, _, _, interlace = struct.unpack(">IIBBBBB", body)
            if depth != 8 or colour_type not in (2, 6) or interlace != 0:
                raise DriveError(f"{path}: unsupported PNG (depth {depth}, type {colour_type})")
        elif kind == b"IDAT":
            idat += body
        elif kind == b"IEND":
            break
        pos += 12 + length
    bpp = 4 if colour_type == 6 else 3
    raw = zlib.decompress(idat)
    stride = width * bpp
    out = bytearray()
    prev = bytearray(stride)
    i = 0
    for _ in range(height):
        ftype = raw[i]
        row = bytearray(raw[i + 1:i + 1 + stride])
        i += 1 + stride
        for x in range(stride):
            a = row[x - bpp] if x >= bpp else 0
            b = prev[x]
            c = prev[x - bpp] if x >= bpp else 0
            if ftype == 1:
                row[x] = (row[x] + a) & 0xFF
            elif ftype == 2:
                row[x] = (row[x] + b) & 0xFF
            elif ftype == 3:
                row[x] = (row[x] + ((a + b) >> 1)) & 0xFF
            elif ftype == 4:
                row[x] = (row[x] + _paeth(a, b, c)) & 0xFF
        if bpp == 3:
            for x in range(width):
                out += row[x * 3:x * 3 + 3] + b"\xff"
        else:
            out += row
        prev = row
    return Png(width, height, bytes(out))


class App:
    """One running instance launched with --remote. Close it with quit()."""

    def __init__(self, proc, port, log_path):
        self.proc, self.port, self.log_path = proc, port, log_path

    @classmethod
    def launch(cls, binary, args=(), hidden=True, timeout=60.0):
        """Start binary with --remote and wait for its control port."""
        env = dict(os.environ)
        if hidden:
            env["MAKEPAD_HIDE_WINDOWS"] = "1"
        log_fd, log_path = tempfile.mkstemp(prefix="reco-desktop-", suffix=".log")
        log = os.fdopen(log_fd, "w")
        proc = subprocess.Popen([binary, "--remote", *args], stdout=log, stderr=subprocess.STDOUT, env=env)
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            with open(log_path) as f:
                found = parse_listen_line(f.read())
            if found:
                return cls(proc, found[0], log_path)
            if proc.poll() is not None:
                break
            time.sleep(0.1)
        proc.kill()
        with open(log_path) as f:
            raise DriveError(f"app did not start remote control:\n{f.read()[-2000:]}")

    def __enter__(self):
        return self

    def __exit__(self, *exc):
        self.quit()

    def get(self, route, timeout=60.0, **params):
        """GET a route and return its JSON answer; raises on {"err": ...}."""
        query = urllib.parse.urlencode({k: v for k, v in params.items() if v is not None})
        url = f"http://127.0.0.1:{self.port}{route}" + (f"?{query}" if query else "")
        try:
            with urllib.request.urlopen(url, timeout=timeout) as r:
                answer = json.loads(r.read().decode())
        except urllib.error.HTTPError as e:
            raise DriveError(f"{route}: HTTP {e.code} {e.read().decode()[:300]}") from e
        if isinstance(answer, dict) and "err" in answer:
            raise DriveError(f"{route}: {answer['err']}")
        return answer

    def snap(self, q):
        """Widgets whose id, type or text contains q (window-local rects)."""
        return self.get("/snap", q=q)["s"]

    def rect(self, widget_id):
        """(x, y, w, h) of the widget with exactly this id, or None if not drawn."""
        for item in self.snap(widget_id):
            if item.get("i") == widget_id and item["r"][2] > 1 and item["r"][3] > 1:
                return tuple(item["r"])
        return None

    def click_id(self, widget_id):
        """Click the centre of a widget found by id."""
        r = self.rect(widget_id)
        if r is None:
            raise DriveError(f"no visible widget `{widget_id}` to click")
        self.get("/click", x=r[0] + r[2] / 2, y=r[1] + r[3] / 2, wait=1)

    def key(self, code, **mods):
        """Press one key, e.g. key("Key1", cmd=1)."""
        self.get("/k", k="press", c=code, wait=1, **mods)

    def grab(self, dest, scale=1.0):
        """Capture the window to dest and return it decoded."""
        answer = self.get("/g", scale=scale)
        os.makedirs(os.path.dirname(dest), exist_ok=True)
        shutil.copyfile(answer["png"], dest)
        return read_png(dest)

    def errors(self):
        """Error lines ([E] or [!]) from the app's own log ring."""
        lines = self.get("/log", n=500)["l"]
        return [line for line in lines if line.startswith("[E]") or line.startswith("[!]")]

    def quit(self):
        """Grab-and-quit, then make sure the process is gone."""
        if self.proc.poll() is None:
            try:
                self.get("/gq", scale=0.25, timeout=30)
            except (DriveError, OSError):
                pass
            try:
                self.proc.wait(timeout=15)
            except subprocess.TimeoutExpired:
                self.proc.terminate()
                self.proc.wait(timeout=5)
```

- [ ] **Step 4: Run the tests**

Run: `cd crates/reco-desktop/tools && python3 -m unittest test_drive -v`
Expected: 5 tests pass.

- [ ] **Step 5: Smoke-test it against the Task 1 binary**

Run:
```bash
cd crates/reco-desktop/tools && python3 -c "
import drive
with drive.App.launch('../../../target/desktop/reco-desktop') as app:
    print(app.get('/s')['w'][0]['t']); print(app.errors())"
```
Expected: `Reco [remote]` and `[]`.

- [ ] **Step 6: Commit**

```bash
git add crates/reco-desktop/tools/drive.py crates/reco-desktop/tools/test_drive.py
git commit -m "test: add the --remote driver used by reco-desktop's module checks"
```

---

### Task 3: Shell state and command-line options (pure logic, TDD)

**Files:**
- Create: `crates/reco-desktop/src/shell_state.rs`
- Create: `crates/reco-desktop/src/cli.rs`
- Modify: `crates/reco-desktop/src/main.rs` (add `mod cli; mod shell_state;`)

**Interfaces:**
- Produces:
  - `shell_state::{Panel::{Media, Inspector}, ShellState, INSPECTOR_FOLD_WIDTH = 960.0, MEDIA_FOLD_WIDTH = 700.0}`
  - `ShellState::default()`
  - `is_open(Panel) -> bool`
  - `files_loaded() -> bool`
  - `can_toggle(Panel) -> bool`
  - `toggle(Panel) -> bool`
  - `set_files_loaded(bool)`
  - `fit_width(f64)`
  - `cli::Args { window_size: Option<(f64, f64)>, look_preview: bool }`
  - `Args::parse<I: IntoIterator<Item = S>, S: AsRef<str>>(I) -> Result<Args, String>`

- [ ] **Step 1: Write the failing tests for `shell_state.rs`**

Create `src/shell_state.rs` containing only the test module and stub types:

```rust
//! Which side panels are open, and the rules that fold them on narrow
//! windows. Pure state, so the rules are unit-tested without a window.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn startup_opens_media_and_closes_inspector() {
        let s = ShellState::default();
        assert!(s.is_open(Panel::Media));
        assert!(!s.is_open(Panel::Inspector));
        assert!(!s.files_loaded());
    }

    #[test]
    fn media_toggles_by_hand() {
        let mut s = ShellState::default();
        assert!(s.toggle(Panel::Media));
        assert!(!s.is_open(Panel::Media));
        assert!(s.toggle(Panel::Media));
        assert!(s.is_open(Panel::Media));
    }

    #[test]
    fn inspector_toggle_refused_before_load() {
        let mut s = ShellState::default();
        assert!(!s.can_toggle(Panel::Inspector));
        assert!(!s.toggle(Panel::Inspector));
        assert!(!s.is_open(Panel::Inspector));
    }

    #[test]
    fn loading_opens_inspector_and_unloading_closes_it() {
        let mut s = ShellState::default();
        s.set_files_loaded(true);
        assert!(s.is_open(Panel::Inspector));
        assert!(s.can_toggle(Panel::Inspector));
        s.set_files_loaded(false);
        assert!(!s.is_open(Panel::Inspector));
    }

    #[test]
    fn narrowing_folds_inspector_then_media_and_widening_restores_both() {
        let mut s = ShellState::default();
        s.set_files_loaded(true);
        s.fit_width(1280.0);
        s.fit_width(900.0);
        assert!(!s.is_open(Panel::Inspector));
        assert!(s.is_open(Panel::Media));
        s.fit_width(650.0);
        assert!(!s.is_open(Panel::Media));
        s.fit_width(1280.0);
        assert!(s.is_open(Panel::Inspector));
        assert!(s.is_open(Panel::Media));
    }

    #[test]
    fn first_width_below_limit_folds() {
        let mut s = ShellState::default();
        s.set_files_loaded(true);
        s.fit_width(720.0);
        assert!(!s.is_open(Panel::Inspector));
        assert!(s.is_open(Panel::Media));
    }

    #[test]
    fn hand_reopen_survives_further_narrow_resizes() {
        let mut s = ShellState::default();
        s.set_files_loaded(true);
        s.fit_width(1280.0);
        s.fit_width(900.0);
        assert!(s.toggle(Panel::Inspector));
        s.fit_width(880.0);
        s.fit_width(860.0);
        assert!(s.is_open(Panel::Inspector));
    }

    #[test]
    fn hand_closed_panel_stays_closed_when_widening() {
        let mut s = ShellState::default();
        s.fit_width(1280.0);
        assert!(s.toggle(Panel::Media));
        s.fit_width(650.0);
        s.fit_width(1280.0);
        assert!(!s.is_open(Panel::Media));
    }
}
```

- [ ] **Step 2: Run them and see them fail**

Add `mod shell_state;` to `main.rs`, then run `cargo test -p reco-desktop shell_state`.
Expected: compile errors, because `ShellState` and `Panel` are not defined.

- [ ] **Step 3: Implement `shell_state.rs` (above the test module)**

```rust
/// Below this window width (points) the Inspector folds by itself.
pub const INSPECTOR_FOLD_WIDTH: f64 = 960.0;
/// Below this window width (points) the Media sidebar folds as well.
pub const MEDIA_FOLD_WIDTH: f64 = 700.0;

/// The two side panels of the shell.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Panel {
    /// The Media sidebar on the left (videos and calibration).
    Media,
    /// The Inspector on the right (view, stitching, lens, stats).
    Inspector,
}

/// Open/closed state of the side panels and whether files are loaded.
#[derive(Clone, Debug, PartialEq)]
pub struct ShellState {
    media_open: bool,
    inspector_open: bool,
    media_auto_folded: bool,
    inspector_auto_folded: bool,
    files_loaded: bool,
    last_width: Option<f64>,
}

impl Default for ShellState {
    /// Nothing loaded: Media open, because loading starts there; Inspector
    /// closed, because there is nothing to adjust yet.
    fn default() -> Self {
        Self {
            media_open: true,
            inspector_open: false,
            media_auto_folded: false,
            inspector_auto_folded: false,
            files_loaded: false,
            last_width: None,
        }
    }
}

impl ShellState {
    /// Whether a panel is currently shown.
    pub fn is_open(&self, panel: Panel) -> bool {
        match panel {
            Panel::Media => self.media_open,
            Panel::Inspector => self.inspector_open,
        }
    }

    /// Whether left/right videos are loaded (gates Export, transport and
    /// the Inspector toggle).
    pub fn files_loaded(&self) -> bool {
        self.files_loaded
    }

    /// Whether the panel's toggle is enabled.
    pub fn can_toggle(&self, panel: Panel) -> bool {
        match panel {
            Panel::Media => true,
            Panel::Inspector => self.files_loaded,
        }
    }

    /// Flip a panel by hand. Returns false (and changes nothing) when the
    /// toggle is disabled. A hand toggle cancels any automatic fold.
    pub fn toggle(&mut self, panel: Panel) -> bool {
        if !self.can_toggle(panel) {
            return false;
        }
        match panel {
            Panel::Media => {
                self.media_open = !self.media_open;
                self.media_auto_folded = false;
            }
            Panel::Inspector => {
                self.inspector_open = !self.inspector_open;
                self.inspector_auto_folded = false;
            }
        }
        true
    }

    /// Record that files were loaded or unloaded. Loading opens the
    /// Inspector; call [`ShellState::fit_width`] afterwards on a narrow window.
    pub fn set_files_loaded(&mut self, loaded: bool) {
        if loaded == self.files_loaded {
            return;
        }
        self.files_loaded = loaded;
        self.inspector_open = loaded;
        self.inspector_auto_folded = false;
    }

    /// Apply the width rule after a resize. A panel folds when the window
    /// crosses below its limit (or starts below it) and reopens when the
    /// window crosses back above it, unless the user toggled it meanwhile.
    pub fn fit_width(&mut self, width: f64) {
        let prev = self.last_width.replace(width);
        let below = |limit: f64| width < limit && prev.is_none_or(|p| p >= limit);
        let above = |limit: f64| width >= limit && prev.is_some_and(|p| p < limit);

        if below(INSPECTOR_FOLD_WIDTH) && self.inspector_open {
            self.inspector_open = false;
            self.inspector_auto_folded = true;
        } else if above(INSPECTOR_FOLD_WIDTH) && self.inspector_auto_folded {
            self.inspector_open = true;
            self.inspector_auto_folded = false;
        }
        if below(MEDIA_FOLD_WIDTH) && self.media_open {
            self.media_open = false;
            self.media_auto_folded = true;
        } else if above(MEDIA_FOLD_WIDTH) && self.media_auto_folded {
            self.media_open = true;
            self.media_auto_folded = false;
        }
    }
}
```

- [ ] **Step 4: Run the shell-state tests**

Run: `cargo test -p reco-desktop shell_state`
Expected: 8 tests pass.

- [ ] **Step 5: Write the failing `cli.rs` tests**

```rust
//! Command-line options. Makepad reads its own flags (`--remote`,
//! `--remote=PORT`, ...) from the same list, so unknown flags are ignored.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_args_give_defaults() {
        assert_eq!(Args::parse(Vec::<String>::new()).unwrap(), Args::default());
    }

    #[test]
    fn parses_window_size_in_both_forms() {
        let a = Args::parse(["--window-size", "720x600"]).unwrap();
        assert_eq!(a.window_size, Some((720.0, 600.0)));
        let b = Args::parse(["--window-size=1920X1200"]).unwrap();
        assert_eq!(b.window_size, Some((1920.0, 1200.0)));
    }

    #[test]
    fn parses_look_preview_and_ignores_makepad_flags() {
        let a = Args::parse(["--remote", "--look-preview", "--remote=5000"]).unwrap();
        assert!(a.look_preview);
        assert_eq!(a.window_size, None);
    }

    #[test]
    fn rejects_bad_sizes() {
        assert!(Args::parse(["--window-size", "wide"]).is_err());
        assert!(Args::parse(["--window-size", "100x100"]).is_err());
        assert!(Args::parse(["--window-size"]).is_err());
    }
}
```

Add `mod cli;` to `main.rs`, then run `cargo test -p reco-desktop cli`.
Expected: compile errors, because `Args` is not defined.

- [ ] **Step 6: Implement `cli.rs` (above the test module)**

```rust
/// Options this app reads from the command line.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Args {
    /// Initial window size in points, from `--window-size WxH`.
    pub window_size: Option<(f64, f64)>,
    /// Show the shell as if files were loaded, with sample content, from
    /// `--look-preview`. For design review until loading exists (Module 1).
    pub look_preview: bool,
}

impl Args {
    /// Parse options from an argument list without the program name.
    pub fn parse<I, S>(args: I) -> Result<Self, String>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut out = Args::default();
        let mut iter = args.into_iter();
        while let Some(arg) = iter.next() {
            let arg = arg.as_ref();
            if arg == "--look-preview" {
                out.look_preview = true;
            } else if let Some(value) = arg.strip_prefix("--window-size=") {
                out.window_size = Some(parse_size(value)?);
            } else if arg == "--window-size" {
                let value = iter
                    .next()
                    .ok_or("--window-size needs a value like 1280x820")?;
                out.window_size = Some(parse_size(value.as_ref())?);
            }
        }
        Ok(out)
    }
}

/// Parse `WxH` (for example `1280x820`) into points.
fn parse_size(value: &str) -> Result<(f64, f64), String> {
    let (w, h) = value
        .split_once(['x', 'X'])
        .ok_or_else(|| format!("window size `{value}` is not WxH"))?;
    let w: f64 = w.trim().parse().map_err(|_| format!("bad window width `{w}`"))?;
    let h: f64 = h.trim().parse().map_err(|_| format!("bad window height `{h}`"))?;
    if !(320.0..=8192.0).contains(&w) || !(240.0..=8192.0).contains(&h) {
        return Err(format!("window size {w}x{h} is outside 320x240..8192x8192"));
    }
    Ok((w, h))
}
```

- [ ] **Step 7: Run all tests and clippy**

Run: `cargo test -p reco-desktop && cargo clippy -p reco-desktop --all-targets -- -D warnings`
Expected: 12 tests pass, and clippy reports nothing. While the modules are unused by `main`, add `#![allow(dead_code)]` at the top of each of the two files with the comment `// Wired into App in Task 5.` and remove it in Task 5.

- [ ] **Step 8: Commit**

```bash
git add crates/reco-desktop/src/shell_state.rs crates/reco-desktop/src/cli.rs crates/reco-desktop/src/main.rs
git commit -m "feat: shell panel rules and command-line options for reco-desktop"
```

---

### Task 4: The Reco theme and icons

**Files:**
- Create: `crates/reco-desktop/src/theme.rs`
- Create: `crates/reco-desktop/resources/icons/` with these files:
  - copied from Makepad's `libs/fab/resources/icons` (MIT, © Makepad B.V.): `help.svg`, `gear.svg`, `play.svg`, `pause.svg`, `plus.svg`, `close.svg`, `folder.svg`, `chevron_down.svg`, `chevron_right.svg`, `warning.svg`, `keyboard.svg`;
  - new: `sidebar_left.svg`, `sidebar_right.svg`, `step_back.svg`, `step_forward.svg`, `export.svg`, `record.svg`.
- Modify: `THIRD_PARTY_NOTICES.md` (Makepad icons, MIT)
- Modify: `crates/reco-desktop/src/main.rs` (theme between `theme_mod` and `widgets_mod`)

**Interfaces:**
- Produces:
  - `crate::theme::script_mod(vm)`
  - Theme roles stock widgets read: `color_bg_app`, `color_app_caption_bar`, `color_text*`, `color_label*`, `color_icon*`, `color_inset*`, `color_outset*`, `color_bevel*`, `color_primary`, `color_on_primary`, `color_primary_container`, `color_on_primary_container`, `color_focus`, `color_bg_highlight*`, `color_error`, `color_warning`, `color_success`, `color_on_surface_variant`, `corner_radius`, `container_corner_radius`, `beveling`, `space_*`.
  - Reco keys:
    - `reco_panel`
    - `reco_surface`
    - `reco_hairline`
    - `reco_viewer`
    - `reco_text_secondary`
    - `reco_text_muted`
    - `reco_record`
    - `reco_topbar_height` (44)
    - `reco_transport_height` (52)
    - `reco_statusbar_height` (26)
    - `reco_panel_pad` (10)
    - `reco_gap` (8)
  - Palette, which the checks verify:

    | Role | Hex |
    |---|---|
    | app | `#0f1115` |
    | panel | `#14171c` |
    | surface | `#1a1d23` |
    | control | `#23272f` |
    | control hover | `#2b303a` |
    | inset | `#171a1f` |
    | hairline | `#2a2f37` |
    | text | `#e6e9ef` |
    | secondary | `#a0a7b2` |
    | muted | `#6b7280` |
    | accent | `#34d399` |
    | on-accent | `#052e1c` |
    | accent container | `#133d2c` |
    | on container | `#a7f3d0` |
    | highlight | `#1f4d3a` |
    | error | `#f87171` |
    | warning | `#fbbf24` |
    | record | `#ef4444` |
    | viewer | `#08090b` |

- [ ] **Step 1: Copy the Makepad icons from the pinned checkout**

Run:
```bash
MP=$(ls -d ~/.cargo/git/checkouts/makepad-*/62691a2 | head -1)
mkdir -p crates/reco-desktop/resources/icons
for i in help gear play pause plus close folder chevron_down chevron_right warning keyboard; do
  cp "$MP/libs/fab/resources/icons/$i.svg" crates/reco-desktop/resources/icons/
done
```

- [ ] **Step 2: Write the six new icons (16×16, fill-only)**

`sidebar_left.svg`:
```svg
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16" width="16" height="16">
<rect x="1" y="2" width="14" height="1.3" fill="#000"/><rect x="1" y="12.7" width="14" height="1.3" fill="#000"/>
<rect x="1" y="2" width="1.3" height="12" fill="#000"/><rect x="13.7" y="2" width="1.3" height="12" fill="#000"/>
<rect x="2.3" y="3.3" width="3.7" height="9.4" fill="#000"/>
</svg>
```
`sidebar_right.svg`: the same frame, with the column at `x="10"`:
```svg
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16" width="16" height="16">
<rect x="1" y="2" width="14" height="1.3" fill="#000"/><rect x="1" y="12.7" width="14" height="1.3" fill="#000"/>
<rect x="1" y="2" width="1.3" height="12" fill="#000"/><rect x="13.7" y="2" width="1.3" height="12" fill="#000"/>
<rect x="10" y="3.3" width="3.7" height="9.4" fill="#000"/>
</svg>
```
`step_back.svg`:
```svg
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16" width="16" height="16">
<rect x="3" y="3" width="1.6" height="10" fill="#000"/><path d="M13 3 L5.5 8 L13 13 Z" fill="#000"/>
</svg>
```
`step_forward.svg`:
```svg
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16" width="16" height="16">
<path d="M3 3 L10.5 8 L3 13 Z" fill="#000"/><rect x="11.4" y="3" width="1.6" height="10" fill="#000"/>
</svg>
```
`export.svg`:
```svg
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16" width="16" height="16">
<path d="M8 1.5 L12 5.5 L9 5.5 L9 10 L7 10 L7 5.5 L4 5.5 Z" fill="#000"/>
<path d="M2 9 L3.5 9 L3.5 12.5 L12.5 12.5 L12.5 9 L14 9 L14 14 L2 14 Z" fill="#000"/>
</svg>
```
`record.svg`:
```svg
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16" width="16" height="16">
<circle cx="8" cy="8" r="4.5" fill="#000"/>
</svg>
```

- [ ] **Step 3: Write `src/theme.rs`**

```rust
//! The Reco look: one theme derived from Makepad's dark theme (DESIGN.md
//! Rule 5). Every colour, radius, spacing step and type size the app uses
//! is set here.
//!
//! Stock widgets copy theme values when they are registered, so this module
//! runs after `makepad_widgets::theme_mod` and before
//! `makepad_widgets::widgets_mod`. Makepad's theme values are computed once,
//! so every role a stock widget reads is set explicitly rather than derived
//! from a few knobs; the role list follows Makepad's own macOS-dark style.
//! Reco's own surfaces read the `reco_*` keys.

use makepad_widgets::*;

script_mod! {
    mod.themes.reco_dark = mod.themes.dark{
        // Geometry
        corner_radius: 5.0
        container_corner_radius: 10.0
        textselection_corner_radius: 3.0
        beveling: 1.0
        space_factor: 7.0
        space_1: 3.5
        space_2: 7.0
        space_3: 10.5
        mspace_1: mod.turtle.Inset{top: 3.5 right: 3.5 bottom: 3.5 left: 3.5}

        // Surfaces
        color_bg_app: #x0f1115
        color_fg_app: #x0f1115
        color_app_caption_bar: #x0f1115
        color_bg_container: #x14171c
        color_bg_highlight: #x1f4d3a
        color_bg_highlight_inline: #x1f4d3a

        // Accent and focus
        color_focus: #x34d399
        color_ctrl_selected: #x34d399
        color_ctrl_active: #x34d399
        color_primary: #x34d399
        color_on_primary: #x052e1c
        color_primary_container: #x133d2c
        color_on_primary_container: #xa7f3d0
        color_text_on_accent: #x052e1c
        color_success: #x34d399
        color_warning: #xfbbf24
        color_error: #xf87171
        color_cursor: #xe6e9ef
        color_text_cursor: #xe6e9ef

        // Text, labels and icons
        color_text: #xe6e9ef
        color_text_hover: #xe6e9ef
        color_text_focus: #xe6e9ef
        color_text_active: #xe6e9ef
        color_text_down: #xe6e9ef
        color_text_disabled: #x6b7280
        color_text_placeholder: #x6b7280
        color_text_meta: #xa0a7b2
        color_on_surface_variant: #xa0a7b2
        color_label: #xe6e9ef
        color_label_hover: #xe6e9ef
        color_label_focus: #xe6e9ef
        color_label_active: #xe6e9ef
        color_label_down: #xe6e9ef
        color_label_disabled: #x6b7280
        color_label_inner: #xe6e9ef
        color_label_inner_hover: #xe6e9ef
        color_label_inner_focus: #xe6e9ef
        color_label_inner_active: #xe6e9ef
        color_label_inner_down: #xe6e9ef
        color_label_inner_disabled: #x6b7280
        color_label_outer: #xe6e9ef
        color_label_outer_hover: #xe6e9ef
        color_label_outer_focus: #xe6e9ef
        color_label_outer_active: #xe6e9ef
        color_label_outer_down: #xe6e9ef
        color_label_outer_disabled: #x6b7280
        color_icon: #xa0a7b2
        color_icon_hover: #xe6e9ef
        color_icon_focus: #xe6e9ef
        color_icon_active: #xe6e9ef
        color_icon_down: #xe6e9ef
        color_icon_disabled: #x4b5160

        // Controls: raised (outset) and sunken (inset) fills
        color_outset: #x23272f
        color_outset_1: #x23272f
        color_outset_2: #x23272f
        color_outset_hover: #x2b303a
        color_outset_1_hover: #x2b303a
        color_outset_2_hover: #x2b303a
        color_outset_focus: #x23272f
        color_outset_1_focus: #x23272f
        color_outset_2_focus: #x23272f
        color_outset_active: #x34d399
        color_outset_1_active: #x23272f
        color_outset_2_active: #x23272f
        color_outset_down: #x1c2026
        color_outset_1_down: #x1c2026
        color_outset_2_down: #x1c2026
        color_outset_disabled: #x181b20
        color_outset_1_disabled: #x181b20
        color_outset_2_disabled: #x181b20
        color_outset_empty: #x23272f
        color_outset_1_empty: #x23272f
        color_outset_2_empty: #x23272f
        color_outset_drag: #x2b303a
        color_outset_1_drag: #x2b303a
        color_outset_2_drag: #x2b303a
        color_inset: #x171a1f
        color_inset_1: #x171a1f
        color_inset_2: #x171a1f
        color_inset_hover: #x1b1f25
        color_inset_1_hover: #x1b1f25
        color_inset_2_hover: #x1b1f25
        color_inset_focus: #x1b1f25
        color_inset_1_focus: #x1b1f25
        color_inset_2_focus: #x1b1f25
        color_inset_active: #x1b1f25
        color_inset_1_active: #x1b1f25
        color_inset_2_active: #x1b1f25
        color_inset_down: #x171a1f
        color_inset_1_down: #x171a1f
        color_inset_2_down: #x171a1f
        color_inset_disabled: #x14171c
        color_inset_1_disabled: #x14171c
        color_inset_2_disabled: #x14171c
        color_inset_empty: #x171a1f
        color_inset_1_empty: #x171a1f
        color_inset_2_empty: #x171a1f
        color_inset_drag: #x1b1f25
        color_inset_1_drag: #x1b1f25
        color_inset_2_drag: #x1b1f25

        // Hairline borders (bevels); focus borders use the accent
        color_bevel: #x2a2f37
        color_bevel_hover: #x353b45
        color_bevel_down: #x2a2f37
        color_bevel_focus: #x34d399
        color_bevel_disabled: #x1f232a
        color_bevel_inset_1: #x2a2f37
        color_bevel_inset_2: #x2a2f37
        color_bevel_outset_1: #x2a2f37
        color_bevel_outset_2: #x2a2f37
        color_bevel_inset_1_hover: #x353b45
        color_bevel_inset_2_hover: #x353b45
        color_bevel_outset_1_hover: #x353b45
        color_bevel_outset_2_hover: #x353b45
        color_bevel_inset_1_focus: #x34d399
        color_bevel_inset_2_focus: #x34d399
        color_bevel_outset_1_focus: #x34d399
        color_bevel_outset_2_focus: #x34d399
        color_bevel_inset_1_active: #x2a2f37
        color_bevel_inset_2_active: #x2a2f37
        color_bevel_outset_1_active: #x2a2f37
        color_bevel_outset_2_active: #x2a2f37
        color_bevel_inset_1_down: #x2a2f37
        color_bevel_inset_2_down: #x2a2f37
        color_bevel_outset_1_down: #x2a2f37
        color_bevel_outset_2_down: #x2a2f37
        color_bevel_inset_1_disabled: #x1f232a
        color_bevel_inset_2_disabled: #x1f232a
        color_bevel_outset_1_disabled: #x1f232a
        color_bevel_outset_2_disabled: #x1f232a

        // Reco surfaces and sizes
        reco_panel: #x14171c
        reco_surface: #x1a1d23
        reco_hairline: #x2a2f37
        reco_viewer: #x08090b
        reco_text_secondary: #xa0a7b2
        reco_text_muted: #x6b7280
        reco_record: #xef4444
        reco_topbar_height: 44.0
        reco_transport_height: 52.0
        reco_statusbar_height: 26.0
        reco_panel_pad: 10.0
        reco_gap: 8.0
    }
    mod.theme = mod.themes.reco_dark
}
```

- [ ] **Step 4: Register the theme in `main.rs`**

Add `mod theme;`, then make the registration order:

```rust
fn script_mod(vm: &mut ScriptVm) -> ScriptValue {
    makepad_widgets::theme_mod(vm);
    crate::theme::script_mod(vm);
    makepad_widgets::widgets_mod(vm);
    self::script_mod(vm)
}
```

- [ ] **Step 5: Add the notice**

Append this to `THIRD_PARTY_NOTICES.md`:

```markdown
## Makepad icons

`crates/reco-desktop/resources/icons/{help,gear,play,pause,plus,close,folder,chevron_down,chevron_right,warning,keyboard}.svg`
are copied from Makepad (https://github.com/makepad/makepad,
`libs/fab/resources/icons`), MIT License, Copyright (c) 2023 Makepad B.V.
```

- [ ] **Step 6: Build, launch and check the background colour**

Run:
```bash
cargo build --profile desktop -p reco-desktop
cd crates/reco-desktop/tools && python3 -c "
import drive
with drive.App.launch('../../../target/desktop/reco-desktop') as app:
    png = app.grab('../../../target/desktop-checks/m0/theme-smoke.png')
    print(png.pixel(png.width - 4, png.height - 4)); print(app.errors())"
```

Expected:
- The corner pixel is within 3 of `#0f1115` on each channel (`drive.close_to(..., '#0f1115')` is true).
- `errors()` is `[]`. If the log names an unknown theme key, delete that key from `theme.rs` (it does not exist in this Makepad version) and record it in "Deviations" at the end of this plan.

- [ ] **Step 7: Commit**

```bash
git add crates/reco-desktop/src/theme.rs crates/reco-desktop/src/main.rs crates/reco-desktop/resources THIRD_PARTY_NOTICES.md
git commit -m "feat: Reco dark theme and icon set for reco-desktop"
```

---

### Task 5: The window shell

**Files:**
- Create:
  - `crates/reco-desktop/src/ui/mod.rs`
  - `controls.rs`
  - `top_bar.rs`
  - `media_panel.rs`
  - `viewer.rs`
  - `inspector.rs`
  - `transport.rs`
  - `status_bar.rs`
  - `shell.rs`
- Modify: `crates/reco-desktop/src/main.rs` (shell, menu, toggles, width rule, look preview; remove the `dead_code` allows)

**Interfaces:**
- Consumes:
  - `ShellState`, `Panel` and `Args` from Task 3.
  - Theme keys from Task 4.
- Produces (widget ids the checks use):
  - Top bar: `toggle_media`, `app_name`, `project_name`, `help_button`, `prefs_button`, `export_button`, `toggle_inspector`.
  - Media panel: `media_panel`, `add_left`, `add_right`, `auto_calibrate`, `load_calibration`, `recent_button`.
  - Viewer: `viewer`, `empty_state`, `empty_add`, `empty_recent`, `sample_frame`.
  - Inspector: `inspector`, `fov_slider`, `seam_blend`, `match_colours`.
  - Transport: `step_back`, `play_pause`, `step_forward`, `time_current`, `timeline`, `time_total`, `record_button`, `aspect`.
  - Status bar: `status_text`, `version_text`, `report_bug`.
  - Splitters: `main_split`, `inner_split`.

- [ ] **Step 1: `src/ui/mod.rs`**

```rust
//! The window shell's widgets, registered in dependency order. Each file
//! registers its widgets under `mod.widgets.Reco*`.

mod controls;
mod inspector;
mod media_panel;
mod shell;
mod status_bar;
mod top_bar;
mod transport;
mod viewer;

use makepad_widgets::*;

/// Register every shell widget. Call after `makepad_widgets::widgets_mod`
/// and before the app's own script module.
pub fn script_mod(vm: &mut ScriptVm) {
    controls::script_mod(vm);
    top_bar::script_mod(vm);
    media_panel::script_mod(vm);
    viewer::script_mod(vm);
    inspector::script_mod(vm);
    transport::script_mod(vm);
    status_bar::script_mod(vm);
    shell::script_mod(vm);
}
```

- [ ] **Step 2: `src/ui/controls.rs`**

```rust
//! Shared building blocks: text roles, panel headers, sections, buttons.

use makepad_widgets::*;

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    mod.widgets.RecoTitle = Label{
        draw_text +: {color: theme.color_text text_style: theme.font_bold{font_size: 11.0}}
    }
    mod.widgets.RecoBody = Label{
        draw_text +: {color: theme.color_text text_style: theme.font_regular{font_size: 10.0}}
    }
    mod.widgets.RecoMuted = Label{
        draw_text +: {color: theme.reco_text_muted text_style: theme.font_regular{font_size: 9.5}}
    }
    mod.widgets.RecoSectionTitle = Label{
        draw_text +: {color: theme.reco_text_secondary text_style: theme.font_bold{font_size: 8.5}}
    }
    mod.widgets.RecoPanelHeader = Label{
        margin: Inset{left: 4 top: 2 bottom: 2}
        draw_text +: {color: theme.color_text text_style: theme.font_bold{font_size: 12.0}}
    }
    mod.widgets.RecoSection = RoundedView{
        width: Fill height: Fit flow: Down spacing: theme.reco_gap
        padding: Inset{left: 12 right: 12 top: 10 bottom: 12}
        show_bg: true new_batch: true
        draw_bg +: {color: theme.reco_surface border_radius: theme.container_corner_radius}
    }
    mod.widgets.RecoIconButton = ButtonIcon{
        width: 30 height: 30 padding: 0 margin: 0 text: ""
        icon_walk: Walk{width: 16 height: 16}
        draw_icon +: {color: theme.reco_text_secondary}
    }
    mod.widgets.RecoButton = Button{height: 28}
    mod.widgets.RecoPrimaryButton = ButtonPrimary{
        height: 30 padding: Inset{left: 14 right: 14}
    }
}
```

- [ ] **Step 3: `src/ui/top_bar.rs`**

```rust
//! The top bar, drawn inside the window's title bar.

use makepad_widgets::*;

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    mod.widgets.RecoTopBar = View{
        width: Fill height: Fill flow: Right spacing: 4
        align: Align{y: 0.5}
        // Room for the macOS window buttons.
        padding: Inset{left: 80 right: 10}
        Tip{text: "Media panel (⌘1)"
            toggle_media := RecoIconButton{draw_icon +: {svg: crate_resource("self:resources/icons/sidebar_left.svg")}}
        }
        View{
            width: Fit height: Fit flow: Right spacing: 10 align: Align{y: 0.5}
            margin: Inset{left: 6}
            app_name := RecoTitle{text: "Reco"}
            project_name := RecoMuted{text: "No videos loaded"}
        }
        View{width: Fill height: 1}
        Tip{text: "Keyboard shortcuts"
            help_button := RecoIconButton{draw_icon +: {svg: crate_resource("self:resources/icons/help.svg")}}
        }
        Tip{text: "Preferences"
            prefs_button := RecoIconButton{draw_icon +: {svg: crate_resource("self:resources/icons/gear.svg")}}
        }
        export_button := RecoPrimaryButton{
            text: "Export"
            margin: Inset{left: 6 right: 6}
            icon_walk: Walk{width: 14 height: 14}
            draw_icon +: {svg: crate_resource("self:resources/icons/export.svg")}
        }
        Tip{text: "Inspector (⌘2)"
            toggle_inspector := RecoIconButton{draw_icon +: {svg: crate_resource("self:resources/icons/sidebar_right.svg")}}
        }
    }
}
```

- [ ] **Step 4: `src/ui/media_panel.rs`**

```rust
//! The Media sidebar: left and right camera videos, and calibration.
//! Module 0 shows its layout only; Module 3 wires it.

use makepad_widgets::*;

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    let AddVideos = RecoButton{
        text: "Add videos…"
        icon_walk: Walk{width: 12 height: 12}
        draw_icon +: {svg: crate_resource("self:resources/icons/plus.svg")}
    }

    mod.widgets.RecoMediaPanel = SolidView{
        width: Fill height: Fill flow: Down spacing: theme.reco_gap
        padding: Inset{left: 10 right: 6 top: 10 bottom: 10}
        draw_bg.color: theme.reco_panel
        RecoPanelHeader{text: "Media"}
        left_camera := RecoSection{
            RecoSectionTitle{text: "LEFT CAMERA"}
            left_empty := RecoMuted{text: "No video yet"}
            add_left := AddVideos{}
        }
        right_camera := RecoSection{
            RecoSectionTitle{text: "RIGHT CAMERA"}
            right_empty := RecoMuted{text: "No video yet"}
            add_right := AddVideos{}
        }
        calibration := RecoSection{
            RecoSectionTitle{text: "CALIBRATION"}
            calibration_status := RecoMuted{text: "Add both cameras to calibrate"}
            View{
                width: Fill height: Fit flow: Right spacing: 6
                auto_calibrate := RecoPrimaryButton{text: "Auto-calibrate"}
                load_calibration := RecoButton{text: "Load…"}
            }
        }
        View{width: Fill height: Fill}
        recent_button := ButtonFlat{
            text: "Recent files…"
            icon_walk: Walk{width: 12 height: 12}
            draw_icon +: {svg: crate_resource("self:resources/icons/folder.svg")}
        }
    }
}
```

- [ ] **Step 5: `src/ui/viewer.rs`**

```rust
//! The viewer: the preview canvas with the empty state. Module 1 draws the
//! stitched preview into it.

use makepad_widgets::*;

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    let Step = View{
        width: Fit height: Fit flow: Right spacing: 10 align: Align{y: 0.5}
        badge := RoundedView{
            width: 22 height: 22 align: Align{x: 0.5 y: 0.5}
            show_bg: true new_batch: true
            draw_bg +: {color: theme.color_primary_container border_radius: 11.0}
            number := Label{
                text: "1"
                draw_text +: {color: theme.color_on_primary_container text_style: theme.font_bold{font_size: 9.0}}
            }
        }
        label := RecoBody{text: ""}
    }

    mod.widgets.RecoViewer = View{
        width: Fill height: Fill flow: Overlay
        padding: 10
        canvas := RoundedView{
            width: Fill height: Fill flow: Overlay
            align: Align{x: 0.5 y: 0.5}
            show_bg: true new_batch: true
            draw_bg +: {color: theme.reco_viewer border_radius: theme.container_corner_radius}
            sample_frame := RoundedView{
                visible: false
                width: Fill height: Fill
                show_bg: true
                draw_bg +: {
                    border_radius: theme.container_corner_radius
                    pixel: fn() {
                        let sdf = Sdf2d.viewport(self.pos * self.rect_size)
                        sdf.box(0.0, 0.0, self.rect_size.x, self.rect_size.y, self.border_radius)
                        let top = vec4(0.11, 0.33, 0.22, 1.0)
                        let bottom = vec4(0.05, 0.19, 0.12, 1.0)
                        sdf.fill(mix(top, bottom, self.pos.y))
                        return sdf.result
                    }
                }
            }
            empty_state := View{
                width: Fit height: Fit flow: Down spacing: 16
                align: Align{x: 0.5 y: 0.5}
                Label{
                    text: "Stitch two camera videos"
                    draw_text +: {color: theme.color_text text_style: theme.font_bold{font_size: 16.0}}
                }
                View{
                    width: Fit height: Fit flow: Down spacing: 10
                    Step{badge.number.text: "1" label.text: "Add the left and right camera videos"}
                    Step{badge.number.text: "2" label.text: "Auto-calibrate, or load a calibration"}
                    Step{badge.number.text: "3" label.text: "Preview, adjust, then export"}
                }
                View{
                    width: Fit height: Fit flow: Right spacing: 8
                    empty_add := RecoPrimaryButton{text: "Add videos…"}
                    empty_recent := RecoButton{text: "Open recent…"}
                }
                empty_status := RecoMuted{text: ""}
            }
        }
    }
}
```

- [ ] **Step 6: `src/ui/inspector.rs`**

```rust
//! The Inspector: view, stitching, lens and stats. Module 0 shows its layout
//! only; Modules 4 and 5 wire it.

use makepad_widgets::*;

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    mod.widgets.RecoInspector = SolidView{
        width: Fill height: Fill flow: Down
        draw_bg.color: theme.reco_panel
        ScrollYView{
            width: Fill height: Fill flow: Down spacing: theme.reco_gap
            padding: Inset{left: 6 right: 10 top: 10 bottom: 10}
            RecoPanelHeader{text: "Adjust"}
            RecoSection{
                RecoSectionTitle{text: "VIEW"}
                fov_slider := Slider{text: "Field of view" min: 20.0 max: 150.0 default: 75.0 precision: 0}
                constrained_look := CheckBox{text: "Constrained look"}
                reset_view := RecoButton{text: "Reset view"}
            }
            RecoSection{
                RecoSectionTitle{text: "STITCHING"}
                seam_blend := Slider{text: "Seam blend" min: 0.0 max: 0.3 default: 0.05 precision: 2}
                match_colours := CheckBox{text: "Match colours" animator +: {active: {default: @on}}}
                rig_tilt := Slider{text: "Rig tilt (°)" min: -30.0 max: 30.0 default: 0.0 precision: 1}
                rig_roll := Slider{text: "Rig roll (°)" min: -15.0 max: 15.0 default: 0.0 precision: 1}
            }
            RecoSection{
                RecoSectionTitle{text: "LENS"}
                RecoMuted{text: "Auto-calibrate to detect the lens"}
            }
            RecoSection{
                RecoSectionTitle{text: "STATS"}
                RecoMuted{text: "No playback yet"}
            }
        }
    }
}
```

- [ ] **Step 7: `src/ui/transport.rs`**

```rust
//! The transport bar. Module 0 shows its layout only; Module 2 wires it.

use makepad_widgets::*;

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    let Time = Label{
        width: 56
        draw_text +: {color: theme.reco_text_secondary text_style: theme.font_code{font_size: 9.5}}
    }

    mod.widgets.RecoTransport = SolidView{
        width: Fill height: theme.reco_transport_height flow: Right spacing: 6
        align: Align{y: 0.5}
        padding: Inset{left: 12 right: 12}
        draw_bg.color: theme.reco_panel
        step_back := RecoIconButton{draw_icon +: {svg: crate_resource("self:resources/icons/step_back.svg")}}
        play_pause := RecoIconButton{
            width: 36 height: 36
            icon_walk: Walk{width: 18 height: 18}
            draw_icon +: {svg: crate_resource("self:resources/icons/play.svg") color: theme.color_text}
        }
        step_forward := RecoIconButton{draw_icon +: {svg: crate_resource("self:resources/icons/step_forward.svg")}}
        time_current := Time{text: "0:00"}
        timeline := SliderMinimal{width: Fill text: "" min: 0.0 max: 1.0 default: 0.0}
        time_total := Time{text: "0:00"}
        record_button := RecoButton{
            text: "Rec"
            icon_walk: Walk{width: 10 height: 10}
            draw_icon +: {svg: crate_resource("self:resources/icons/record.svg") color: theme.reco_record}
        }
        aspect := DropDown{width: 84 labels: ["Auto" "16:9" "4:3" "21:9"]}
    }
}
```

- [ ] **Step 8: `src/ui/status_bar.rs`**

```rust
//! The status bar: status line, version and Report bug.

use makepad_widgets::*;

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    mod.widgets.RecoStatusBar = SolidView{
        width: Fill height: theme.reco_statusbar_height flow: Right spacing: 12
        align: Align{y: 0.5}
        padding: Inset{left: 12 right: 8}
        draw_bg.color: theme.color_bg_app
        status_text := RecoMuted{width: Fill text: "Ready"}
        version_text := RecoMuted{text: ""}
        report_bug := ButtonFlatter{text: "Report bug"}
    }
}
```

- [ ] **Step 9: `src/ui/shell.rs`**

```rust
//! The shell: Media | viewer | Inspector between splitters, then the
//! transport bar and the status bar.

use makepad_widgets::*;

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    mod.widgets.RecoShell = View{
        width: Fill height: Fill flow: Down
        main_split := Splitter{
            axis: SplitterAxis.Horizontal
            align: SplitterAlign.FromA(260.0)
            min_vertical: 180.0 max_vertical: 360.0
            min_horizontal: 180.0 max_horizontal: 360.0
            a: View{width: Fill height: Fill media_panel := RecoMediaPanel{}}
            b: View{
                width: Fill height: Fill
                inner_split := Splitter{
                    axis: SplitterAxis.Horizontal
                    align: SplitterAlign.FromB(280.0)
                    min_vertical: 360.0 max_vertical: 200.0
                    min_horizontal: 360.0 max_horizontal: 200.0
                    a: View{width: Fill height: Fill viewer := RecoViewer{}}
                    b: View{width: Fill height: Fill inspector := RecoInspector{}}
                }
            }
        }
        transport := RecoTransport{}
        status_bar := RecoStatusBar{}
    }
}
```

- [ ] **Step 10: Wire it in `main.rs`**

Replace `main.rs` with:

```rust
//! Reco Desktop: the Makepad 2 desktop app for Reco.
//!
//! Module 0 (see `DESIGN.md`): the window shell and the look. Nothing is
//! wired to the engine yet; `--look-preview` shows the loaded-state layout
//! with sample content for design review.

pub use makepad_widgets;
use makepad_widgets::*;

mod cli;
mod shell_state;
mod theme;
mod ui;

use shell_state::{Panel, ShellState};

app_main!(App);

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    startup() do #(App::script_component(vm)){
        ui: Root{
            main_window := Window{
                window.title: "Reco"
                window.inner_size: vec2(1280, 820)
                caption_bar +: {
                    caption_label +: {
                        flow: Right spacing: 0 align: Align{y: 0.5}
                        caption_icon +: {width: 0 height: 0 margin: 0}
                        label +: {visible: false}
                        top_bar := RecoTopBar{}
                    }
                }
                window_menu +: {
                    main := MenuItem.Main{items: [@app_menu, @view_menu]}
                    app_menu := MenuItem.Sub{name: "Reco" items: [@quit]}
                    quit := MenuItem.Item{name: "Quit Reco" key: KeyCode.KeyQ enabled: true}
                    view_menu := MenuItem.Sub{name: "View" items: [@toggle_media_menu, @toggle_inspector_menu]}
                    toggle_media_menu := MenuItem.Item{name: "Media Panel" key: KeyCode.Key1 enabled: true}
                    toggle_inspector_menu := MenuItem.Item{name: "Inspector" key: KeyCode.Key2 enabled: true}
                }
                body +: {
                    flow: Overlay
                    shell := RecoShell{}
                    tip_layer := TipLayer{}
                }
            }
        }
    }
}

/// The application: widget tree, shell state and startup options.
#[derive(Script, ScriptHook)]
pub struct App {
    #[live]
    ui: WidgetRef,
    #[rust]
    shell: ShellState,
    #[rust]
    args: cli::Args,
}

impl App {
    /// Push the shell state into the widgets: panel folds, enabled
    /// controls, empty state versus sample frame.
    fn apply_shell(&mut self, cx: &mut Cx) {
        let media = if self.shell.is_open(Panel::Media) {
            SplitterCollapse::None
        } else {
            SplitterCollapse::A
        };
        self.ui.splitter(cx, ids!(main_split)).set_collapse(cx, media);
        let inspector = if self.shell.is_open(Panel::Inspector) {
            SplitterCollapse::None
        } else {
            SplitterCollapse::B
        };
        self.ui.splitter(cx, ids!(inner_split)).set_collapse(cx, inspector);

        let loaded = self.shell.files_loaded();
        self.ui.button(cx, ids!(export_button)).set_enabled(cx, loaded);
        self.ui
            .button(cx, ids!(toggle_inspector))
            .set_enabled(cx, self.shell.can_toggle(Panel::Inspector));
        for id in [ids!(step_back), ids!(play_pause), ids!(step_forward), ids!(record_button)] {
            self.ui.button(cx, id).set_enabled(cx, loaded);
        }
        self.ui.widget(cx, ids!(timeline)).set_disabled(cx, !loaded);
        self.ui.widget(cx, ids!(aspect)).set_disabled(cx, !loaded);
        self.ui.view(cx, ids!(empty_state)).set_visible(cx, !loaded);
        self.ui.view(cx, ids!(sample_frame)).set_visible(cx, loaded);
        self.ui.redraw(cx);
    }

    fn toggle(&mut self, cx: &mut Cx, panel: Panel) {
        if self.shell.toggle(panel) {
            self.apply_shell(cx);
        }
    }

    /// `--look-preview`: the loaded-state layout with sample texts.
    fn show_look_preview(&mut self, cx: &mut Cx) {
        self.shell.set_files_loaded(true);
        self.ui.label(cx, ids!(project_name)).set_text(cx, "GX010120.MP4 + GX010092.MP4");
        self.ui.label(cx, ids!(time_current)).set_text(cx, "12:34");
        self.ui.label(cx, ids!(time_total)).set_text(cx, "1:45:00");
        self.ui.label(cx, ids!(status_text)).set_text(cx, "Ready - 59.9 fps");
    }
}

impl MatchEvent for App {
    fn handle_startup(&mut self, cx: &mut Cx) {
        match cli::Args::parse(std::env::args().skip(1)) {
            Ok(args) => self.args = args,
            Err(err) => log!("ignoring command line: {err}"),
        }
        self.ui
            .label(cx, ids!(version_text))
            .set_text(cx, concat!("v", env!("CARGO_PKG_VERSION")));
        if let Some((w, h)) = self.args.window_size {
            self.ui.window(cx, ids!(main_window)).resize(cx, dvec2(w, h));
        }
        if self.args.look_preview {
            self.show_look_preview(cx);
        }
        self.apply_shell(cx);
    }

    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions) {
        if self.ui.button(cx, ids!(toggle_media)).clicked(actions) {
            self.toggle(cx, Panel::Media);
        }
        if self.ui.button(cx, ids!(toggle_inspector)).clicked(actions) {
            self.toggle(cx, Panel::Inspector);
        }
    }
}

impl AppMain for App {
    fn script_mod(vm: &mut ScriptVm) -> ScriptValue {
        makepad_widgets::theme_mod(vm);
        crate::theme::script_mod(vm);
        makepad_widgets::widgets_mod(vm);
        crate::ui::script_mod(vm);
        self::script_mod(vm)
    }

    fn handle_event(&mut self, cx: &mut Cx, event: &Event) {
        match event {
            Event::WindowGeomChange(ge) => {
                self.shell.fit_width(ge.new_geom.inner_size.x);
                self.apply_shell(cx);
            }
            Event::MacosMenuCommand(item) if *item == live_id!(toggle_media_menu) => {
                self.toggle(cx, Panel::Media);
            }
            Event::MacosMenuCommand(item) if *item == live_id!(toggle_inspector_menu) => {
                self.toggle(cx, Panel::Inspector);
            }
            _ => {}
        }
        self.match_event(cx, event);
        self.ui.handle_event(cx, event, &mut Scope::empty());
    }
}
```

Remove the `#![allow(dead_code)]` lines added in Task 3.

- [ ] **Step 11: Build, then fix any compile or log errors**

Run: `cargo build --profile desktop -p reco-desktop`, then launch once with `--look-preview` through `drive.py` and print `app.errors()`.

Expected:
- It builds, and `errors()` is `[]`.
- If an id lookup type differs (for example `SliderMinimal` is not a `Slider` for `set_disabled`) or a Splash property is rejected, change it to the nearest working form from Makepad's sources and record each change under "Deviations".

- [ ] **Step 12: Commit**

```bash
git add crates/reco-desktop/src
git commit -m "feat: reco-desktop window shell (top bar, media, viewer, inspector, transport, status)"
```

---

### Task 6: Module 0 check, screenshots and parity

**Files:**
- Create: `crates/reco-desktop/tools/check_m0.py`
- Modify: `crates/reco-desktop/PARITY.md` (tick Module 0 items with evidence)

**Interfaces:**
- Consumes: `drive.App`, `drive.close_to`, and the widget ids from Task 5.

- [ ] **Step 1: Write `tools/check_m0.py`**

```python
#!/usr/bin/env python3
"""Module 0 check: shell and look (PARITY.md, Module 0).

Run after `cargo build --profile desktop -p reco-desktop`. Saves screenshots
to target/desktop-checks/m0/ and exits non-zero at the first failure.
"""
import os
import sys

import drive

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "..", ".."))
BIN = os.path.join(ROOT, "target", "desktop", "reco-desktop")
OUT = os.path.join(ROOT, "target", "desktop-checks", "m0")

APP_BG = "#0f1115"
PANEL_BG = "#14171c"
ACCENT = "#34d399"


def expect(ok, message):
    if not ok:
        print(f"FAIL: {message}")
        sys.exit(1)
    print(f"ok: {message}")


def visible(app, widget_id):
    return app.rect(widget_id) is not None


def launch(size, look_preview):
    args = ["--window-size", f"{size[0]}x{size[1]}"]
    if look_preview:
        args.append("--look-preview")
    return drive.App.launch(BIN, args)


def check_state(size, look_preview):
    name = f"{'loaded' if look_preview else 'empty'}-{size[0]}x{size[1]}"
    with launch(size, look_preview) as app:
        info = app.get("/s")["w"][0]
        expect(info["t"].startswith("Reco"), f"{name}: window title is Reco ({info['t']!r})")
        expect(abs(info["sz"][0] - size[0]) <= 2 and abs(info["sz"][1] - size[1]) <= 2,
               f"{name}: window size {info['sz']} matches {size}")
        for wid in ("toggle_media", "export_button", "toggle_inspector", "status_text",
                    "version_text", "report_bug", "play_pause", "timeline"):
            expect(visible(app, wid), f"{name}: `{wid}` is on screen")
        expect(visible(app, "media_panel"), f"{name}: Media panel open")
        if look_preview:
            expect(visible(app, "sample_frame"), f"{name}: sample frame shown")
            expect(not visible(app, "empty_state"), f"{name}: empty state hidden")
            inspector_expected = size[0] >= 960
            expect(visible(app, "inspector") == inspector_expected,
                   f"{name}: Inspector {'open' if inspector_expected else 'folded'}")
        else:
            expect(visible(app, "empty_state"), f"{name}: empty state shown")
            expect(not visible(app, "inspector"), f"{name}: Inspector closed before load")
        png = app.grab(os.path.join(OUT, f"{name}.png"))
        corner = png.pixel(png.width - 3, png.height - 3)
        expect(drive.close_to(corner, APP_BG, tol=4), f"{name}: status bar background {corner[:3]} is {APP_BG}")
        ex, ey, ew, eh = app.rect("export_button")
        scale = png.width / info["sz"][0]
        centre_left = png.pixel(int((ex + 6) * scale), int((ey + eh / 2) * scale))
        expect(drive.close_to(centre_left, ACCENT, tol=40) == look_preview,
               f"{name}: Export is {'enabled (accent)' if look_preview else 'disabled'} ({centre_left[:3]})")
        expect(app.errors() == [], f"{name}: no errors in the app log")


def check_toggles():
    with launch((1280, 820), True) as app:
        app.click_id("toggle_media")
        expect(not visible(app, "media_panel"), "toggle: Media folds")
        app.grab(os.path.join(OUT, "loaded-media-folded.png"))
        app.click_id("toggle_media")
        expect(visible(app, "media_panel"), "toggle: Media reopens")
        app.click_id("toggle_inspector")
        expect(not visible(app, "inspector"), "toggle: Inspector folds")
        app.click_id("toggle_inspector")
        expect(visible(app, "inspector"), "toggle: Inspector reopens")
        mx, my, mw, mh = app.rect("media_panel")
        bar_x, bar_y = mx + mw + 3, my + mh / 2
        app.get("/m", k="down", x=bar_x, y=bar_y)
        app.get("/m", k="move", x=bar_x + 60, y=bar_y)
        app.get("/m", k="up", x=bar_x + 60, y=bar_y, wait=1)
        expect(app.rect("media_panel")[2] > mw + 40, "resize: dragging the bar widens Media")
        app.grab(os.path.join(OUT, "loaded-media-widened.png"))
        expect(app.errors() == [], "toggle: no errors in the app log")
    with launch((1280, 820), False) as app:
        app.click_id("toggle_inspector")
        expect(not visible(app, "inspector"), "toggle: Inspector stays closed before load")


def main():
    os.makedirs(OUT, exist_ok=True)
    for size in ((720, 600), (1280, 820), (1920, 1200)):
        check_state(size, look_preview=False)
        check_state(size, look_preview=True)
    check_toggles()
    print(f"Module 0 check passed. Screenshots: {OUT}")


if __name__ == "__main__":
    main()
```

- [ ] **Step 2: Run it**

Run: `python3 crates/reco-desktop/tools/check_m0.py`
Expected: every line is `ok:`, ending with "Module 0 check passed". Fix the app (never the expectation) for each failure, unless the expectation contradicts PARITY.md.

- [ ] **Step 3: Look at every screenshot**

Open each PNG in `target/desktop-checks/m0/`. Check:
- alignment and spacing;
- text that does not clip;
- consistent radii;
- legible contrast;
- nothing overlapping at 720×600.

Fix anything off in `theme.rs` or `src/ui/*` and rerun Step 2.

- [ ] **Step 4: Tick PARITY.md Module 0**

For each item proven, change `- [ ]` to `- [x]` and append the evidence. For example: `check_m0.py: "loaded-1280x820: Inspector open"; target/desktop-checks/m0/loaded-1280x820.png`.

Change the window item to: `Window: title "Reco", default 1280×820; usable to 720×600 by folding panels (Makepad has no minimum-size API).`

Change the theme item's last sentence to: `Dark only; light tokens arrive with the dark-mode preference (Module 7).`

Leave "Owner approved the look" unticked.

- [ ] **Step 5: Final verification**

Run:
```bash
cargo fmt -p reco-desktop -- --check && cargo clippy -p reco-desktop --all-targets -- -D warnings && cargo test -p reco-desktop && (cd crates/reco-desktop/tools && python3 -m unittest test_drive) && python3 crates/reco-desktop/tools/check_m0.py
```
Expected: all clean and passing.

- [ ] **Step 6: Commit**

```bash
git add crates/reco-desktop/tools/check_m0.py crates/reco-desktop/PARITY.md crates/reco-desktop/plans
git commit -m "test: Module 0 check for reco-desktop (shell and look)"
```

- [ ] **Step 7: Owner review**

Send the owner:
- `empty-1280x820.png`
- `loaded-1280x820.png`
- `loaded-720x600.png`
- `loaded-1920x1200.png`
- `loaded-media-folded.png`

Ask them to approve the look or say what to change. Module 1 starts only after approval (DESIGN.md, Module 0 "done when").

---

## Deviations

Changes made during execution where Makepad's actual API differed from this plan (one line each):

- T1/T2: `/s` lists no window right after the control port opens, so `App.launch` now waits for the first window (`_wait_for_window`).
- T2: `drive.py` imports `urllib.error` explicitly. `test_drive.py` starts with its docstring (the plan's path comment was dropped).
- T4: the THIRD_PARTY_NOTICES entry follows the file's bold-name paragraph style. The theme pixel test is kept as `tools/check_theme.py`.
- T5: the `Script` derive cannot parse a path type (`cli::Args`), so `App` uses `use cli::Args` and the bare type.
- T5: the app script needs `use mod.draw.KeyCode` for the menu shortcuts.
- T5: `Button::set_enabled` only gates input; the dimmed look is the animator's `disabled` state.
  - `App::set_button_enabled` sets both.
  - Controls that start disabled declare `animator +: {disabled: {default: @on}}`, so they do not flash at startup.
- T5: `check_m0.py` was written and run red before the shell existed. It reads the Export colour only once it stops changing (`settled_pixel`).
- T6 review fixes:
  - The Media panel scrolls (`ScrollYView`; Recent files sits under Calibration).
  - Disabled buttons fade their icon too (theme `reco_disabled_icon_opacity` 0.35).
  - The timeline hides its value field.
  - `settings.svg` (sliders) replaces Makepad's sun-like `gear.svg`.
  - Report bug uses the secondary text colour.
  - Auto-calibrate starts disabled.
  - Look-preview fills the Media panel with sample names.
- T6: the Inspector floor is 206 pt, because the B-side floor includes the 6 pt splitter bar. Minimum-width drag checks were added to `check_m0.py`.
