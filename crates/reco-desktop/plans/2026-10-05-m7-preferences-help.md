# Module 7: Preferences and help (compact plan)

> Run inline, as Modules 3–6 (owner, 2026-10-05: speed up; compact plan,
> no review pause; TDD, checks and a self-review stay). Main holds Modules
> 0–6 (6 without AI tracking, which is 6b), merged locally, not pushed.

**Goal:** the app menu's commands work:
- Preferences (a sheet): export defaults (codec, quality), the default seam
  blend for new calibrations, the AI model path with Browse, recording
  quality and folder with Browse, the telemetry opt-in; Save keeps them,
  Cancel (or Escape) reverts everything.
- Keyboard shortcuts (a sheet) listing exactly the keys the app handles,
  with Website and Forum.
- Report a bug (a sheet): what went wrong, contact, include system info
  and logs; Send when telemetry is on, Copy report always.
- Version in the app menu; an update check that shows a toast with a
  Download button (it never opens the browser by itself).
- Settings persistence: window size, maximized state and panel widths are
  restored (New), with everything already kept (recent sessions, export
  and recording defaults, preview aspect).

**Spec:** DESIGN.md row 7; PARITY.md Module 7; slint-inventory §2.8
(shortcuts), §2.9 (preferences), §2.10 (bug report), §8 (settings, URLs,
update check, clipboard); reco-gui `main.rs` (`on_save_prefs`,
`on_open_website`/`on_open_forum` (github.com/reco-project/video-stitcher,
forum.reco-project.org), `on_submit_bug_report`, the update thread on
api.github.com/repos/reco-project/video-stitcher/releases/latest) and
`telemetry_client.rs` (its endpoint and events).

## Decisions

- **Network.** Telemetry (opt-in, off by default), the update check and a
  sent bug report are the only network uses. Checks never touch the
  network: `RECO_DESKTOP_NO_NETWORK` turns every request into a log line
  ("would send …"), and `RECO_DESKTOP_FAKE_RELEASE=v9.9.9` stands in for
  the release lookup. Prefer Makepad's HTTP (`cx.http_request`, answered as
  `Event::NetworkResponses`) over new crates; if it can't, ureq is already
  in the lockfile (reco-gui) — ask the owner before adding any crate that
  isn't.
- **Bug report.** The report text (description, contact, and with the box
  ticked: version, OS, GPU, the open files' names, the app log's tail) is
  composed by a pure function in reco-app. Send goes to the telemetry
  endpoint only when telemetry is on (New: the Slint app sent it
  regardless); Copy report puts it on the clipboard only when clicked
  (New: the Slint app always copied it).
- **Dark mode.** Ruling: not offered. The app has one look, the Rerun dark
  look the owner chose; a light theme is a design project of its own.
  PARITY says so.
- **Preferences values** live in `desktop.json` (`settings.rs`, defaults
  for anything missing). The sheet works on a copy; Save writes it and
  applies it (recording quality in the view bar, export defaults in the
  next export sheet); Cancel drops the copy.
- **Default seam blend** is written into calibrations the app makes
  (Module 3's job), not into loaded ones.
- **Shortcuts** come from one table in `keys.rs` that the key handler and
  the sheet share; a test fails when a handled key has no row.
- **Window and panels.** Size and maximized state, and the Setup and Adjust
  panel widths, are saved when they change (after a quiet second) and
  restored at start; `--window-size` still wins for checks.
- **Dialogs** go through the dialog seam: keys `model` (an .onnx file) and
  `recording_folder` (a folder).

## Tasks

1. **reco-app.** Settings fields (default blend, AI model path, telemetry
   on and client id, window size and maximized, panel widths) with
   defaults and old-file tests; `bug_report::compose`; version comparison
   for the update check (pure); the telemetry events' JSON (pure).
2. **Preferences sheet** and the app menu wiring (Preferences…, Keyboard
   shortcuts, Report a bug…, the version). Save/Cancel; Browse for the
   model and the recording folder. check_m7 `prefs`.
3. **Shortcuts sheet** from the shared table; Website and Forum (no browser
   in checks). check `shortcuts`.
4. **Bug report sheet**: Send (telemetry on; no network in checks), Copy
   report. check `bug`.
5. **Update check** on a thread at start (network switch; fake release in
   checks) and its toast with Download. check `update`.
6. **Window and panel persistence.** check `persist` (a second launch
   restores them).
7. **Final.** check_m7; check_m0–m6; PARITY, DESIGN and FRICTION;
   self-review; finishing.
