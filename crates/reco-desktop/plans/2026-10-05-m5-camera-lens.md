# Module 5: Camera and Lens (compact plan)

> Run inline, as Modules 3 and 4 (owner, 2026-10-05: speed up; compact
> plan, no review pause; TDD, checks and a self-review stay).

**Goal:** the Adjust panel's View and Lens sections work:
- the field of view, "stay inside" (constrained look) and Reset view;
- lens info for each camera;
- lens correction on or off;
- a single-camera lens preview (left or right);
- fine-tuning each camera's lens (fx, fy, cx, cy, k1–k4) with Reset lens;
- the lens picker: search the profile database off the UI thread, apply to
  both, left or right, or load from a file;
- a Stats section fed while previewing.

**Spec:** DESIGN.md row 5; PARITY.md Module 5; slint-inventory §2.4 (Camera,
Lens, Fine-tune, Stats) and §2.12 (lens picker); reco-gui `main.rs`
lens handlers (`on_changed_lens_param`, `on_reset_lens`, the lens picker,
`on_changed_lens_preview`, `on_changed_lens_correction`) and `preview.rs`
`render_lens_preview`.

## Decisions

- **Ownership.** The worker owns the lens, as it owns the stitch.
  `Tune` gains `Fov`, `Constrained`, `LensCorrection`, `Camera { side,
  params }`, `ResetLens` and `Profile { side, params, name }`.
  `CalibrationValues` gains the field of view, constrained look, lens
  correction, each camera's params and profile name, and the loaded params
  (for slider ranges). Save folds the lens in (the pipeline's calibration
  already holds the params).
- **Lens preview.** Reco's `LensPreviewRenderer` draws the chosen camera
  flat, with or without correction, into a new texture at the input's size
  each frame; the worker copies it, fitted and centred, into the ring slot
  (or the readback) with a small blit pass. Panning is off while it shows;
  the side and the mode are commands. FRICTION: the per-frame input-size
  texture.
- **Lens picker.** It is a panel over the viewer, not a modal window: a
  search field, results (camera · lens · size), "Apply to" (Both, Left,
  Right), "Load from file…" and Close. The search runs on a short thread;
  the newest query wins.
- **Fine-tune ranges** are the Slint app's (`set_lens_sliders`): fx and fy
  within ±15% of the larger loaded focal length (at least 5 px), cx and cy
  within ±10% of the image width and height (at least 5 px), k1–k4 within
  ±0.3. Ranges come from the left camera, as there. (Corrected from the
  first draft, which guessed ±50% and ±0.5.)
- **Lens info.** Each camera's camera, lens and source: from a
  calibration run (`CalibrationResult::{left,right}_lens_profile`), from
  the picker ("Picker") or a file ("File"), and for a loaded calibration
  detected from the video's telemetry (`detect_profile`) on a short
  thread when the preview opens (New: the Slint app showed lens info only
  after calibrating in the same session).
- **Lens correction** is saved with the calibration
  (`lens_correction_amount`), on or off as in the Slint app.
- **Stats.** Once a second the worker reports fps, the average and p99
  frame time, decode, render and readback times, frames dropped and the
  GPU. The Stats section shows them, and the calibration's confidence and
  matches when known.

## Tasks

1. **reco-app lens tuning.** The new `Tuning` variants, the values, saving,
   the lens-preview render path and the stats event. Tests:
   - each lens change reaches the renderer and marks it changed;
   - Reset lens restores the loaded params;
   - a picked profile is scaled to the input size;
   - the lens preview renders a different picture from the stitch, and the
     side matters;
   - stats arrive while playing.
2. **Desktop: View and Lens.** The FOV slider, stay-inside and Reset view;
   lens info; correction; the lens preview with Left/Right; fine-tune with
   Left/Right/Both; Reset lens. check_m5 `lens`.
3. **Lens picker.** The overlay, the background search, Apply to, Load from
   file (dialog seam), Close. check `picker`.
4. **Stats** section. check `stats`.
5. **Final.** check_m5; check_m0–m4; PARITY, DESIGN and FRICTION;
   self-review; finishing.
