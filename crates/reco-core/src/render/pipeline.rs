//! Stitch pipeline orchestration.
//!
//! The [`StitchPipeline`] coordinates all stages: GPU setup, frame ingestion,
//! rendering, viewport cropping, and output encoding. It is the primary
//! entry point for consumers of `reco-core`.
//!
//! ## Usage
//!
//! Most consumers should use [`StitchSession`](crate::session::StitchSession)
//! instead of `StitchPipeline` directly. The pipeline is exposed for advanced
//! use cases like preview windows that need direct surface rendering.
//!
//! ```rust,no_run,compile_fail
//! use reco_core::render::pipeline::StitchPipeline;
//! use reco_core::gpu::GpuContext;
//!
//! let gpu = pollster::block_on(GpuContext::new())?;
//! let pipeline = StitchPipeline::with_gpu(
//!     gpu, calibration, viewport, 1920, 1080,
//!     wgpu::TextureFormat::Rgba8UnormSrgb,
//!     reco_core::render::renderer::InputFormat::Yuv420p,
//! )?;
//! ```

use super::renderer::{InputFormat, RenderError, Renderer};
use super::scene::SceneGeometry;
use super::viewport::{ResolvedViewport, ViewportConfig};
use crate::calibration::MatchCalibration;
use crate::detect::director::ViewportPosition;
use crate::gpu::{GpuContext, GpuError};

use thiserror::Error;

pub use super::planes::{BgraPlanes, FramePlaneView, Nv12Planes, StridedYuvPlanes, YuvPlanes};

/// Errors from the stitch pipeline. `Clone + Send + Sync` so consumers
/// posting results to worker threads can carry the typed error.
#[derive(Debug, Clone, Error)]
pub enum PipelineError {
    /// GPU initialization failed.
    #[error("GPU error: {0}")]
    Gpu(#[from] GpuError),

    /// Render error.
    #[error("render error: {0}")]
    Render(#[from] RenderError),

    /// Wrong StereoFrame variant for this render method.
    #[error("unsupported frame variant: {reason}")]
    UnsupportedFrameVariant {
        /// Description of the mismatch.
        reason: &'static str,
    },

    /// Invalid configuration.
    #[error("invalid config: {reason}")]
    InvalidConfig {
        /// What is wrong.
        reason: String,
    },
}

/// The main stitching pipeline.
///
/// Owns the GPU context, scene geometry, and renderer. Consumers provide
/// YUV420P or NV12 frames and receive stitched RGBA output via
/// [`Self::render_to_target`] or [`Self::render_to_target_nv12`].
pub struct StitchPipeline {
    /// GPU device and queue.
    pub(crate) gpu: GpuContext,
    /// 3D scene layout computed from calibration.
    pub(crate) scene: SceneGeometry,
    /// Calibration data (camera intrinsics + layout).
    pub(crate) calibration: MatchCalibration,
    /// Output viewport configuration.
    pub(crate) viewport: ViewportConfig,
    /// GPU renderer (textures, pipelines, bind groups).
    renderer: Renderer,
    /// Input frame dimensions.
    input_width: u32,
    input_height: u32,
}

/// Pre-built bind groups for GPU-resident zero-copy sources.
///
/// Created by [`StitchPipeline::configure_gpu_source`]. Each slot
/// corresponds to a double-buffer index used by the decode thread.
#[cfg(target_os = "linux")]
pub struct GpuSourceBindGroups {
    left: [wgpu::BindGroup; 2],
    right: [wgpu::BindGroup; 2],
}

impl StitchPipeline {
    /// Create a pipeline with an existing GPU context and custom output format.
    ///
    /// Used by the preview window which needs a specific surface format
    /// and provides its own GPU context (selected with surface compatibility).
    pub fn with_gpu(
        gpu: GpuContext,
        calibration: MatchCalibration,
        viewport: ViewportConfig,
        input_width: u32,
        input_height: u32,
        output_format: impl Into<wgpu::TextureFormat>,
        input_format: InputFormat,
    ) -> Result<Self, PipelineError> {
        // Validate inputs before GPU resource creation.
        if let Err(e) = viewport.validate() {
            return Err(PipelineError::InvalidConfig { reason: e });
        }
        if input_width == 0 || input_height == 0 {
            return Err(PipelineError::InvalidConfig {
                reason: format!("input dimensions must be > 0, got {input_width}x{input_height}"),
            });
        }
        if input_width > crate::calibration::MAX_DIM || input_height > crate::calibration::MAX_DIM {
            return Err(PipelineError::InvalidConfig {
                reason: format!(
                    "input dimensions {input_width}x{input_height} exceed MAX_DIM ({})",
                    crate::calibration::MAX_DIM
                ),
            });
        }

        let output_format = output_format.into();
        let aspect = calibration.left.width as f32 / calibration.left.height as f32;
        let scene = SceneGeometry::from_layout_with_aspect(&calibration.layout, aspect);
        let renderer = Renderer::new(
            &gpu,
            viewport.width,
            viewport.height,
            input_width,
            input_height,
            output_format,
            input_format,
            &scene,
        );

        log::info!(
            "Pipeline initialized: {}x{} output, GPU: {}",
            viewport.width,
            viewport.height,
            gpu.adapter_info.name
        );

        Ok(Self {
            gpu,
            scene,
            calibration,
            viewport,
            renderer,
            input_width,
            input_height,
        })
    }

    /// The name of the GPU this pipeline is running on.
    pub fn gpu_name(&self) -> &str {
        self.gpu.gpu_name()
    }

    /// Shared reference to the GPU context.
    ///
    /// Needed by consumers that create their own wgpu resources
    /// (e.g. surface configuration for a preview window).
    pub fn gpu(&self) -> &GpuContext {
        &self.gpu
    }

    /// The calibration data this pipeline was created with.
    pub fn calibration(&self) -> &MatchCalibration {
        &self.calibration
    }

    /// The current output viewport configuration.
    pub fn viewport(&self) -> &ViewportConfig {
        &self.viewport
    }

    /// Input frame dimensions as `(width, height)`.
    pub fn source_info(&self) -> (u32, u32) {
        (self.input_width, self.input_height)
    }

    /// Input pixel format the pipeline was built for. Needed by the
    /// stacked-video GPU packer so it can pick the matching shader
    /// kernel variant (separate R8 planes for YUV420P vs interleaved
    /// Rg8 UV for NV12) without the consumer passing the format
    /// through a second time.
    pub(crate) fn input_format(&self) -> super::renderer::InputFormat {
        self.renderer.input_format()
    }

    /// Left-side source plane views (Y/U/V texture views). Used by
    /// the stacked-video GPU packer to read the same uploaded
    /// source data the stitch shader samples; the pack runs in
    /// parallel with the panorama render into its own atlas buffer.
    /// For NV12 inputs the `U` view is the interleaved UV texture
    /// and the `V` view is a 1×1 dummy.
    pub(crate) fn left_plane_views(
        &self,
    ) -> (wgpu::TextureView, wgpu::TextureView, wgpu::TextureView) {
        self.renderer.left_plane_views()
    }

    /// Right-side counterpart to [`Self::left_plane_views`].
    pub(crate) fn right_plane_views(
        &self,
    ) -> (wgpu::TextureView, wgpu::TextureView, wgpu::TextureView) {
        self.renderer.right_plane_views()
    }

    /// Update the viewport metadata (aspect ratio, projection matrix).
    ///
    /// **Important:** this does NOT recreate GPU textures or the render
    /// target. Use this for viewport-metadata changes (e.g. surface
    /// reconfigure in a preview window). For actual output resolution
    /// changes, rebuild the pipeline with [`Self::with_gpu`].
    /// Returns `Some((width, height))` on success, or `None` if the
    /// dimensions were zero (ignored). Consumers that own external
    /// staging buffers (e.g.
    /// [`RgbaReadback`](crate::gpu::rgba_readback::RgbaReadback)) should
    /// recreate them when the returned size differs from the previous.
    pub fn resize(&mut self, width: u32, height: u32) -> Option<(u32, u32)> {
        if width == 0 || height == 0 {
            log::warn!("resize({width}, {height}) ignored: dimensions must be non-zero");
            return None;
        }
        self.viewport.width = width;
        self.viewport.height = height;
        Some((width, height))
    }

    /// Set the vertical field of view in degrees.
    ///
    /// Values are clamped to `[1.0, 179.0]` to prevent degenerate
    /// projection matrices (0 or 180 would produce NaN/Inf).
    pub fn set_fov(&mut self, fov_degrees: f32) {
        self.viewport.fov_degrees = fov_degrees.clamp(1.0, 179.0);
    }

    /// Get the current field of view in degrees.
    pub fn fov(&self) -> f32 {
        self.viewport.fov_degrees
    }

    /// Set the lens distortion correction amount for the stitch view.
    pub fn set_lens_correction_amount(&mut self, amount: f32) {
        self.viewport.lens_correction_amount = amount.clamp(0.0, 1.0);
    }

    /// Update calibration parameters. Recomputes [`SceneGeometry`] from the
    /// new layout. Takes effect on the next render call (uniforms are rebuilt
    /// each frame from the stored calibration and scene).
    ///
    /// No GPU pipeline recreation needed - only the uniform data changes.
    pub fn update_calibration(&mut self, calibration: MatchCalibration) {
        let aspect = calibration.left.width as f32 / calibration.left.height as f32;
        self.scene = SceneGeometry::from_layout_with_aspect(&calibration.layout, aspect);
        self.calibration = calibration;
        log::debug!("Pipeline calibration updated");
    }

    /// Update only the plane layout (convenience for slider adjustments).
    ///
    /// Equivalent to cloning the current calibration, replacing its layout,
    /// and calling [`update_calibration`](Self::update_calibration).
    pub fn update_layout(&mut self, layout: crate::calibration::PlaneLayout) {
        let mut cal = self.calibration.clone();
        cal.layout = layout;
        self.update_calibration(cal);
    }

    /// Update per-camera intrinsics (focal, principal point, distortion)
    /// for one or both cameras without touching the plane layout or rig
    /// orientation.
    ///
    /// Intended for interactive lens tweaking in a GUI: each `CameraParams`
    /// change is written into the shader's per-frame uniform buffer, so the
    /// next render call reflects the new values. No GPU pipeline or scene
    /// recreation is needed - cheap enough (~microseconds) to call on
    /// every slider drag.
    ///
    /// `left`/`right` are `None` to leave that side untouched. If both are
    /// `None` this is a no-op. Passing `Some` for a side replaces that
    /// side's `CameraParams` on the stored calibration; the next render
    /// picks it up automatically.
    ///
    /// Does not recompute `SceneGeometry` because the plane layout is
    /// unchanged; only the camera intrinsics (which live on the stored
    /// calibration and are re-read each frame) need updating.
    pub fn update_camera_params(
        &mut self,
        left: Option<crate::calibration::CameraParams>,
        right: Option<crate::calibration::CameraParams>,
    ) {
        if left.is_none() && right.is_none() {
            return;
        }
        if let Some(l) = left {
            self.calibration.left = l;
        }
        if let Some(r) = right {
            self.calibration.right = r;
        }
        log::debug!("Pipeline camera params updated");
    }

    /// Set up bind groups for GPU-resident zero-copy input.
    ///
    /// Creates bind groups for the provided shared textures (Y + UV per slot
    /// per camera). Call once during setup, then pass the result to
    /// [`Self::render_gpu_frame`] each frame.
    #[cfg(target_os = "linux")]
    pub fn configure_gpu_source(
        &mut self,
        left_textures: [(
            &crate::interop::vulkan::SharedTexture,
            &crate::interop::vulkan::SharedTexture,
        ); 2],
        right_textures: [(
            &crate::interop::vulkan::SharedTexture,
            &crate::interop::vulkan::SharedTexture,
        ); 2],
    ) -> GpuSourceBindGroups {
        let left_bg_0 = self.renderer.create_texture_bind_group(
            &left_textures[0].0.texture,
            &left_textures[0].1.texture,
            "left_slot0",
        );
        let left_bg_1 = self.renderer.create_texture_bind_group(
            &left_textures[1].0.texture,
            &left_textures[1].1.texture,
            "left_slot1",
        );
        let right_bg_0 = self.renderer.create_texture_bind_group(
            &right_textures[0].0.texture,
            &right_textures[0].1.texture,
            "right_slot0",
        );
        let right_bg_1 = self.renderer.create_texture_bind_group(
            &right_textures[1].0.texture,
            &right_textures[1].1.texture,
            "right_slot1",
        );
        GpuSourceBindGroups {
            left: [left_bg_0, left_bg_1],
            right: [right_bg_0, right_bg_1],
        }
    }

    /// Select bind groups for a GPU-resident frame and render.
    ///
    /// Call this instead of manually setting bind groups on the renderer.
    #[cfg(target_os = "linux")]
    pub fn render_gpu_frame(
        &mut self,
        bind_groups: &GpuSourceBindGroups,
        left_slot: u8,
        right_slot: u8,
        yaw: f32,
        pitch: f32,
    ) -> wgpu::CommandBuffer {
        self.renderer
            .set_left_bind_group(bind_groups.left[left_slot as usize].clone());
        self.renderer
            .set_right_bind_group(bind_groups.right[right_slot as usize].clone());
        self.render_to_target_gpu(yaw, pitch)
    }

    /// Create a texture bind group from Y + UV textures.
    pub fn create_texture_bind_group(
        &self,
        y_texture: &wgpu::Texture,
        uv_texture: &wgpu::Texture,
        label: &str,
    ) -> wgpu::BindGroup {
        self.renderer
            .create_texture_bind_group(y_texture, uv_texture, label)
    }

    /// Render from pre-built bind groups (VRAM pool path).
    pub fn render_with_bind_groups(
        &mut self,
        left_bg: &wgpu::BindGroup,
        right_bg: &wgpu::BindGroup,
        yaw: f32,
        pitch: f32,
    ) -> wgpu::CommandBuffer {
        self.renderer.set_left_bind_group(left_bg.clone());
        self.renderer.set_right_bind_group(right_bg.clone());
        self.render_to_target_gpu(yaw, pitch)
    }

    /// Render from imported GPU textures (e.g. Metal/VideoToolbox zero-copy).
    ///
    /// Takes raw Y + UV texture references for each camera, creates bind groups,
    /// and renders. Unlike [`Self::render_gpu_frame`] which uses pre-built
    /// double-buffered bind groups, this creates them per-frame (the overhead
    /// is negligible compared to decode time).
    pub fn render_imported_textures(
        &mut self,
        left_y: &wgpu::Texture,
        left_uv: &wgpu::Texture,
        right_y: &wgpu::Texture,
        right_uv: &wgpu::Texture,
        yaw: f32,
        pitch: f32,
    ) -> wgpu::CommandBuffer {
        let left_bg = self
            .renderer
            .create_texture_bind_group(left_y, left_uv, "metal_left");
        let right_bg = self
            .renderer
            .create_texture_bind_group(right_y, right_uv, "metal_right");
        self.renderer.set_left_bind_group(left_bg);
        self.renderer.set_right_bind_group(right_bg);
        self.render_to_target_gpu(yaw, pitch)
    }

    /// Render from pre-built GPU texture views.
    ///
    /// Used by the D3D11VA zero-copy path where NV12 plane views are
    /// created from `TextureAspect::Plane0` / `Plane1`.
    pub fn render_imported_views(
        &mut self,
        left_y: &wgpu::TextureView,
        left_uv: &wgpu::TextureView,
        right_y: &wgpu::TextureView,
        right_uv: &wgpu::TextureView,
        yaw: f32,
        pitch: f32,
    ) -> wgpu::CommandBuffer {
        let left_bg = self
            .renderer
            .create_bind_group_from_views(left_y, left_uv, "d3d11_left");
        let right_bg = self
            .renderer
            .create_bind_group_from_views(right_y, right_uv, "d3d11_right");
        self.renderer.set_left_bind_group(left_bg);
        self.renderer.set_right_bind_group(right_bg);
        self.render_to_target_gpu(yaw, pitch)
    }

    /// Process a CPU-resident stereo frame and return the render command buffer.
    ///
    /// Handles YUV420P vs NV12 format differences internally.
    /// For GPU-resident frames, use [`Self::render_gpu_frame`] instead.
    pub fn render_stereo_frame(
        &self,
        frame: &crate::source::StereoFrame,
        yaw: f32,
        pitch: f32,
    ) -> Result<wgpu::CommandBuffer, PipelineError> {
        use crate::source::StereoFrame;
        match frame {
            StereoFrame::Yuv420p(pair) => {
                let left = YuvPlanes {
                    y: &pair.left.y,
                    u: &pair.left.u,
                    v: &pair.left.v,
                };
                let right = YuvPlanes {
                    y: &pair.right.y,
                    u: &pair.right.u,
                    v: &pair.right.v,
                };
                self.render_to_target(&left, &right, yaw, pitch)
            }
            StereoFrame::Nv12(pair) => {
                let left = Nv12Planes {
                    y: &pair.left.y,
                    uv: &pair.left.uv,
                };
                let right = Nv12Planes {
                    y: &pair.right.y,
                    uv: &pair.right.uv,
                };
                self.render_to_target_nv12(&left, &right, yaw, pitch)
            }
            StereoFrame::GpuResident { .. } => Err(PipelineError::UnsupportedFrameVariant {
                reason: "GpuResident frames must use render_gpu_frame()",
            }),
            #[allow(unreachable_patterns)]
            _ => Err(PipelineError::UnsupportedFrameVariant {
                reason: "unsupported StereoFrame variant for CPU render path",
            }),
        }
    }

    /// Render a frame directly to a texture view (for window display).
    ///
    /// Unlike the encode path, this does NOT read back to CPU — the result
    /// stays on the GPU and is presented to the surface.
    pub fn render_to_view(
        &self,
        left: &YuvPlanes<'_>,
        right: &YuvPlanes<'_>,
        yaw: f32,
        pitch: f32,
        target_view: &wgpu::TextureView,
    ) -> Result<(), PipelineError> {
        self.renderer
            .upload_left_yuv(&self.gpu, left.y, left.u, left.v)?;
        self.renderer
            .upload_right_yuv(&self.gpu, right.y, right.u, right.v)?;
        self.render_uploaded_to_view(yaw, pitch, target_view);
        Ok(())
    }

    /// Render the frames uploaded last (by [`Self::render_to_view`] or
    /// [`Self::render_nv12_to_view`]) to a texture view at a new yaw/pitch,
    /// without uploading them again.
    ///
    /// For a view change on a frame that hasn't changed (a pan or a zoom
    /// while paused): the upload is most of a render's cost for large
    /// inputs.
    pub fn render_uploaded_to_view(&self, yaw: f32, pitch: f32, target_view: &wgpu::TextureView) {
        let viewport = ResolvedViewport {
            config: self.viewport.clone(),
            position: ViewportPosition {
                yaw,
                pitch,
                fov_degrees: None,
            },
        };

        self.renderer.render_to_view(
            &self.gpu,
            &self.scene,
            &self.calibration,
            &viewport,
            self.viewport.blend_width,
            target_view,
        );
    }

    /// Render NV12 frames directly to a texture view (for window display).
    ///
    /// Like [`Self::render_to_view`] but accepts NV12 input (Y + interleaved
    /// UV) instead of YUV420P. Requires the pipeline to be initialized with
    /// `InputFormat::Nv12`.
    pub fn render_nv12_to_view(
        &self,
        left: &Nv12Planes<'_>,
        right: &Nv12Planes<'_>,
        yaw: f32,
        pitch: f32,
        target_view: &wgpu::TextureView,
    ) -> Result<(), PipelineError> {
        self.renderer.upload_left_nv12(&self.gpu, left.y, left.uv)?;
        self.renderer
            .upload_right_nv12(&self.gpu, right.y, right.uv)?;
        self.render_uploaded_to_view(yaw, pitch, target_view);
        Ok(())
    }

    /// Render a frame to the internal render target without CPU readback.
    ///
    /// Uploads YUV planes and returns the render `CommandBuffer` without
    /// submitting. The caller must submit it (typically together with NV12
    /// conversion commands via the NV12 converter).
    #[cfg_attr(
        feature = "profiling",
        tracing::instrument(skip_all, name = "render_to_target")
    )]
    pub fn render_to_target(
        &self,
        left: &YuvPlanes<'_>,
        right: &YuvPlanes<'_>,
        yaw: f32,
        pitch: f32,
    ) -> Result<wgpu::CommandBuffer, PipelineError> {
        self.renderer
            .upload_left_yuv(&self.gpu, left.y, left.u, left.v)?;
        self.renderer
            .upload_right_yuv(&self.gpu, right.y, right.u, right.v)?;

        let viewport = ResolvedViewport {
            config: self.viewport.clone(),
            position: ViewportPosition {
                yaw,
                pitch,
                fov_degrees: None,
            },
        };

        Ok(self.renderer.render_to_target(
            &self.gpu,
            &self.scene,
            &self.calibration,
            &viewport,
            self.viewport.blend_width,
        ))
    }

    /// Upload NV12 frames and render to the internal target.
    ///
    /// Like `render_to_target` but accepts NV12 input (Y + interleaved UV)
    /// instead of YUV420P. Requires the pipeline to be initialized with
    /// `InputFormat::Nv12`.
    #[cfg_attr(
        feature = "profiling",
        tracing::instrument(skip_all, name = "render_to_target_nv12")
    )]
    pub fn render_to_target_nv12(
        &self,
        left: &Nv12Planes<'_>,
        right: &Nv12Planes<'_>,
        yaw: f32,
        pitch: f32,
    ) -> Result<wgpu::CommandBuffer, PipelineError> {
        self.renderer.upload_left_nv12(&self.gpu, left.y, left.uv)?;
        self.renderer
            .upload_right_nv12(&self.gpu, right.y, right.uv)?;

        let viewport = ResolvedViewport {
            config: self.viewport.clone(),
            position: ViewportPosition {
                yaw,
                pitch,
                fov_degrees: None,
            },
        };

        Ok(self.renderer.render_to_target(
            &self.gpu,
            &self.scene,
            &self.calibration,
            &viewport,
            self.viewport.blend_width,
        ))
    }

    /// Upload packed BGRA/RGBA frames and render to the internal target.
    ///
    /// Expects each plane as `width * height * 4` bytes in (R, G, B, A) byte
    /// order. Use [`BgraPlanes::from_bgra_swizzle_into`] when the source
    /// is BGRA. Requires the pipeline to be initialized with
    /// [`InputFormat::Bgra`](crate::render::renderer::InputFormat#variant.Bgra).
    #[cfg_attr(
        feature = "profiling",
        tracing::instrument(skip_all, name = "render_to_target_bgra")
    )]
    pub fn render_to_target_bgra(
        &self,
        left: &BgraPlanes<'_>,
        right: &BgraPlanes<'_>,
        yaw: f32,
        pitch: f32,
    ) -> Result<wgpu::CommandBuffer, PipelineError> {
        self.renderer.upload_left_bgra(&self.gpu, left.rgba)?;
        self.renderer.upload_right_bgra(&self.gpu, right.rgba)?;

        let viewport = ResolvedViewport {
            config: self.viewport.clone(),
            position: ViewportPosition {
                yaw,
                pitch,
                fov_degrees: None,
            },
        };

        Ok(self.renderer.render_to_target(
            &self.gpu,
            &self.scene,
            &self.calibration,
            &viewport,
            self.viewport.blend_width,
        ))
    }

    /// Render from GPU-resident RGBA textures (e.g. Bayer demosaic output).
    ///
    /// Copies source textures into the input planes, then renders the
    /// stitch to the internal target. Returns the complete command buffer.
    /// The caller submits the demosaic encoder first, then this one.
    /// Requires `InputFormat::Bgra`.
    pub fn render_from_gpu_rgba(
        &self,
        left_rgba: &wgpu::Texture,
        right_rgba: &wgpu::Texture,
        yaw: f32,
        pitch: f32,
    ) -> wgpu::CommandBuffer {
        // Copy demosaiced textures into stitch pipeline input planes
        let mut copy_encoder =
            self.gpu
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("bayer_copy"),
                });
        self.renderer
            .copy_texture_to_left(&mut copy_encoder, left_rgba);
        self.renderer
            .copy_texture_to_right(&mut copy_encoder, right_rgba);
        self.gpu
            .queue
            .submit(std::iter::once(copy_encoder.finish()));

        // Render stitch (reads from the just-populated input textures)
        self.render_to_target_gpu(yaw, pitch)
    }

    /// Render to the internal target without upload or readback (zero-copy path).
    ///
    /// Returns the render `CommandBuffer` without submitting. Assumes textures
    /// are already populated via CUDA/Vulkan shared memory.
    #[cfg_attr(
        feature = "profiling",
        tracing::instrument(skip_all, name = "render_to_target_gpu")
    )]
    /// Render to the internal target using whatever textures are currently
    /// bound. Call [`Self::render_imported_textures`] once to set up
    /// bind groups, then use this for subsequent frames with the same
    /// textures to avoid per-frame bind group allocation.
    pub fn render_to_target_gpu(&self, yaw: f32, pitch: f32) -> wgpu::CommandBuffer {
        let viewport = ResolvedViewport {
            config: self.viewport.clone(),
            position: ViewportPosition {
                yaw,
                pitch,
                fov_degrees: None,
            },
        };

        self.renderer.render_to_target(
            &self.gpu,
            &self.scene,
            &self.calibration,
            &viewport,
            self.viewport.blend_width,
        )
    }

    /// Enable 180-degree UV flip for the GPU zero-copy path.
    ///
    /// When set, the shader flips texture coordinates before sampling,
    /// equivalent to the CPU path's buffer reversal for rotated video
    /// (e.g., DJI cameras with rotation=180 metadata).
    pub fn set_flip_180(&mut self, left: bool, right: bool) {
        self.renderer.set_flip_180(left, right);
    }

    pub fn set_full_range(&mut self, full_range: bool) {
        self.renderer.set_full_range(full_range);
    }

    /// Turn automatic exposure and colour matching between the two
    /// cameras on or off (see [`ViewportConfig::color_match`]).
    pub fn set_color_match(&mut self, enabled: bool) {
        self.viewport.color_match = enabled;
    }

    /// Access the rendered RGBA texture for NV12 conversion.
    pub fn render_target(&self) -> &wgpu::Texture {
        self.renderer.render_target()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::planes::copy_plane_tight;

    /// Build a test plane where row `r` contains byte value `r` for the first
    /// `width` bytes, followed by `0xFF` padding up to `stride`.
    fn padded_plane(width: u32, height: u32, stride: u32) -> Vec<u8> {
        let mut buf = vec![0xFF; (stride * height) as usize];
        for r in 0..height {
            for c in 0..width {
                buf[(r * stride + c) as usize] = r as u8;
            }
        }
        buf
    }

    #[test]
    fn copy_into_strips_row_padding() {
        // 4-pixel wide plane padded to 8-byte rows (typical OBS alignment).
        let y_data = padded_plane(4, 3, 8);
        let u_data = padded_plane(2, 2, 4);
        let v_data = padded_plane(2, 2, 4);
        let strided = StridedYuvPlanes {
            y: FramePlaneView {
                data: &y_data,
                stride: 8,
                width: 4,
                height: 3,
            },
            u: FramePlaneView {
                data: &u_data,
                stride: 4,
                width: 2,
                height: 2,
            },
            v: FramePlaneView {
                data: &v_data,
                stride: 4,
                width: 2,
                height: 2,
            },
        };

        let mut buffer = Vec::new();
        let tight = strided.copy_into(&mut buffer);

        assert_eq!(tight.y.len(), 12);
        assert_eq!(tight.u.len(), 4);
        assert_eq!(tight.v.len(), 4);
        // Row 0 should be [0,0,0,0], row 1 [1,1,1,1], etc - no 0xFF padding.
        assert_eq!(tight.y, &[0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2]);
        assert_eq!(tight.u, &[0, 0, 1, 1]);
        assert_eq!(tight.v, &[0, 0, 1, 1]);
    }

    #[test]
    fn copy_into_fast_path_when_tight() {
        // stride == width means no padding - fast path takes a single memcpy.
        let y_data: Vec<u8> = (0..12).collect();
        let u_data: Vec<u8> = (0..4).collect();
        let v_data: Vec<u8> = (4..8).collect();
        let strided = StridedYuvPlanes {
            y: FramePlaneView {
                data: &y_data,
                stride: 4,
                width: 4,
                height: 3,
            },
            u: FramePlaneView {
                data: &u_data,
                stride: 2,
                width: 2,
                height: 2,
            },
            v: FramePlaneView {
                data: &v_data,
                stride: 2,
                width: 2,
                height: 2,
            },
        };
        let mut buffer = Vec::new();
        let tight = strided.copy_into(&mut buffer);
        assert_eq!(tight.y, y_data.as_slice());
        assert_eq!(tight.u, u_data.as_slice());
        assert_eq!(tight.v, v_data.as_slice());
    }

    #[test]
    fn copy_into_reuses_buffer_without_realloc() {
        let plane = padded_plane(4, 3, 8);
        let strided = StridedYuvPlanes {
            y: FramePlaneView {
                data: &plane,
                stride: 8,
                width: 4,
                height: 3,
            },
            u: FramePlaneView {
                data: &plane,
                stride: 8,
                width: 2,
                height: 2,
            },
            v: FramePlaneView {
                data: &plane,
                stride: 8,
                width: 2,
                height: 2,
            },
        };

        let mut buffer = Vec::with_capacity(64);
        let cap_before = buffer.capacity();
        let _tight = strided.copy_into(&mut buffer);
        // 12 + 4 + 4 = 20 bytes needed, 64 capacity, no realloc.
        assert_eq!(buffer.capacity(), cap_before);

        // Second call with same dims: still no realloc.
        let _tight2 = strided.copy_into(&mut buffer);
        assert_eq!(buffer.capacity(), cap_before);
    }

    // ── B-24 regression: copy_plane_tight must not panic on malformed input

    #[test]
    fn copy_plane_tight_handles_stride_less_than_width() {
        // Pathological: caller declares width=8 but stride=4.
        // Before B-24 this would overlap rows and panic on slice
        // index. Now it zero-fills and logs.
        let data = vec![0xAA_u8; 16]; // 4 rows * 4 stride
        let src = FramePlaneView {
            data: &data,
            stride: 4,
            width: 8,
            height: 4,
        };
        let mut dst = vec![0xFF_u8; 32]; // 8*4
        copy_plane_tight(&src, &mut dst);
        assert!(
            dst.iter().all(|&b| b == 0),
            "zero-fill expected on stride<width"
        );
    }

    #[test]
    fn copy_plane_tight_handles_short_source_buffer() {
        let data = vec![0x77_u8; 4]; // Way too small for 8*4 claim.
        let src = FramePlaneView {
            data: &data,
            stride: 8,
            width: 8,
            height: 4,
        };
        let mut dst = vec![0xFF_u8; 32];
        copy_plane_tight(&src, &mut dst);
        assert!(dst.iter().all(|&b| b == 0));
    }

    #[test]
    fn copy_plane_tight_handles_dst_size_mismatch() {
        let data = vec![0xAB_u8; 32];
        let src = FramePlaneView {
            data: &data,
            stride: 8,
            width: 8,
            height: 4,
        };
        let mut dst = vec![0xFF_u8; 16]; // half of what's claimed
        copy_plane_tight(&src, &mut dst);
        assert!(dst.iter().all(|&b| b == 0));
    }

    #[test]
    fn copy_plane_tight_still_fast_path_when_tight() {
        let data: Vec<u8> = (0..32).collect();
        let src = FramePlaneView {
            data: &data,
            stride: 8,
            width: 8,
            height: 4,
        };
        let mut dst = vec![0; 32];
        copy_plane_tight(&src, &mut dst);
        assert_eq!(dst.as_slice(), data.as_slice());
    }

    /// Output and input size for the GPU tests: small, to keep them quick.
    const SIDE: u32 = 64;

    /// Two 64×64 cameras side by side (as `session/tests.rs`).
    fn gpu_test_calibration() -> MatchCalibration {
        let cam = crate::calibration::CameraParams {
            width: SIDE,
            height: SIDE,
            fx: 32.0,
            fy: 32.0,
            cx: 32.0,
            cy: 32.0,
            d: [0.0; 4],
        };
        MatchCalibration {
            left: cam.clone(),
            right: cam,
            layout: crate::calibration::PlaneLayout {
                camera_axis_offset: 0.25,
                intersect: 0.5,
                x_ty: 0.0,
                x_rz: 0.0,
                z_rx: 0.0,
                x_rx: 0.0,
                z_rz: 0.0,
            },
            rig_tilt: 0.0,
            rig_roll: 0.0,
            sync_offset: 0,
            field_roi: None,
            lens_correction_amount: 1.0,
            blend_width: 0.05,
        }
    }

    /// A YUV420P frame whose luma rises left to right from `base`, so a
    /// pan or a new frame changes the picture.
    fn ramp(base: u8) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
        let y = (0..SIDE * SIDE)
            .map(|i| base.wrapping_add((i % SIDE * 2) as u8))
            .collect();
        let chroma = vec![128; (SIDE / 2 * SIDE / 2) as usize];
        (y, chroma.clone(), chroma)
    }

    fn planes(frame: &(Vec<u8>, Vec<u8>, Vec<u8>)) -> YuvPlanes<'_> {
        YuvPlanes {
            y: &frame.0,
            u: &frame.1,
            v: &frame.2,
        }
    }

    /// A render target the tests can read back.
    fn target(gpu: &GpuContext) -> wgpu::Texture {
        gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("test target"),
            size: wgpu::Extent3d {
                width: SIDE,
                height: SIDE,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Bgra8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        })
    }

    /// Render with `draw` into a new target and read its pixels back.
    fn drawn(gpu: &GpuContext, draw: impl FnOnce(&wgpu::TextureView)) -> Vec<u8> {
        let texture = target(gpu);
        draw(&texture.create_view(&Default::default()));
        // 64 px × 4 bytes is 256 bytes: rows need no padding.
        let row = SIDE * 4;
        let buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("test readback"),
            size: u64::from(row * SIDE),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = gpu.device.create_command_encoder(&Default::default());
        encoder.copy_texture_to_buffer(
            texture.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(row),
                    rows_per_image: Some(SIDE),
                },
            },
            wgpu::Extent3d {
                width: SIDE,
                height: SIDE,
                depth_or_array_layers: 1,
            },
        );
        gpu.queue.submit(Some(encoder.finish()));
        buffer.slice(..).map_async(wgpu::MapMode::Read, |_| {});
        gpu.device
            .poll(wgpu::PollType::wait_indefinitely())
            .expect("GPU poll");
        buffer.slice(..).get_mapped_range().to_vec()
    }

    /// Cameras for the whole-field tests: 512×288 equidistant fisheyes
    /// (fx 240, about 120° across) in the 2026-10-03 rig's layout.
    fn wide_calibration(rig_tilt: f64, rig_roll: f64) -> MatchCalibration {
        let cam = crate::calibration::CameraParams {
            width: 512,
            height: 288,
            fx: 240.0,
            fy: 240.0,
            cx: 256.0,
            cy: 144.0,
            d: [0.0; 4],
        };
        MatchCalibration {
            left: cam.clone(),
            right: cam,
            layout: crate::calibration::PlaneLayout {
                camera_axis_offset: 0.2405,
                intersect: 0.5366,
                x_ty: -0.0089,
                x_rz: -0.0407,
                z_rx: -0.0444,
                x_rx: 0.0,
                z_rz: 0.0,
            },
            rig_tilt,
            rig_roll,
            sync_offset: 0,
            field_roi: None,
            lens_correction_amount: 1.0,
            blend_width: 0.05,
        }
    }

    /// A 512×288 YUV420P frame: black, with white 5×5 dots centred on
    /// the given pixels.
    fn dots(at: &[(u32, u32)]) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
        let (w, h) = (512u32, 288u32);
        let mut y = vec![0u8; (w * h) as usize];
        for &(cx, cy) in at {
            for py in cy - 2..=cy + 2 {
                for px in cx - 2..=cx + 2 {
                    y[(py * w + px) as usize] = 255;
                }
            }
        }
        let chroma = vec![128; (w / 2 * h / 2) as usize];
        (y, chroma.clone(), chroma)
    }

    /// A 512×288 frame full of detail: a fine checker over a ramp.
    fn busy() -> (Vec<u8>, Vec<u8>, Vec<u8>) {
        let (w, h) = (512u32, 288u32);
        let y = (0..w * h)
            .map(|i| {
                let (x, row) = (i % w, i / w);
                let checker = if (x + row) % 2 == 0 { 70 } else { 0 };
                (x * 120 / w + checker + 30) as u8
            })
            .collect();
        let chroma = vec![128; (w / 2 * h / 2) as usize];
        (y, chroma.clone(), chroma)
    }

    /// Render with `draw` into a `width × height` target and read its
    /// BGRA pixels back (rows unpadded).
    fn drawn_sized(
        gpu: &GpuContext,
        width: u32,
        height: u32,
        draw: impl FnOnce(&wgpu::TextureView),
    ) -> Vec<u8> {
        let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("test target"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Bgra8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        draw(&texture.create_view(&Default::default()));
        let row = (width * 4).div_ceil(256) * 256;
        let buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("test readback"),
            size: u64::from(row * height),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = gpu.device.create_command_encoder(&Default::default());
        encoder.copy_texture_to_buffer(
            texture.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(row),
                    rows_per_image: Some(height),
                },
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        gpu.queue.submit(Some(encoder.finish()));
        buffer.slice(..).map_async(wgpu::MapMode::Read, |_| {});
        gpu.device
            .poll(wgpu::PollType::wait_indefinitely())
            .expect("GPU poll");
        let padded = buffer.slice(..).get_mapped_range().to_vec();
        padded
            .chunks(row as usize)
            .flat_map(|r| r[..(width * 4) as usize].to_vec())
            .collect()
    }

    /// A pipeline that renders the whole field through `layout`.
    fn panorama_pipeline(
        gpu: &GpuContext,
        cal: &MatchCalibration,
        layout: crate::projection::PanoramaLayout,
    ) -> StitchPipeline {
        StitchPipeline::with_gpu(
            gpu.clone(),
            cal.clone(),
            ViewportConfig {
                width: layout.width,
                height: layout.height,
                rig_tilt: cal.rig_tilt as f32,
                rig_roll: cal.rig_roll as f32,
                color_match: false,
                panorama: Some(layout),
                ..ViewportConfig::default()
            },
            512,
            288,
            wgpu::TextureFormat::Bgra8Unorm,
            InputFormat::Yuv420p,
        )
        .expect("pipeline")
    }

    /// The brightness-weighted centre of what is lit within 12 px of
    /// `near` (pixel coordinates, centres at +0.5).
    fn lit_centre(bgra: &[u8], width: u32, near: (f64, f64)) -> Option<(f64, f64)> {
        let height = bgra.len() as u32 / 4 / width;
        let (mut sum, mut sx, mut sy) = (0.0, 0.0, 0.0);
        let (x0, y0) = (near.0 as i64 - 12, near.1 as i64 - 12);
        for y in y0.max(0)..(y0 + 25).min(i64::from(height)) {
            for x in x0.max(0)..(x0 + 25).min(i64::from(width)) {
                let i = ((y as u32 * width + x as u32) * 4) as usize;
                let lit =
                    (f64::from(bgra[i]) + f64::from(bgra[i + 1]) + f64::from(bgra[i + 2])) / 3.0;
                if lit > 40.0 {
                    sum += lit;
                    sx += lit * (x as f64 + 0.5);
                    sy += lit * (y as f64 + 0.5);
                }
            }
        }
        (sum > 0.0).then(|| (sx / sum, sy / sum))
    }

    #[test]
    #[ignore = "requires a GPU (wgpu adapter init)"]
    fn dots_land_where_the_mapping_says() {
        use crate::detect::detector::CameraId;
        use crate::projection::{PanoramaBasis, PanoramaDetail, PanoramaLayout};
        let gpu = GpuContext::new_blocking().expect("GPU init");
        let black = dots(&[]);
        // Away from the seam: the left camera's left side, the right's right side.
        let (left_dot, right_dot) = ((150, 150), (360, 120));
        for (tilt, roll) in [(0.0, 0.0), (0.05, -0.03)] {
            let cal = wide_calibration(tilt, roll);
            let basis = PanoramaBasis::new(&cal);
            for detail in [PanoramaDetail::Half, PanoramaDetail::Full] {
                let layout = PanoramaLayout::for_field(&cal, detail);
                let pipeline = panorama_pipeline(&gpu, &cal, layout);
                for (camera, dot) in [(CameraId::Left, left_dot), (CameraId::Right, right_dot)] {
                    let lit = dots(&[dot]);
                    let (l, r) = match camera {
                        CameraId::Left => (&lit, &black),
                        CameraId::Right => (&black, &lit),
                    };
                    let image = drawn_sized(&gpu, layout.width, layout.height, |view| {
                        pipeline
                            .render_to_view(&planes(l), &planes(r), 0.0, 0.0, view)
                            .expect("render");
                    });
                    let (nx, ny) = (
                        (f64::from(dot.0) + 0.5) / 512.0,
                        (f64::from(dot.1) + 0.5) / 288.0,
                    );
                    let expected = layout
                        .camera_to_pixel(&basis, &cal, camera, nx, ny)
                        .expect("the dot is in the picture");
                    let found = lit_centre(&image, layout.width, expected).unwrap_or_else(|| {
                        panic!("no dot near {expected:?} ({camera:?} {detail:?} tilt {tilt})")
                    });
                    let miss =
                        ((found.0 - expected.0).powi(2) + (found.1 - expected.1).powi(2)).sqrt();
                    // Measured 0.02-0.11 px; a 0.004 rad slip is 0.5 px here.
                    assert!(
                        miss < 0.25,
                        "{camera:?} {detail:?} tilt {tilt} roll {roll}: found {found:?}, mapping says {expected:?}"
                    );
                }
            }
        }
    }

    #[test]
    #[ignore = "requires a GPU (wgpu adapter init)"]
    fn half_matches_full_box_filtered() {
        use crate::projection::{PanoramaDetail, PanoramaLayout};
        let gpu = GpuContext::new_blocking().expect("GPU init");
        let cal = wide_calibration(0.0, 0.0);
        let half = PanoramaLayout::for_field(&cal, PanoramaDetail::Half);
        let full = PanoramaLayout {
            width: half.width * 2,
            height: half.height * 2,
            px_per_rad: half.px_per_rad * 2.0,
            samples: 1,
            ..half
        };
        let single = PanoramaLayout { samples: 1, ..half };
        let frame = busy();
        let render = |layout: PanoramaLayout| {
            let pipeline = panorama_pipeline(&gpu, &cal, layout);
            drawn_sized(&gpu, layout.width, layout.height, |view| {
                pipeline
                    .render_to_view(&planes(&frame), &planes(&frame), 0.0, 0.0, view)
                    .expect("render");
            })
        };
        let (fine, smooth, aliased) = (render(full), render(half), render(single));
        let (w, h) = (half.width as usize, half.height as usize);
        let mut boxed = vec![0.0f64; w * h * 4];
        for y in 0..h {
            for x in 0..w {
                for c in 0..4 {
                    let at = |dx: usize, dy: usize| {
                        f64::from(fine[((2 * y + dy) * 2 * w + 2 * x + dx) * 4 + c])
                    };
                    boxed[(y * w + x) * 4 + c] = (at(0, 0) + at(1, 0) + at(0, 1) + at(1, 1)) / 4.0;
                }
            }
        }
        let error = |image: &[u8]| {
            image
                .iter()
                .zip(&boxed)
                .map(|(a, b)| (f64::from(*a) - b).abs())
                .sum::<f64>()
                / boxed.len() as f64
        };
        let (smooth_error, aliased_error) = (error(&smooth), error(&aliased));
        assert!(
            smooth_error <= 2.0,
            "half differs from full box-filtered by {smooth_error:.2}"
        );
        assert!(
            aliased_error > smooth_error * 2.0,
            "one sample per pixel ({aliased_error:.2}) should be visibly worse than 2×2 ({smooth_error:.2})"
        );
    }

    #[test]
    #[ignore = "requires a GPU (wgpu adapter init)"]
    fn render_uploaded_to_view_draws_the_last_upload() {
        let gpu = GpuContext::new_blocking().expect("GPU init");
        let pipeline = StitchPipeline::with_gpu(
            gpu.clone(),
            gpu_test_calibration(),
            ViewportConfig {
                width: SIDE,
                height: SIDE,
                ..ViewportConfig::default()
            },
            SIDE,
            SIDE,
            wgpu::TextureFormat::Bgra8Unorm,
            InputFormat::Yuv420p,
        )
        .expect("pipeline");
        let (first, second) = (ramp(0), ramp(90));
        let uploaded = drawn(&gpu, |view| {
            pipeline
                .render_to_view(&planes(&first), &planes(&first), 0.0, 0.0, view)
                .expect("render");
        });
        let again = drawn(&gpu, |view| {
            pipeline.render_uploaded_to_view(0.0, 0.0, view)
        });
        assert_eq!(
            again, uploaded,
            "the same frame and pose, without an upload"
        );
        let panned = drawn(&gpu, |view| {
            pipeline.render_uploaded_to_view(0.2, 0.0, view)
        });
        assert_ne!(panned, uploaded, "a new pose draws the picture again");

        let next = drawn(&gpu, |view| {
            pipeline
                .render_to_view(&planes(&second), &planes(&second), 0.2, 0.0, view)
                .expect("render");
        });
        assert_ne!(next, panned, "the second frame differs from the first");
        let next_again = drawn(&gpu, |view| {
            pipeline.render_uploaded_to_view(0.2, 0.0, view)
        });
        assert_eq!(next_again, next, "it draws the frame uploaded last");
    }
}
