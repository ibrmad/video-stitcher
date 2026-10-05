//! Recording the preview (Record in the view bar). A second renderer at a
//! fixed size draws each source frame shown with the preview's pose; the GPU
//! converts it to NV12 and reads it back; an encoder thread writes it.
//!
//! One frame per source frame shown, so the file plays at the source rate
//! whatever the display does, and nothing is recorded while paused. No frame
//! is dropped: the render thread waits when the encoder's queue is full.
//! The readback runs two frames behind, so finishing flushes it. The
//! renderer stays on the render thread (its GPU types are not `Send`); only
//! the encoder moves. The preview's own renderer is not reused: its render
//! target keeps its first size when the preview resizes.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, SyncSender};
use std::thread::JoinHandle;

use reco_core::calibration::MatchCalibration;
use reco_core::detect::director::ViewportPosition;
use reco_core::encoder::{Encoder, OutputFrame, PixelFormat};
use reco_core::gpu::GpuContext;
use reco_core::lens::rig_correction::render_pitch;
use reco_core::render::stitch_renderer::StitchRenderer;
use reco_core::wgpu;
use reco_io::adapters::create_encoder;

use super::playback::StereoYuv;
use super::session::{FOV_DEFAULT, build_renderer};
use crate::recording::RecordingFormat;

/// Frames queued for the encoder before the render thread waits for it.
const ENCODE_QUEUE: usize = 8;

/// A finished recording.
#[derive(Clone, Debug, PartialEq)]
pub struct Recording {
    /// The file.
    pub path: PathBuf,
    /// Frames written.
    pub frames: u64,
}

/// A recording in progress.
pub struct Recorder {
    renderer: StitchRenderer,
    size: (u32, u32),
    frames: Option<SyncSender<Vec<u8>>>,
    encoder: Option<JoinHandle<Result<u64, String>>>,
    path: PathBuf,
}

impl Recorder {
    /// Open the encoder and a renderer in `format` (blocking for a moment).
    pub fn start(
        gpu: GpuContext,
        calibration: MatchCalibration,
        input: (u32, u32),
        fps: (i32, i32),
        path: &Path,
        format: &RecordingFormat,
    ) -> Result<Self, String> {
        let size = format.size;
        let renderer = build_renderer(
            gpu,
            calibration,
            input,
            size,
            wgpu::TextureFormat::Rgba8Unorm,
        )
        .map_err(|e| e.to_string())?;
        let (mut encoder, _name) = create_encoder(
            path,
            size.0,
            size.1,
            fps,
            &format.codec,
            format.quality.name(),
            None,
            None,
            None,
        )
        .map_err(|e| format!("the encoder didn't start: {e}"))?;
        let (tx, rx) = mpsc::sync_channel::<Vec<u8>>(ENCODE_QUEUE);
        let (width, height) = size;
        let thread = std::thread::Builder::new()
            .name("reco-record".into())
            .spawn(move || {
                let mut written = 0u64;
                for data in rx {
                    encoder
                        .submit(OutputFrame {
                            data: &data,
                            width,
                            height,
                            format: PixelFormat::Nv12,
                            pts_us: 0,
                        })
                        .map_err(|e| format!("frame {written} didn't encode: {e}"))?;
                    written += 1;
                }
                encoder
                    .finish()
                    .map_err(|e| format!("the file didn't close: {e}"))?;
                Ok(written)
            })
            .map_err(|e| e.to_string())?;
        Ok(Self {
            renderer,
            size,
            frames: Some(tx),
            encoder: Some(thread),
            path: path.to_path_buf(),
        })
    }

    /// Record `frame` as the preview shows it: the pose's yaw, pitch and
    /// FOV, kept inside the picture for this recording's aspect.
    pub fn record(&mut self, frame: &StereoYuv, pose: ViewportPosition) -> Result<(), String> {
        let fov = pose.fov_degrees.unwrap_or(FOV_DEFAULT);
        self.renderer.pipeline_mut().set_fov(fov);
        let aspect = self.size.0 as f32 / self.size.1 as f32;
        let clamped = self.renderer.clamp_pose(pose.yaw, pose.pitch, fov, aspect);
        let pitch = render_pitch(
            clamped.yaw,
            clamped.pitch,
            self.renderer.pipeline().viewport().rig_tilt,
        );
        let nv12 = self
            .renderer
            .render_and_readback_nv12(
                &frame.left.as_planes(),
                &frame.right.as_planes(),
                clamped.yaw,
                pitch,
            )
            .map_err(|e| e.to_string())?
            .map(<[u8]>::to_vec);
        match nv12 {
            Some(nv12) => self.send(nv12),
            None => Ok(()),
        }
    }

    /// Follow the preview's tuning: its blend, colour match, tilt, roll and
    /// layout, so the file shows what the preview shows.
    pub fn follow(&mut self, preview: &StitchRenderer) {
        let viewport = preview.pipeline().viewport();
        self.renderer.set_blend_width(viewport.blend_width);
        self.renderer.set_color_match(viewport.color_match);
        self.renderer.set_rig_tilt(viewport.rig_tilt);
        self.renderer.set_rig_roll(viewport.rig_roll);
        self.renderer
            .update_layout(preview.calibration().layout.clone());
    }

    /// The recording renderer's viewport (tests).
    #[cfg(test)]
    pub(crate) fn viewport(&self) -> &reco_core::render::viewport::ViewportConfig {
        self.renderer.pipeline().viewport()
    }

    fn send(&self, nv12: Vec<u8>) -> Result<(), String> {
        let tx = self.frames.as_ref().ok_or("the recording has stopped")?;
        tx.send(nv12).map_err(|_| "the encoder stopped".to_string())
    }

    /// Flush the last frames, close the file and wait for it (blocking:
    /// call on the render thread). An encoder error wins over a flush error.
    pub fn finish(mut self) -> Result<Recording, String> {
        let mut flushed = Ok(());
        loop {
            let next = self.renderer.flush_nv12().map(|o| o.map(<[u8]>::to_vec));
            match next {
                Ok(Some(nv12)) => {
                    if self.send(nv12).is_err() {
                        break;
                    }
                }
                Ok(None) => break,
                Err(e) => {
                    flushed = Err(e.to_string());
                    break;
                }
            }
        }
        drop(self.frames.take());
        let frames = match self.encoder.take().map(JoinHandle::join) {
            Some(Ok(result)) => result?,
            _ => return Err("the encoder thread crashed".into()),
        };
        flushed?;
        Ok(Recording {
            path: self.path.clone(),
            frames,
        })
    }
}

#[cfg(test)]
mod tests {
    use reco_io::ffmpeg::decoder::VideoDecoder;
    use reco_io::stitch_job::InputPath;

    use super::*;
    use crate::preview::fixtures;
    use crate::preview::session::PreviewSession;

    fn temp_video(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("reco-app-{name}-{}.mp4", std::process::id()))
    }

    fn open_session() -> Option<PreviewSession> {
        let (left, right, cal) = fixtures::fast_set()?;
        let gpu = GpuContext::new_blocking().ok()?;
        PreviewSession::open(
            gpu,
            &InputPath::Single(left),
            &InputPath::Single(right),
            &cal,
            (320, 180),
        )
        .ok()
    }

    #[test]
    fn recording_has_one_frame_per_source_frame() {
        let Some(mut session) = open_session() else {
            return;
        };
        let path = temp_video("one-per-frame");
        session
            .start_recording(&path, &fixtures::small_recording())
            .expect("start");
        for i in 0..15 {
            if i > 0 {
                session.playback_mut().step_forward().unwrap();
            }
            session.record_frame().expect("record");
        }
        let recording = session
            .stop_recording()
            .expect("was recording")
            .expect("finish");
        assert_eq!(
            recording.frames, 15,
            "every frame reaches the file, the readback's last two too"
        );
        let video = VideoDecoder::open(&path).expect("a playable file");
        assert_eq!((video.width(), video.height()), (640, 360));
        let secs = video.duration_secs().unwrap_or(0.0);
        assert!(
            (secs - 0.5).abs() < 0.07,
            "15 frames at 30 fps last 0.5 s, not {secs}"
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn recording_keeps_its_size_when_the_preview_resizes() {
        let Some(mut session) = open_session() else {
            return;
        };
        let path = temp_video("fixed-size");
        session
            .start_recording(&path, &fixtures::small_recording())
            .expect("start");
        session.record_frame().unwrap();
        session.resize(1000, 300);
        session.playback_mut().step_forward().unwrap();
        session.record_frame().unwrap();
        session.stop_recording().unwrap().unwrap();
        let video = VideoDecoder::open(&path).expect("a playable file");
        assert_eq!((video.width(), video.height()), (640, 360));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn a_folder_that_does_not_exist_fails_to_start() {
        let Some(mut session) = open_session() else {
            return;
        };
        let err = session
            .start_recording(
                Path::new("/nonexistent/folder/x.mp4"),
                &fixtures::small_recording(),
            )
            .expect_err("no folder, no recording");
        assert!(err.to_string().starts_with("Couldn't record"), "{err}");
        assert!(!session.is_recording());
    }
}
