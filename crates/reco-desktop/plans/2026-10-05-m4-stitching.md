# Module 4: Stitching and Calibration (compact plan)

> Run inline, as Module 3 (owner, 2026-10-05: speed up; compact plan, no
> review pause; TDD, checks and a self-review stay).

**Goal:** the Adjust panel tunes the stitch live, and the tuned calibration
is saved:
- seam blend, colour match, rig tilt and roll;
- the sync offset (frames, Apply);
- the layout (intersect, camera axis offset, `x_ty`, Reset);
- Save calibration when anything changed;
- the field ROI: its status, the browser editor, and Paste ROI.

**Spec:** DESIGN.md row 4; PARITY.md Module 4; slint-inventory §2.2
(Stitching, Field ROI), §2.4 (Calibration), §5.2; reco-gui `main.rs`
`set_blend_width`…`set_rig_roll`, `apply_layout`, `save_calibration`,
`reset_calibration`, `on_launch_roi_editor`, `on_paste_roi`.

## Decisions

- **Ownership.** The worker owns the live calibration. One command,
  `PreviewCommand::Tune(Tuning)`, sets blend, colour match, tilt, roll or the
  layout. The sync offset gets its own command and reopens playback at the
  same frame.
- **Saving.** `SaveCalibration { path }` writes atomically on the worker. It
  folds in the live lens, blend and lens-correction values, as Slint did.
- **Dirty state.** After any change the worker reports
  `Calibration(CalibrationState { values, dirty })`. The panel shows the
  values, and "Save calibration" appears while `dirty`. Reset restores the
  layout loaded with the file.
- **Inputs follow the file.** The sliders show the calibration's own values
  after every open; Slint's sliders read 0 until touched.
- **Sync offset.** Entered in frames (a text field and Apply), bounded by the
  playable length; out-of-range values are refused with a toast.
- **Where things go.** Layout and sync sit in the Stitch section's Advanced
  tier. Save calibration goes in the Calibration section of the Setup panel,
  beside the file it writes.
- **Field ROI.**
  - The Setup panel says "Field outline: set" or "none".
  - "Edit in browser…" writes undistorted first frames and the editor page on
    a worker thread (reco-gui's page, reused), then opens it. A toast says
    when it is ready.
  - "Paste outline" reads the clipboard (Makepad's clipboard paste) or the
    text field and validates it as `FieldRoi` JSON.
  - The outline is kept in the calibration and saved with it. Drawing it over
    the lens preview is Module 5's.

## Tasks

1. **reco-app.** `Tuning`, `Tune`, `SetSyncOffset`, `SaveCalibration`,
   `SetFieldRoi`, and the `Calibration` event with dirty tracking. Tests:
   - each tuning reaches the renderer and marks dirty;
   - Reset restores the loaded layout;
   - a save writes JSON that reloads with the tuned values and clears dirty;
   - the sync offset moves playback and stays inside the length.
2. **Desktop wiring.** Sliders and Apply to commands; values from
   `Calibration` events; Save calibration. check_m4 `tune`:
   - blend changes the picture;
   - tilt and the layout move it;
   - Save writes and the button hides;
   - after a relaunch the values come back;
   - the sync offset moves the right camera.
3. **Field ROI.** The status, the editor export (thread), Paste outline
   (clipboard and field), and saving. check `roi`: pasting valid JSON sets
   the outline; invalid JSON says why; the editor folder is written (never
   opened in checks: a `RECO_DESKTOP_NO_BROWSER` seam).
4. **Final.** check_m4; check_m0–m3; PARITY, DESIGN and FRICTION; self-review;
   finishing.
