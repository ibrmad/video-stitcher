# Module 6b: AI tracking in the export (compact plan)

> Run inline, as Modules 3–7 (owner: speed up; compact plan, no review
> pause; TDD, checks and a self-review stay). Main holds Modules 0–7 and
> the roomier design. `reco-autocam` has another session's uncommitted,
> additive change (a private fn and a new `horizon_pitch` option); this
> module only calls its public API and never edits it.

**Goal:** the export sheet's AI tracking, as the Slint app's (PARITY
Module 6, the open item): Enable (when the machine can run the detector)
with a status line; the model with Choose… and a missing-model warning;
tracking mode (field, ball, sweep); detect every N frames; style preset
(broadcast, action, frame all) that sets the knobs below it; framing;
pitch lock; lookahead with the VRAM risk zones; an Advanced panner tier
(cluster mode, ball weight, cluster bandwidth, dead zone, FOV tight,
default, wide). Export runs the tracking, and says whether it is active.

**Spec:** DESIGN.md row 6; PARITY.md Module 6 ("AI tracking: …");
slint-inventory §2.11 (rows 2489–2698), §5 (`apply-panner-preset`,
`pick-export-model`, lookahead zones); reco-gui `export.rs` (`run_export`:
model validation, `job.lookahead`, `on_session` → `setup_autocam` with the
preset as base and the knobs over it, ball mode's 0.25 confidence floor,
the field ROI) and `main.rs` (`on_apply_panner_preset`, the lookahead fit
at load: `lookahead_budget_bytes` + `lookahead_fit` on the source size,
lowering an out-of-budget lookahead to the safe value).

## Decisions

- **Engine dependency.** reco-app takes `reco-autocam` (default `ort`, as
  reco-gui) behind an `ai` feature, default on, with `coreml` passed
  through; reco-detect's probe gives availability. Without the feature
  the section says "AI tracking isn't in this build" and stays off.
- **Availability** is probed once off the UI thread at start (ort's
  session builder can take a moment); until it answers, Enable waits.
- **Model.** The sheet's model starts from Preferences' AI model; Choose…
  (dialog key `model`) sets it for the sheet and saves it as the default
  (the Slint app saved a picked model). Export is refused, with the
  reason in the sheet, when tracking is on and the model is missing,
  except in Sweep, which needs none (New: the Slint app wanted one).
- **Presets** are the engine's (`FieldPannerConfig::from_preset_name`):
  choosing one sets the visible knobs; the config sent is the preset with
  the knobs over it, as the Slint app built it.
- **Lookahead zones** come from the preview's GPU (free and total VRAM at
  open, from the worker) and the source size and rate; a lookahead past
  the ceiling at open drops to the safe value (as the Slint app). Without
  a VRAM reading the slider has no zones.
- **Remembered:** tracking on/off, mode, interval, preset, framing, pitch
  lock and lookahead (New: the Slint app kept only the model); the
  Advanced tier follows the preset each time.
- **Status** ("AI tracking: active" or why not) is logged, shown on the
  export card and in the final notice.

## Tasks

1. **reco-app `ai`**: the choices (settings fields with defaults and an
   old-file test), names ↔ engine enums, the panner config (preset base,
   knobs over it), the lookahead zones (pure), the availability probe job;
   `PreviewInfo` gains the VRAM reading; `ExportOptions` gains the
   tracking and `ExportJob` wires `lookahead` and `setup_autocam`,
   reporting `ExportEvent::Tracking`. Tests; a slow ignored export with
   the fixture model.
2. **Sheet**: the AI tracking section (a fold) and its Advanced tier; the
   zoned lookahead slider; the sheet scrolls when the window is short.
3. **Wiring**: enable rules and status, Choose…, presets, remembering,
   Export with tracking, the card's and notice's status.
4. **Final**: check_m6 `ai` (refused without a model, Choose… fixes it,
   presets set the knobs, Sweep needs no model, a two-second export with
   the fixture model says tracking was active and writes detections to
   the events file); check_m0–m7; PARITY, DESIGN, FRICTION; self-review;
   the merge is the owner's choice.

## Rulings made while running

- **Rows, not a fold.** The AI rows show while "Follow the play" is on, as
  in the Slint app; only the Advanced tier is a fold. A fold of its own
  would be a second control for the same thing.
- **The model's rule is Preferences'.** The model is an .onnx file that
  exists, so the default Preferences shows stays one it accepts. Sweep
  takes any text there, but only a usable model is saved. Cost if wrong: a
  coreml build's `.mlmodelc` can't be chosen (it couldn't in Preferences
  either).
- **Enable is gated in code.** Makepad's checkbox takes clicks while it
  looks disabled (FRICTION.md); the sheet puts it back off.
  `RECO_DESKTOP_FAKE_AI` is a check-only switch, as `FAKE_RELEASE` is.
- **The lookahead fills to its knob.** The Slint app's risk slider
  painted its zones whatever the value. Here the zones show dimmed and the
  fill takes the colour of the knob's zone, so the slider moves like every
  other. Without a reading it is a plain slider.
- **What is saved is what the export did.** On a machine that can't run
  the detector, an export saves tracking as off.
- **Plain labels.** "Follow" (Players and ball, Ball only, Sweep (no AI));
  "Detection" (Every N frames); "Tilt: Hold it level" (in this app
  "pitch" means the football pitch); "Group" and "Group size" for the
  cluster mode and bandwidth; "Tightest, Usual and Widest view" for the
  fields of view.
- **ORT on the CPU by default**, as the Slint app's default build; CoreML
  stays the `coreml` feature. The status reads "Ready: runs on CPU" here.
- **AI figures in Stats** (PARITY's Stats item had left them to 6b; this
  plan missed them). The engine measures them only without a lookahead
  (FRICTION.md), so they are sent once measured, never as zeros.
- **One commit for tasks 2 and 3:** the sheet without its wiring does
  nothing.
