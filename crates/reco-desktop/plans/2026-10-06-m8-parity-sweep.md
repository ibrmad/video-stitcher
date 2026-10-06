# Module 8: parity sweep (compact plan)

> Run inline (owner: compact plan, no review pause; TDD, checks and a
> self-review stay). Branch `feat/desktop-m8-parity`.

**Goal (DESIGN.md, Modules):** every PARITY.md item ticked with evidence,
a polish and performance pass, then the Slint app retired with the
owner's sign-off.

## Tasks

1. **Inventory sweep.** Walk `docs/slint-inventory.md` §1–§9 (and the
   DESIGN.md "Slint issues to fix, not copy" list) against PARITY.md and
   the app. Every Slint behaviour either has a ticked PARITY line with
   evidence, or gets built and checked now, or gets a ruling the owner
   sees at sign-off. Result goes in PARITY.md's Module 8 section.
2. **Open items.** Tick "Export progress, status text and Cancel; Show
   in folder after an export" with check_m6 evidence. The "ROI points
   over the lens preview" ruling goes to the owner at sign-off.
3. **Performance pass** (`tools/check_m8.py perf`), on the alfheim pair
   and the 5.3K pair, against DESIGN.md Rule 8:
   - first window frame under 1 s after start (a `--perf-log` line);
   - zero-copy preview in use (no CPU copies);
   - playing: the picture keeps the source rate; panning: the picture
     re-renders every frame; UI draw under 4 ms a frame;
   - about 0% CPU while paused.
   Numbers go in PARITY.md.
4. **Polish pass.** One full suite run (theme, check_m0–m8), one look at
   every screenshot, the findings fixed in one batch, the affected checks
   rerun.
5. **Sign-off and retirement.** Screenshots, parity summary, rulings and
   numbers to the owner. On sign-off only: one commit removes
   `crates/reco-gui` and moves the workspace, `deny.toml`, the CI and
   release workflows and the docs to reco-desktop. (The checkout holds
   another session's uncommitted edits to `crates/reco-gui/ui/main.slint`:
   the owner decides what happens to them first.)

## Sweep results (2026-10-06)

An agent walked the inventory (about 450 entries); I checked each finding.
Owner's choices: port the export figures in Stats, keys by typed
character and the benchmark auto-export; ask before quitting with unsaved
edits; the ROI-over-lens-preview ruling stands; no Slint settings import;
the other session's `main.slint` edits go with the Slint app.

Fixes, each with a test or check that fails first:

1. Shortcuts never fire behind a sheet; Escape closes every sheet (bug
   report, lens picker too). DESIGN Rule 9.
2. The export sheet keeps choices made and not exported when it closes.
   Rule 9.
3. Closing or ⌘Q with unsaved calibration edits asks: Save, Don't Save,
   Cancel (the pasted outline is one of them).
4. Recalibrate keeps the lens in use (a picked profile, unsaved edits),
   not the file's.
5. A log file (engine and app lines, cut at 2 MB, `RUST_LOG`), panics
   written to it; the bug report attaches its tail.
6. Usage data and the bug report name the real AI capability and the GPU
   backend; `decoder` means what Slint sent.
7. Reset layout only when the layout changed.
8. The lens preview opens Fine-tune.
9. An export says where it starts while it seeks.
10. F/F11 toggles maximize on Windows and Linux (Makepad has no
    fullscreen there: FRICTION).
11. No console window for Windows release builds.
12. Detector features as reco-gui: `load-dynamic`, `cuda`, `tensorrt`,
    `ncnn`, `directml` (Windows).
13. Export figures in Stats (owner).
14. Keys by typed character (owner).
15. Benchmark auto-export (owner): `RECO_AUTOEXPORT`, `_MODEL`,
    `_LOOKAHEAD`, `_REPEAT`, `RECO_VRAM_BUDGET_GB`.
16. PARITY bookkeeping: rulings (accessibility, colour match not
    remembered, ROI over the lens preview, no settings import, the paused
    preview's VRAM during an export), current panel widths, the
    off-UI-thread decoder line.

Retirement (with the sign-off): the Linux build (OrbStack, done in
Module 8), resources packaged beside the binary, the CI and release
workflows moved over (their Linux packages grow by X11, GLX, xkbcommon,
PulseAudio, ALSA, gbm, drm), Windows proven in CI.
