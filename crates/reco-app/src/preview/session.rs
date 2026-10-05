//! The engine glue for one open camera pair: the calibration, playback,
//! Reco's `StitchRenderer` and the camera pose (as reco-gui's
//! `PreviewBridge` and `AppState` pose code, without Slint).

use std::path::Path;
use std::time::Duration;

use reco_control::pose_control::{PoseControl, PoseControlConfig};
use reco_control::{ControlIntent, IntentTranslator, PoseIntent};
use reco_core::calibration::{CameraParams, FieldRoi, MatchCalibration, PlaneLayout};
use reco_core::detect::director::ViewportPosition;
use reco_core::gpu::GpuContext;
use reco_core::render::renderer::InputFormat;
use reco_core::render::stitch_renderer::StitchRenderer;
use reco_core::render::viewport::ViewportConfig;
use reco_core::wgpu;
use reco_io::stitch_job::InputPath;

use super::fit::Fit;
use super::playback::Playback;
use super::recorder::{Recorder, Recording};
use super::tuning::{CalibrationValues, Tuning};
use crate::lens::Lens;
use crate::project::Camera;
use crate::recording::RecordingQuality;
use reco_core::lens::preview::LensPreviewRenderer;

/// The pipeline's output and the ring's format (never sRGB: the shader
/// writes sRGB-encoded values already).
pub const OUTPUT_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Bgra8Unorm;

/// Drag sensitivity (reco-gui `DRAG_DEG_PER_PIXEL`: 0.005 rad per point).
pub const DRAG_DEG_PER_PIXEL: f32 = 0.287;
/// Pose smoothing per 60 Hz frame (reco-gui `POSE_SMOOTHING`, applied once
/// per frame there); see [`smoothing_for`].
pub const POSE_SMOOTHING: f32 = 0.25;
/// FOV limits and rest value, in degrees.
pub const FOV_MIN: f32 = 20.0;
/// See [`FOV_MIN`].
pub const FOV_MAX: f32 = 150.0;
/// See [`FOV_MIN`].
pub const FOV_DEFAULT: f32 = 75.0;

/// Why a session could not open or render.
#[derive(Debug)]
pub enum SessionError {
    /// The calibration file could not be read.
    Calibration(String),
    /// A video could not be opened or decoded.
    Source(String),
    /// The renderer failed.
    Render(String),
    /// No frame decoded yet.
    NoFrame,
    /// Recording failed.
    Record(String),
    /// The sync offset could not change.
    Sync(String),
}

impl std::fmt::Display for SessionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Calibration(e) => write!(f, "Couldn't read the calibration: {e}"),
            Self::Source(e) => write!(f, "Couldn't open the videos: {e}"),
            Self::Render(e) => write!(f, "The preview failed to render: {e}"),
            Self::NoFrame => write!(f, "No frame decoded yet"),
            Self::Record(e) => write!(f, "Couldn't record: {e}"),
            Self::Sync(e) => write!(f, "Couldn't change the sync offset: {e}"),
        }
    }
}

impl std::error::Error for SessionError {}

/// One open camera pair, ready to render.
pub struct PreviewSession {
    renderer: StitchRenderer,
    playback: Playback,
    pose: PoseControl,
    size: (u32, u32),
    recorder: Option<Recorder>,
    /// The inputs, for reopening at another sync offset.
    inputs: (InputPath, InputPath),
    /// The layout the calibration file had (Reset goes back to it).
    loaded_layout: PlaneLayout,
    /// The lenses the calibration file had (Reset lens goes back to them).
    loaded_lenses: (CameraParams, CameraParams),
    sync_offset: i64,
    field_roi: Option<FieldRoi>,
    /// The view stays inside the stitched picture ("stay inside").
    constrained: bool,
    /// One camera shown flat instead of the stitch (the lens preview), and
    /// what draws it (made on first use).
    shown: Option<Camera>,
    lens_preview: Option<(LensPreviewRenderer, Fit)>,
    /// The calibration changed since it was loaded or saved.
    dirty: bool,
}

fn pose_config() -> PoseControlConfig {
    PoseControlConfig {
        drag_deg_per_pixel: DRAG_DEG_PER_PIXEL,
        smoothing: POSE_SMOOTHING,
        fov_min_degrees: FOV_MIN,
        fov_max_degrees: FOV_MAX,
        // reco-gui's PTZ-head feel: drag right turns the camera right.
        invert_drag_x: true,
        rest_pose: ViewportPosition {
            yaw: 0.0,
            pitch: 0.0,
            fov_degrees: Some(FOV_DEFAULT),
        },
        ..PoseControlConfig::default()
    }
}

/// The share of the gap to the target pose to close in a step of `dt`:
/// [`POSE_SMOOTHING`] per 60 Hz frame whatever the step rate, so easing feels
/// the same at any render size, render path or input rate.
pub fn smoothing_for(dt: Duration) -> f32 {
    1.0 - (1.0 - POSE_SMOOTHING).powf(dt.as_secs_f32() * 60.0)
}

/// The engine opens a pair even when one file gives no frame (it only warns)
/// and reports decode errors as the end of the stream, and it renders a pair
/// of different sizes only to fail on every frame. Both are refused here, so
/// they read as a failed open rather than a broken preview.
fn check_first_frame(playback: &Playback, (width, height): (u32, u32)) -> Result<(), SessionError> {
    let frame = playback.current_frame().ok_or_else(|| {
        SessionError::Source("no frame could be decoded; check that both files are videos".into())
    })?;
    let pixels = width as usize * height as usize;
    if frame.left.y.len() != pixels || frame.right.y.len() != pixels {
        return Err(SessionError::Source(format!(
            "the two cameras' videos are different sizes; both must be {width}×{height}"
        )));
    }
    Ok(())
}

/// Reco's renderer for `calibration` at `size`, set up as the preview uses
/// it: the calibration's lens correction, blend, tilt and roll, and colour
/// match on.
pub(crate) fn build_renderer(
    gpu: GpuContext,
    calibration: MatchCalibration,
    input: (u32, u32),
    size: (u32, u32),
    format: wgpu::TextureFormat,
) -> Result<StitchRenderer, SessionError> {
    let lens_correction = calibration.lens_correction_amount;
    let viewport = ViewportConfig {
        width: size.0,
        height: size.1,
        fov_degrees: FOV_DEFAULT,
        blend_width: calibration.blend_width,
        rig_tilt: calibration.rig_tilt as f32,
        rig_roll: calibration.rig_roll as f32,
        ..ViewportConfig::default()
    };
    let mut renderer = StitchRenderer::new(
        calibration,
        gpu,
        viewport,
        input.0,
        input.1,
        format,
        InputFormat::Yuv420p,
    )
    .map_err(|e| SessionError::Render(e.to_string()))?;
    renderer
        .pipeline_mut()
        .set_lens_correction_amount(lens_correction);
    renderer.set_color_match(true);
    Ok(renderer)
}

impl PreviewSession {
    /// Read the calibration, open both videos, decode the first frame and
    /// build the renderer for a `size` render target. Blocking: run it on
    /// a worker.
    pub fn open(
        gpu: GpuContext,
        left: &InputPath,
        right: &InputPath,
        calibration: &Path,
        size: (u32, u32),
    ) -> Result<Self, SessionError> {
        let calibration = MatchCalibration::from_file(calibration)
            .map_err(|e| SessionError::Calibration(e.to_string()))?;
        let mut playback = Playback::new();
        playback
            .open(left, right, calibration.sync_offset)
            .map_err(|e| SessionError::Source(e.to_string()))?;
        let (input_w, input_h) = playback.input_dimensions().ok_or(SessionError::NoFrame)?;
        check_first_frame(&playback, (input_w, input_h))?;
        let loaded_layout = calibration.layout.clone();
        let loaded_lenses = (calibration.left.clone(), calibration.right.clone());
        let (sync_offset, field_roi) = (calibration.sync_offset, calibration.field_roi.clone());
        let renderer = build_renderer(gpu, calibration, (input_w, input_h), size, OUTPUT_FORMAT)?;
        let mut session = Self {
            renderer,
            playback,
            pose: PoseControl::new(pose_config()),
            size,
            recorder: None,
            inputs: (left.clone(), right.clone()),
            loaded_layout,
            loaded_lenses,
            sync_offset,
            field_roi,
            constrained: true,
            shown: None,
            lens_preview: None,
            dirty: false,
        };
        session.clamp();
        Ok(session)
    }

    /// Drag by points (X inverted, as reco-gui).
    pub fn pan(&mut self, dx_pt: f32, dy_pt: f32) {
        self.pose.apply_drag(dx_pt, dy_pt);
        self.clamp();
    }

    /// Show one camera on its own, flat (`None`: the stitched picture).
    pub fn show_camera(&mut self, camera: Option<Camera>) {
        self.shown = camera;
        if camera.is_some() && self.lens_preview.is_none() {
            let gpu = self.renderer.gpu().clone();
            let (w, h) = self.playback.input_dimensions().unwrap_or((1, 1));
            let aspect = w as f32 / h.max(1) as f32;
            self.lens_preview = Some((
                LensPreviewRenderer::new(&gpu, w, h, aspect, wgpu::TextureFormat::Rgba8Unorm),
                Fit::new(&gpu, OUTPUT_FORMAT),
            ));
        }
    }

    /// Aim the field of view at `degrees` (kept inside its range).
    pub fn set_fov(&mut self, degrees: f32) {
        self.pose.set_target_fov(degrees.clamp(FOV_MIN, FOV_MAX));
        self.clamp();
    }

    /// The field of view the view is heading to, degrees.
    pub fn target_fov(&self) -> f32 {
        self.pose.target_pose().fov_degrees.unwrap_or(FOV_DEFAULT)
    }

    /// Keep the view inside the stitched picture, or let it go past the
    /// edges ("stay inside").
    pub fn set_constrained(&mut self, on: bool) {
        self.constrained = on;
        // Back inside at once, not at the next drag.
        self.clamp();
    }

    /// Whether the view stays inside the picture.
    pub fn constrained(&self) -> bool {
        self.constrained
    }

    /// Change the FOV by `degrees` (negative zooms in).
    pub fn zoom(&mut self, degrees: f32) {
        IntentTranslator::new(&mut self.pose)
            .dispatch(ControlIntent::Pose(PoseIntent::DeltaFovDeg(degrees)));
        self.clamp();
    }

    /// Back to the rest pose.
    pub fn reset_view(&mut self) {
        IntentTranslator::new(&mut self.pose).dispatch(ControlIntent::Pose(PoseIntent::Reset));
        self.clamp();
    }

    /// One smoothing step of `dt` toward the target pose; `true` if it
    /// moved.
    pub fn smooth(&mut self, dt: Duration) -> bool {
        let before = self.pose.current_pose();
        self.pose.tick_with(smoothing_for(dt));
        self.clamp();
        let after = self.pose.current_pose();
        (before.yaw - after.yaw).abs() > f32::EPSILON
            || (before.pitch - after.pitch).abs() > f32::EPSILON
            || before.fov_degrees != after.fov_degrees
    }

    /// A new render-target size (the view's aspect follows it).
    pub fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        self.size = (width, height);
        self.renderer.pipeline_mut().resize(width, height);
        self.clamp();
    }

    /// Render the current frame pair into `target` (submits; the caller
    /// waits for the GPU).
    pub fn render(&self, target: &wgpu::TextureView) -> Result<(), SessionError> {
        let frame = self.playback.current_frame().ok_or(SessionError::NoFrame)?;
        if let (Some(camera), Some((preview, fit))) = (self.shown, self.lens_preview.as_ref()) {
            // One camera, flat, at its own shape inside the frame.
            let calibration = self.renderer.calibration();
            let (planes, params) = match camera {
                Camera::Left => (frame.left.as_planes(), &calibration.left),
                Camera::Right => (frame.right.as_planes(), &calibration.right),
            };
            let amount = self.renderer.pipeline().viewport().lens_correction_amount;
            let gpu = self.renderer.gpu();
            let picture = preview.render_yuv(gpu, &planes, params, amount);
            fit.draw(gpu, &picture, target, self.size);
            return Ok(());
        }
        let rig_tilt = self.renderer.pipeline().viewport().rig_tilt;
        let pose = self.pose.render_pose(rig_tilt);
        self.renderer
            .render_yuv(
                &frame.left.as_planes(),
                &frame.right.as_planes(),
                pose.yaw,
                pose.pitch,
                target,
            )
            .map_err(|e| SessionError::Render(e.to_string()))
    }

    /// The render-target size.
    pub fn size(&self) -> (u32, u32) {
        self.size
    }

    /// The two cameras' inputs.
    pub fn inputs(&self) -> (InputPath, InputPath) {
        self.inputs.clone()
    }

    /// The calibration's sync offset in frames (positive: the right camera
    /// started first).
    pub fn sync_offset(&self) -> i64 {
        self.sync_offset
    }

    /// Apply one change from the Adjust panel (kept inside its range).
    pub fn tune(&mut self, tuning: Tuning) {
        match tuning.clamped() {
            Tuning::Blend(v) => self.renderer.set_blend_width(v),
            Tuning::ColorMatch(on) => self.renderer.set_color_match(on),
            Tuning::Tilt(degrees) => self.renderer.set_rig_tilt(degrees.to_radians()),
            Tuning::Roll(degrees) => self.renderer.set_rig_roll(degrees.to_radians()),
            Tuning::Intersect(v) => self.edit_layout(|l| l.intersect = v),
            Tuning::AxisOffset(v) => self.edit_layout(|l| l.camera_axis_offset = v),
            Tuning::XTy(v) => self.edit_layout(|l| l.x_ty = v),
            Tuning::ResetLayout => self.renderer.update_layout(self.loaded_layout.clone()),
            Tuning::LensCorrection(on) => self
                .renderer
                .pipeline_mut()
                .set_lens_correction_amount(if on { 1.0 } else { 0.0 }),
            Tuning::Lens { cameras, lens } => {
                let calibration = self.renderer.calibration();
                let left = cameras.left().then(|| lens.applied_to(&calibration.left));
                let right = cameras.right().then(|| lens.applied_to(&calibration.right));
                self.renderer.update_camera_params(left, right);
            }
            Tuning::ResetLens => {
                let (left, right) = self.loaded_lenses.clone();
                self.renderer.update_camera_params(Some(left), Some(right));
            }
        }
        self.dirty = true;
        // A recording shows what the preview shows.
        if let Some(recorder) = self.recorder.as_mut() {
            recorder.follow(&self.renderer);
        }
        // Tilt and the layout move the picture's edges: keep the view inside.
        self.clamp();
    }

    /// What the Adjust panel shows.
    pub fn values(&self) -> CalibrationValues {
        let viewport = self.renderer.pipeline().viewport();
        let calibration = self.renderer.calibration();
        let layout = &calibration.layout;
        CalibrationValues {
            blend: viewport.blend_width,
            color_match: viewport.color_match,
            tilt: viewport.rig_tilt.to_degrees(),
            roll: viewport.rig_roll.to_degrees(),
            intersect: layout.intersect,
            axis_offset: layout.camera_axis_offset,
            x_ty: layout.x_ty,
            sync_offset: self.sync_offset,
            roi_points: self
                .field_roi
                .as_ref()
                .map_or(0, |r| r.left.len() + r.right.len()),
            lens_correction: viewport.lens_correction_amount > 0.5,
            left_lens: Lens::of(&calibration.left),
            right_lens: Lens::of(&calibration.right),
            lens_changed: Lens::of(&calibration.left) != Lens::of(&self.loaded_lenses.0)
                || Lens::of(&calibration.right) != Lens::of(&self.loaded_lenses.1),
            lens_size: (calibration.left.width, calibration.left.height),
            dirty: self.dirty,
            // The worker numbers its opens (`Worker::send_calibration`).
            opened: 0,
        }
    }

    /// Play the cameras `frames` apart, from the same frame (blocking: it
    /// reopens the videos). Refused when the offset is as long as the videos.
    pub fn set_sync_offset(&mut self, frames: i64) -> Result<(), SessionError> {
        let length = self.playback.total_frames().unwrap_or(u64::MAX);
        if frames.unsigned_abs() >= length {
            return Err(SessionError::Sync(format!(
                "{frames} frames is as long as the videos"
            )));
        }
        let at = self.playback.frame_index().saturating_sub(1);
        let (left, right) = self.inputs.clone();
        self.playback
            .open(&left, &right, frames)
            .map_err(|e| SessionError::Sync(e.to_string()))?;
        self.sync_offset = frames;
        self.dirty = true;
        if at > 0 {
            // Past the new end it stays at the start; the frame shown is
            // still a real one.
            let _ = self.playback.seek_to_frame(at);
        }
        Ok(())
    }

    /// Set or clear the field outline.
    pub fn set_field_roi(&mut self, roi: Option<FieldRoi>) {
        self.field_roi = roi;
        self.dirty = true;
    }

    /// The calibration as tuned: the file's, with the live blend, tilt,
    /// roll, sync offset and field outline folded in (the Slint app's save).
    pub fn calibration_to_save(&self) -> MatchCalibration {
        let mut out = self.renderer.calibration().clone();
        let viewport = self.renderer.pipeline().viewport();
        out.blend_width = viewport.blend_width;
        out.rig_tilt = f64::from(viewport.rig_tilt);
        out.rig_roll = f64::from(viewport.rig_roll);
        out.lens_correction_amount = viewport.lens_correction_amount;
        out.sync_offset = self.sync_offset;
        out.field_roi = self.field_roi.clone();
        out
    }

    /// Change the layout through `edit` (the renderer recomputes its
    /// coverage).
    fn edit_layout(&mut self, edit: impl FnOnce(&mut PlaneLayout)) {
        let mut layout = self.renderer.calibration().layout.clone();
        edit(&mut layout);
        self.renderer.update_layout(layout);
    }

    /// The calibration was saved: nothing is unsaved now.
    pub fn mark_saved(&mut self) {
        self.dirty = false;
    }

    /// Playback, for the worker's tick and transport commands.
    pub fn playback(&self) -> &Playback {
        &self.playback
    }

    /// See [`Self::playback`].
    pub fn playback_mut(&mut self) -> &mut Playback {
        &mut self.playback
    }

    /// The GPU the renderer uses.
    pub fn gpu(&self) -> &GpuContext {
        self.renderer.gpu()
    }

    /// Start recording to `path` at `size` (blocking for a moment while the
    /// encoder opens: run on the worker).
    pub fn start_recording(
        &mut self,
        path: &Path,
        size: (u32, u32),
        quality: RecordingQuality,
    ) -> Result<(), SessionError> {
        let input = self
            .playback
            .input_dimensions()
            .ok_or(SessionError::NoFrame)?;
        let recorder = Recorder::start(
            self.gpu().clone(),
            self.renderer.calibration().clone(),
            input,
            size,
            self.playback.fps_rational(),
            path,
            quality,
        )
        .map_err(SessionError::Record)?;
        self.recorder = Some(recorder);
        Ok(())
    }

    /// Whether a recording is running.
    pub fn is_recording(&self) -> bool {
        self.recorder.is_some()
    }

    /// Record the frame on screen with the current pose. The worker calls
    /// this once per new source frame.
    pub fn record_frame(&mut self) -> Result<(), SessionError> {
        let (Some(recorder), Some(frame)) = (self.recorder.as_mut(), self.playback.current_frame())
        else {
            return Ok(());
        };
        recorder
            .record(frame, self.pose.current_pose())
            .map_err(SessionError::Record)
    }

    /// Stop: flush, close the file, report it (`None` when not recording).
    pub fn stop_recording(&mut self) -> Option<Result<Recording, SessionError>> {
        self.recorder
            .take()
            .map(|r| r.finish().map_err(SessionError::Record))
    }

    /// Keep the pose inside the picture while the look is constrained
    /// ("stay inside", on by default).
    fn clamp(&mut self) {
        if self.constrained {
            let (w, h) = self.size;
            let aspect = w as f32 / h.max(1) as f32;
            let rig_tilt = self.renderer.pipeline().viewport().rig_tilt;
            self.pose
                .clamp_via_coverage(self.renderer.coverage(), aspect, rig_tilt);
        }
        // The coverage can narrow the FOV (at open too, before any
        // smoothing): the renderer always draws the pose's current FOV.
        if let Some(fov) = self.pose.current_pose().fov_degrees {
            self.renderer.pipeline_mut().set_fov(fov);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lens::{Cameras, Lens};
    use crate::preview::fixtures;
    use crate::project::Camera;

    /// One 60 Hz frame, reco-gui's smoothing step.
    const FRAME: Duration = Duration::from_micros(16_667);

    fn gpu() -> Option<GpuContext> {
        GpuContext::new_blocking()
            .map_err(|e| eprintln!("skipping: no GPU ({e})"))
            .ok()
    }

    #[test]
    fn easing_speed_does_not_depend_on_the_step_rate() {
        // reco-gui eased by POSE_SMOOTHING once per 60 Hz frame.
        let frame = Duration::from_secs_f64(1.0 / 60.0);
        assert!((smoothing_for(frame) - POSE_SMOOTHING).abs() < 1e-6);
        // 25 steps of 4 ms close the same share of the gap as one of 100 ms.
        let many: f32 = 1.0 - (1.0 - smoothing_for(Duration::from_millis(4))).powi(25);
        assert!((many - smoothing_for(Duration::from_millis(100))).abs() < 1e-4);
    }

    #[test]
    fn open_reports_missing_files() {
        let Some(gpu) = gpu() else { return };
        let missing = InputPath::Single("/nonexistent/left.mp4".into());
        let err = PreviewSession::open(
            gpu,
            &missing,
            &missing,
            Path::new("/nonexistent/cal.json"),
            (640, 360),
        )
        .err()
        .expect("opening missing files must fail");
        assert!(matches!(err, SessionError::Calibration(_)), "{err}");
    }

    #[test]
    fn a_file_that_is_not_a_video_fails_to_open() {
        let (Some((left, _, cal)), Some(gpu)) = (fixtures::fast_set(), gpu()) else {
            return;
        };
        let junk =
            std::env::temp_dir().join(format!("reco-app-not-a-video-{}.mp4", std::process::id()));
        std::fs::write(&junk, vec![0x5a_u8; 4096]).unwrap();
        let result = PreviewSession::open(
            gpu,
            &InputPath::Single(left),
            &InputPath::Single(junk.clone()),
            &cal,
            (320, 180),
        );
        let _ = std::fs::remove_file(&junk);
        let err = result
            .err()
            .expect("a right file that is not a video must fail to open");
        assert!(matches!(err, SessionError::Source(_)), "{err}");
    }

    #[test]
    fn videos_of_different_sizes_fail_to_open() {
        let (Some((left, _, cal)), Some(gpu)) = (fixtures::fast_set(), gpu()) else {
            return;
        };
        let Some(other) = fixtures::other_size() else {
            return;
        };
        let err = PreviewSession::open(
            gpu,
            &InputPath::Single(left),
            &InputPath::Single(other),
            &cal,
            (320, 180),
        )
        .err()
        .expect("a right video of another size must fail to open");
        assert!(
            matches!(&err, SessionError::Source(m) if m.contains("size")),
            "{err}"
        );
    }

    #[test]
    fn renders_a_stitched_frame() {
        let (Some((left, right, cal)), Some(gpu)) = (fixtures::fast_set(), gpu()) else {
            return;
        };
        let mut session = PreviewSession::open(
            gpu,
            &InputPath::Single(left),
            &InputPath::Single(right),
            &cal,
            (320, 180),
        )
        .expect("open the fixture pair");
        let pixels = render_to_cpu(&mut session);
        let lit = pixels.iter().filter(|p| (**p & 0x00ff_ffff) != 0).count();
        assert!(
            lit > pixels.len() / 2,
            "mostly black frame: {lit} of {} lit",
            pixels.len()
        );
    }

    #[test]
    fn pan_changes_the_picture() {
        let (Some((left, right, cal)), Some(gpu)) = (fixtures::fast_set(), gpu()) else {
            return;
        };
        let mut session = PreviewSession::open(
            gpu,
            &InputPath::Single(left),
            &InputPath::Single(right),
            &cal,
            (320, 180),
        )
        .expect("open the fixture pair");
        let before = render_to_cpu(&mut session);
        session.pan(80.0, 0.0);
        for _ in 0..40 {
            session.smooth(FRAME);
        }
        let after = render_to_cpu(&mut session);
        let changed = before.iter().zip(&after).filter(|(a, b)| a != b).count();
        assert!(
            changed > before.len() / 4,
            "pan changed only {changed} pixels"
        );
    }

    #[test]
    fn reset_restores_the_opening_view() {
        let (Some((left, right, cal)), Some(gpu)) = (fixtures::fast_set(), gpu()) else {
            return;
        };
        // Near-square, like the viewer: the coverage caps the FOV below 75°.
        let mut session = PreviewSession::open(
            gpu,
            &InputPath::Single(left),
            &InputPath::Single(right),
            &cal,
            (320, 300),
        )
        .expect("open the fixture pair");
        let opening = render_to_cpu(&mut session);
        session.zoom(-30.0);
        for _ in 0..80 {
            session.smooth(FRAME);
        }
        session.reset_view();
        for _ in 0..80 {
            session.smooth(FRAME);
        }
        let reset = render_to_cpu(&mut session);
        let changed = opening.iter().zip(&reset).filter(|(a, b)| a != b).count();
        assert!(
            changed < opening.len() / 100,
            "reset left {changed} of {} pixels changed",
            opening.len()
        );
    }

    /// Render into an offscreen texture and read it back (BGRA in u32).
    fn render_to_cpu(session: &mut PreviewSession) -> Vec<u32> {
        let (w, h) = session.size();
        let gpu = session.gpu().clone();
        let target = gpu.device().create_texture(&wgpu::TextureDescriptor {
            label: Some("test target"),
            size: wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: OUTPUT_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        session
            .render(&target.create_view(&Default::default()))
            .expect("render");
        crate::preview::readback::read_bgra(&gpu, &target, w, h).expect("readback")
    }

    fn open_fast(size: (u32, u32)) -> Option<PreviewSession> {
        let (Some((left, right, cal)), Some(gpu)) = (fixtures::fast_set(), gpu()) else {
            return None;
        };
        PreviewSession::open(
            gpu,
            &InputPath::Single(left),
            &InputPath::Single(right),
            &cal,
            size,
        )
        .ok()
    }

    #[test]
    fn tuning_reaches_the_renderer_and_marks_it_changed() {
        let Some(mut session) = open_fast((320, 180)) else {
            return;
        };
        assert!(!session.values().dirty);
        session.tune(Tuning::Blend(0.9));
        session.tune(Tuning::Tilt(4.0));
        session.tune(Tuning::ColorMatch(false));
        let viewport = session.renderer.pipeline().viewport();
        assert_eq!(viewport.blend_width, 0.3, "kept inside its range");
        assert!((viewport.rig_tilt - 4f32.to_radians()).abs() < 1e-6);
        assert!(!viewport.color_match);
        let values = session.values();
        assert!(values.dirty && values.blend == 0.3 && (values.tilt - 4.0).abs() < 1e-4);
    }

    #[test]
    fn lens_changes_reach_the_renderer_and_reset_restores_them() {
        let Some(mut session) = open_fast((320, 180)) else {
            return;
        };
        let loaded = session.values();
        assert!(!loaded.lens_changed && loaded.left_lens.fx > 0.0);
        let wider = Lens {
            fx: loaded.left_lens.fx * 1.1,
            ..loaded.left_lens
        };
        session.tune(Tuning::Lens {
            cameras: Cameras::Left,
            lens: wider,
        });
        let tuned = session.values();
        assert_eq!(tuned.left_lens, wider);
        assert_eq!(
            tuned.right_lens, loaded.right_lens,
            "the right camera is untouched"
        );
        assert!(tuned.lens_changed && tuned.dirty);
        assert_eq!(
            session.renderer.calibration().left.fx,
            wider.fx,
            "the renderer draws it"
        );
        session.tune(Tuning::Lens {
            cameras: Cameras::Both,
            lens: wider,
        });
        assert_eq!(session.values().right_lens, wider);
        session.tune(Tuning::ResetLens);
        let reset = session.values();
        assert_eq!(
            (reset.left_lens, reset.right_lens),
            (loaded.left_lens, loaded.right_lens)
        );
        assert!(!reset.lens_changed);
        assert_eq!(reset.lens_size, (loaded.lens_size.0, loaded.lens_size.1));
        assert!(reset.lens_size.0 > 0);
    }

    #[test]
    fn a_lens_change_changes_the_picture() {
        let Some(mut session) = open_fast((320, 180)) else {
            return;
        };
        let before = render_to_cpu(&mut session);
        let lens = session.values().left_lens;
        session.tune(Tuning::Lens {
            cameras: Cameras::Both,
            lens: Lens {
                fx: lens.fx * 1.2,
                fy: lens.fy * 1.2,
                ..lens
            },
        });
        assert_ne!(before, render_to_cpu(&mut session), "the stitch follows the lens");
    }

    #[test]
    fn lens_correction_is_saved_with_the_calibration() {
        let Some(mut session) = open_fast((320, 180)) else {
            return;
        };
        assert!(session.values().lens_correction);
        session.tune(Tuning::LensCorrection(false));
        assert!(!session.values().lens_correction && session.values().dirty);
        assert_eq!(session.calibration_to_save().lens_correction_amount, 0.0);
        session.tune(Tuning::LensCorrection(true));
        assert_eq!(session.calibration_to_save().lens_correction_amount, 1.0);
    }

    #[test]
    fn the_field_of_view_follows_its_slider() {
        let Some(mut session) = open_fast((320, 180)) else {
            return;
        };
        // Free of the picture's edges, the slider's whole range.
        session.set_constrained(false);
        session.set_fov(60.0);
        assert_eq!(session.target_fov(), 60.0);
        session.set_fov(1.0);
        assert_eq!(session.target_fov(), FOV_MIN, "kept inside its range");
        session.set_fov(FOV_MAX);
        // Staying inside narrows it to what the picture covers.
        session.set_constrained(true);
        assert!(session.target_fov() < FOV_MAX, "{}", session.target_fov());
        assert!(
            !session.values().dirty,
            "a view change is not a calibration change"
        );
    }

    #[test]
    fn staying_inside_still_zooms_in() {
        let Some(mut session) = open_fast((320, 180)) else {
            return;
        };
        let widest = session.target_fov();
        session.set_fov(FOV_MIN);
        assert_eq!(session.target_fov(), FOV_MIN, "from {widest}");
    }

    #[test]
    fn an_unconstrained_look_may_leave_the_picture() {
        let Some(mut session) = open_fast((320, 180)) else {
            return;
        };
        assert!(session.constrained());
        session.pan(-5000.0, 0.0);
        let kept = session.pose.target_pose().yaw.abs();
        session.set_constrained(false);
        session.pan(-5000.0, 0.0);
        let free = session.pose.target_pose().yaw.abs();
        assert!(free > kept + 0.1, "free {free} vs kept {kept}");
        session.set_constrained(true);
        assert!(
            session.pose.target_pose().yaw.abs() <= kept + 1e-3,
            "back inside at once"
        );
    }

    #[test]
    fn a_camera_shows_flat_and_each_side_differs() {
        let Some(mut session) = open_fast((320, 180)) else {
            return;
        };
        let stitched = render_to_cpu(&mut session);
        session.show_camera(Some(Camera::Left));
        let left = render_to_cpu(&mut session);
        assert_ne!(left, stitched, "one camera, not the stitch");
        session.show_camera(Some(Camera::Right));
        let right = render_to_cpu(&mut session);
        assert_ne!(left, right, "the side matters");
        session.tune(Tuning::LensCorrection(false));
        assert_ne!(right, render_to_cpu(&mut session), "correction shows");
        session.tune(Tuning::LensCorrection(true));
        session.show_camera(None);
        assert_eq!(render_to_cpu(&mut session), stitched, "back to the stitch");
    }

    #[test]
    fn a_camera_sits_inside_the_frame() {
        let Some(mut session) = open_fast((320, 180)) else {
            return;
        };
        session.show_camera(Some(Camera::Left));
        let pixels = render_to_cpu(&mut session);
        let (w, h) = (320, 180);
        let at = |x: usize, y: usize| pixels[y * w + x] & 0x00ff_ffff;
        // The fast pair is 4:3: black at the sides of a 16:9 frame.
        assert!(
            (0..h).all(|y| at(5, y) == 0 && at(w - 6, y) == 0),
            "bars at the sides"
        );
        assert!(
            (0..h).any(|y| at(w / 2, y) != 0),
            "the camera in the middle"
        );
    }

    #[test]
    fn reset_restores_the_loaded_layout() {
        let Some(mut session) = open_fast((320, 180)) else {
            return;
        };
        let loaded = session.values();
        session.tune(Tuning::Intersect(0.9));
        session.tune(Tuning::XTy(-0.05));
        assert_eq!(session.renderer.calibration().layout.intersect, 0.9);
        session.tune(Tuning::ResetLayout);
        let reset = session.values();
        assert_eq!(
            (reset.intersect, reset.x_ty),
            (loaded.intersect, loaded.x_ty)
        );
    }

    #[test]
    fn a_saved_calibration_reloads_with_the_tuned_values() {
        let Some(mut session) = open_fast((320, 180)) else {
            return;
        };
        session.tune(Tuning::Tilt(3.0));
        session.tune(Tuning::Blend(0.2));
        session.tune(Tuning::AxisOffset(0.25));
        session.set_field_roi(Some(FieldRoi {
            left: vec![[0.1, 0.2], [0.9, 0.2], [0.5, 0.8]],
            right: vec![],
        }));
        let path = std::env::temp_dir().join(format!("reco-app-tuned-{}.json", std::process::id()));
        let out = session.calibration_to_save();
        std::fs::write(&path, out.to_json_pretty()).unwrap();
        session.mark_saved();
        assert!(!session.values().dirty);
        let back = MatchCalibration::from_file(&path).expect("a calibration the preview reads");
        assert!(
            (back.rig_tilt - 3f64.to_radians()).abs() < 1e-6,
            "{}",
            back.rig_tilt
        );
        assert!((back.blend_width - 0.2).abs() < 1e-6);
        assert_eq!(back.layout.camera_axis_offset, 0.25);
        assert_eq!(back.field_roi.map(|r| r.left.len()), Some(3));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn the_sync_offset_moves_the_cameras_within_the_videos() {
        let Some(mut session) = open_fast((320, 180)) else {
            return;
        };
        session.playback_mut().seek_to_frame(100).unwrap();
        session.set_sync_offset(30).expect("30 frames fit in 60 s");
        assert_eq!(session.sync_offset(), 30);
        assert_eq!(
            session.playback().frame_index(),
            101,
            "the same frame stays on screen"
        );
        assert!(session.values().dirty);
        assert!(session.set_sync_offset(1_000_000).is_err());
        assert_eq!(
            session.sync_offset(),
            30,
            "a refused offset changes nothing"
        );
    }

    #[test]
    fn tilt_and_layout_change_the_picture() {
        let Some(mut session) = open_fast((320, 180)) else {
            return;
        };
        // Looking straight ahead the render pitch cancels the tilt exactly
        // (rig_correction::render_pitch): it shows once the view turns.
        session.pan(200.0, 0.0);
        for _ in 0..80 {
            session.smooth(FRAME);
        }
        let before = render_to_cpu(&mut session);
        session.tune(Tuning::Tilt(18.0));
        let tilted = render_to_cpu(&mut session);
        assert_ne!(before, tilted, "tilting the rig changes the picture");
        session.tune(Tuning::Tilt(0.0));
        session.tune(Tuning::Intersect(0.3));
        assert_ne!(
            before,
            render_to_cpu(&mut session),
            "the overlap changes the picture"
        );
    }

    #[test]
    fn a_recording_follows_the_tuning() {
        let Some(mut session) = open_fast((320, 180)) else {
            return;
        };
        let path =
            std::env::temp_dir().join(format!("reco-app-tuned-rec-{}.mp4", std::process::id()));
        session
            .start_recording(&path, (640, 360), crate::recording::RecordingQuality::Fast)
            .expect("start");
        session.tune(Tuning::Blend(0.25));
        session.tune(Tuning::Tilt(5.0));
        session.tune(Tuning::Intersect(0.4));
        let recorder = session.recorder.as_ref().unwrap();
        assert_eq!(recorder.viewport().blend_width, 0.25);
        assert!((recorder.viewport().rig_tilt - 5f32.to_radians()).abs() < 1e-6);
        let _ = session.stop_recording();
        let _ = std::fs::remove_file(&path);
    }
}
