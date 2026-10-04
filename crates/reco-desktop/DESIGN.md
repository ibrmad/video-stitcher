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

Preview bridge (`reco-desktop/src/preview/`, Module 1):

- A wgpu 28 device on the Metal backend. At startup the app checks that its
  `MTLDevice` is Makepad's (`cx.metal_device()`); if not, it uses the
  fallback below.
- A ring of three render targets. Reco renders into the next free slot. A slot
  is shown only after `queue.on_submitted_work_done` has fired for its
  submission, and it is reused only after a newer slot has been on screen.
- Portable fallback: `copy_texture_to_buffer` and `map_async` into a Makepad
  `VecBGRAu8_32` texture. It is slower but works on every desktop.

Threading, adopted from Makepad's own rules:

- The UI thread never blocks. It shares no `Mutex` or `RwLock` with workers,
  never waits on a channel, and never opens or seeks a decoder or does file I/O
  beyond small settings writes.
- Workers are long-lived and fed by bounded channels. Large payloads travel
  as `Arc`.

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

## Modules

| # | Module | Done when |
|---|---|---|
| 0 | Shell and look | The crate builds in the workspace on Rust 1.92. Theme tokens are in place. The window shell (top bar, Media sidebar, viewer with empty state, Inspector, transport bar, status bar) works, with panel toggles. The minimum, default and large window sizes look right. `tools/drive.py` exists and the Module 0 check passes. The owner approves the look. |
| 1 | Preview | `reco-app` exists with the session. Reco's renderer draws into the viewer through the zero-copy bridge. Pan, zoom, reset, keyboard nudges, preview aspect and playback ticking work. The fallback path builds. |
| 2 | Transport and status | Play, pause, step and seek on the timeline, with the export-range tint and time display. Record button with quality. Status line, version, toasts and Report bug link. |
| 3 | Files | Left and right segment lists (add, remove, reorder). Calibration: auto-calibrate with progress and Cancel, load and clear. Recent files. Drop videos onto the window. |
| 4 | Stitching and calibration | Seam blend, colour match, rig tilt and roll, sync offset, intersect, axis offset, `x_ty`, save and reset calibration, field ROI (paste and show). |
| 5 | Camera and lens | FOV, constrained look, reset view, lens info, lens correction, lens preview with side, fine-tune sliders, reset lens, lens picker dialog, stats. |
| 6 | Export | Output path, resolution, codec and quality, processing range, replay and debug options, AI tracking (model, mode, interval, preset, framing, pitch lock, lookahead with VRAM risk, advanced panner), progress and Cancel, show in folder. |
| 7 | Preferences and help | Preferences dialog, keyboard shortcuts dialog, bug report dialog, update check, settings persistence (including window size and panel state). |
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
