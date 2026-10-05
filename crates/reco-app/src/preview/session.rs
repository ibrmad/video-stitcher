//! The engine glue for one open camera pair: the calibration, playback,
//! Reco's `StitchRenderer` and the camera pose (as reco-gui's
//! `PreviewBridge` and `AppState` pose code, without Slint).

use std::path::Path;

use reco_control::pose_control::{PoseControl, PoseControlConfig};
use reco_control::{ControlIntent, IntentTranslator, PoseIntent};
use reco_core::calibration::MatchCalibration;
use reco_core::detect::director::ViewportPosition;
use reco_core::gpu::GpuContext;
use reco_core::render::renderer::InputFormat;
use reco_core::render::stitch_renderer::StitchRenderer;
use reco_core::render::viewport::ViewportConfig;
use reco_core::wgpu;
use reco_io::stitch_job::InputPath;

use super::playback::Playback;

/// The pipeline's output and the ring's format (never sRGB: the shader
/// writes sRGB-encoded values already).
pub const OUTPUT_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Bgra8Unorm;

/// Drag sensitivity (reco-gui `DRAG_DEG_PER_PIXEL`: 0.005 rad per point).
pub const DRAG_DEG_PER_PIXEL: f32 = 0.287;
/// Pose smoothing per step (reco-gui `POSE_SMOOTHING`).
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
}

impl std::fmt::Display for SessionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Calibration(e) => write!(f, "Couldn't read the calibration: {e}"),
            Self::Source(e) => write!(f, "Couldn't open the videos: {e}"),
            Self::Render(e) => write!(f, "The preview failed to render: {e}"),
            Self::NoFrame => write!(f, "No frame decoded yet"),
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
            input_w,
            input_h,
            OUTPUT_FORMAT,
            InputFormat::Yuv420p,
        )
        .map_err(|e| SessionError::Render(e.to_string()))?;
        renderer
            .pipeline_mut()
            .set_lens_correction_amount(lens_correction);
        renderer.set_color_match(true);
        let mut session = Self {
            renderer,
            playback,
            pose: PoseControl::new(pose_config()),
            size,
        };
        session.clamp();
        Ok(session)
    }

    /// Drag by points (X inverted, as reco-gui).
    pub fn pan(&mut self, dx_pt: f32, dy_pt: f32) {
        self.pose.apply_drag(dx_pt, dy_pt);
        self.clamp();
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

    /// One smoothing step toward the target pose; `true` if it moved.
    pub fn smooth(&mut self) -> bool {
        let before = self.pose.current_pose();
        self.pose.tick();
        self.clamp();
        let after = self.pose.current_pose();
        if before.fov_degrees != after.fov_degrees
            && let Some(fov) = after.fov_degrees
        {
            self.renderer.pipeline_mut().set_fov(fov);
        }
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

    /// Keep the pose inside the picture (constrained look, always on in
    /// Module 1; the Adjust panel's switch arrives with Module 5).
    fn clamp(&mut self) {
        let (w, h) = self.size;
        let aspect = w as f32 / h.max(1) as f32;
        let rig_tilt = self.renderer.pipeline().viewport().rig_tilt;
        self.pose
            .clamp_via_coverage(self.renderer.coverage(), aspect, rig_tilt);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::preview::fixtures;

    fn gpu() -> Option<GpuContext> {
        GpuContext::new_blocking()
            .map_err(|e| eprintln!("skipping: no GPU ({e})"))
            .ok()
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
            session.smooth();
        }
        let after = render_to_cpu(&mut session);
        let changed = before.iter().zip(&after).filter(|(a, b)| a != b).count();
        assert!(
            changed > before.len() / 4,
            "pan changed only {changed} pixels"
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
}
