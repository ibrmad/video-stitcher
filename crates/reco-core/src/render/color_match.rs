//! Automatic exposure and colour matching between the two cameras.
//!
//! Cameras on auto exposure and auto white balance each meter their own
//! half of the scene, so their pictures rarely agree: one half of the
//! panorama comes out brighter or tinted, which shows most in an even sky.
//! This module measures both cameras where they see the same scene (the
//! overlap around the seam) and gives each camera one tone curve per
//! colour channel so that both meet in the middle.
//!
//! ## Measurement
//!
//! Every [`MEASURE_INTERVAL`] frames both planes are drawn on their own
//! into a small offscreen target from a fixed view centred on the seam,
//! with no blending and no colour correction. Pixels that both cameras
//! cover are the overlap. The readback is mapped asynchronously and
//! picked up on a later frame, so the stitch does not wait for it (except
//! once, for the very first measurement, so the first frame is already
//! matched).
//!
//! ## Matching
//!
//! Pixels both cameras show at the same spot are compared channel by
//! channel in [`BANDS`] brightness bands (by the average of the two
//! values). In each band the two cameras' mean values are both mapped to
//! their average, and the curves run through those points (and through
//! black and white). Banding by brightness lets the correction differ
//! between, say, grass and sky: the cameras' tone curves compress
//! highlights, so the difference between them depends on brightness, and
//! a single gain per channel fixed the grass while unbalancing the sky.
//! Per-channel histogram matching fails outright on this kind of picture,
//! because sky and grass share each channel's range and the ranks of the
//! two cameras' values do not line up.
//!
//! Only the seam counts: the lenses darken toward their edges, so across
//! the overlap the ratio between the cameras drifts (on GoPro footage by a
//! quarter from one side to the other) and what has to match is what
//! meets at the seam. Pixels are weighed by their distance to the middle
//! of the seam fade, which the measurement pass writes into alpha. Values
//! near black or clipped in either camera are left out. The bands are
//! averaged over time and the curves eased in, so a player crossing the
//! seam or an exposure step does not make the picture pump.
//!
//! Set `RECO_COLOR_MATCH=0` to turn it off.

use super::renderer::GpuUniforms;
use crate::gpu::GpuContext;

use std::sync::mpsc;

/// Number of evenly spaced points on each tone curve over `[0, 1]`.
///
/// Must match the `tone_curve` array length in `fisheye.wgsl`.
pub(crate) const TONE_CURVE_KNOTS: usize = 32;

/// One tone curve per RGB channel (`xyz`, `w` unused), sampled at
/// [`TONE_CURVE_KNOTS`] evenly spaced input values.
pub(crate) type ToneCurve = [[f32; 4]; TONE_CURVE_KNOTS];

/// Size of the offscreen measurement target. The width keeps rows at
/// the 256-byte copy alignment wgpu requires.
pub(crate) const MEASURE_WIDTH: u32 = 256;
pub(crate) const MEASURE_HEIGHT: u32 = 256;
/// Vertical field of view of the measurement view, centred on the seam.
pub(crate) const MEASURE_FOV_DEGREES: f32 = 70.0;
/// Frames between measurements.
const MEASURE_INTERVAL: u32 = 5;
/// Weight of each new measurement in the running bands (about two
/// seconds of memory at 30 fps with one measurement every five frames).
const BAND_WEIGHT: f64 = 0.1;
/// Fraction of the remaining distance the applied curves move each frame.
const CURVE_EASING: f32 = 0.2;
/// Fewest overlap pixels near the seam a measurement needs to count.
const MIN_SAMPLES: u32 = 500;
/// Brightness bands each channel is compared in.
const BANDS: usize = 16;
/// Smallest share of the measured weight a band needs to be used.
const MIN_BAND_SHARE: f64 = 0.005;
/// Channel values both cameras must have for the channel to count:
/// darker is mostly noise, brighter may be clipped.
const USABLE: std::ops::RangeInclusive<u8> = 6..=249;
/// How far (in plane widths) beyond either side of the seam fade pixels
/// still count, with a weight falling off linearly to zero.
const SEAM_MARGIN: f64 = 0.1;
/// Largest change the curves may make to any value.
const MAX_CORRECTION: f64 = 0.2;

/// The tone curve that leaves every value unchanged.
pub(crate) fn identity_curve() -> ToneCurve {
    std::array::from_fn(|k| {
        let x = k as f32 / (TONE_CURVE_KNOTS - 1) as f32;
        [x, x, x, 0.0]
    })
}

/// Both cameras' values near the seam, per channel and brightness band.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct OverlapBands {
    /// `[channel][band]` = `[weight, weight * left, weight * right]`
    /// summed over the band's pixels (values 0..255), all divided by the
    /// measurement's total weight so measurements average fairly.
    bands: [[[f64; 3]; BANDS]; 3],
}

impl OverlapBands {
    /// Bands of two RGBA8 measurement images of the same view, or `None`
    /// when too few pixels near the seam are covered by both. Alpha 0 is
    /// uncovered; otherwise it holds the plane's horizontal position (see
    /// [`plane_position`]). Each pixel is weighted by the right plane's
    /// distance to the middle of the seam fade, which runs over
    /// `0..blend_width` of that plane.
    pub(crate) fn from_rgba(left: &[u8], right: &[u8], blend_width: f64) -> Option<Self> {
        let reach = blend_width / 2.0 + SEAM_MARGIN;
        let mut bands = [[[0.0f64; 3]; BANDS]; 3];
        let mut total = 0.0;
        let mut samples = 0u32;
        for (l, r) in left.chunks_exact(4).zip(right.chunks_exact(4)) {
            if l[3] == 0 || r[3] == 0 {
                continue;
            }
            let weight = 1.0 - (plane_position(r[3]) - blend_width / 2.0).abs() / reach;
            if weight <= 0.0 {
                continue;
            }
            total += weight;
            samples += 1;
            for (c, channel) in bands.iter_mut().enumerate() {
                let (lv, rv) = (l[c], r[c]);
                if USABLE.contains(&lv) && USABLE.contains(&rv) {
                    let band = &mut channel[(usize::from(lv) + usize::from(rv)) * BANDS / 512];
                    band[0] += weight;
                    band[1] += weight * f64::from(lv);
                    band[2] += weight * f64::from(rv);
                }
            }
        }
        if samples < MIN_SAMPLES {
            return None;
        }
        for value in bands.iter_mut().flatten().flatten() {
            *value /= total;
        }
        Some(Self { bands })
    }

    /// Move these bands toward `newer` by `weight` (0 = keep, 1 = replace).
    pub(crate) fn blend(&mut self, newer: &Self, weight: f64) {
        for (old, new) in self
            .bands
            .iter_mut()
            .flatten()
            .flatten()
            .zip(newer.bands.iter().flatten().flatten())
        {
            *old += (new - *old) * weight;
        }
    }

    /// Tone curves `[left, right]` that take each band's mean value in
    /// either camera to the average of the two.
    pub(crate) fn midway_curves(&self) -> [ToneCurve; 2] {
        let mut curves = [identity_curve(); 2];
        for (c, channel) in self.bands.iter().enumerate() {
            // (left, right) mean values in [0, 1] of each usable band.
            let means: Vec<(f64, f64)> = channel
                .iter()
                .filter(|band| band[0] >= MIN_BAND_SHARE)
                .map(|band| (band[1] / band[0] / 255.0, band[2] / band[0] / 255.0))
                .collect();
            for (side, curve) in curves.iter_mut().enumerate() {
                let mut points: Vec<(f64, f64)> = means
                    .iter()
                    .map(|&(l, r)| (if side == 0 { l } else { r }, (l + r) / 2.0))
                    .collect();
                points.sort_by(|a, b| a.0.total_cmp(&b.0));
                let (xs, ys) = rising_through(&points);
                for (k, knot) in curve.iter_mut().enumerate() {
                    let x = k as f64 / (TONE_CURVE_KNOTS - 1) as f64;
                    knot[c] = interpolate(&xs, &ys, x)
                        .clamp(x - MAX_CORRECTION, x + MAX_CORRECTION)
                        .clamp(0.0, 1.0) as f32;
                }
            }
        }
        curves
    }
}

/// The points (sorted by x) with black and white added at the ends,
/// dropping any that would not rise in both x and y.
fn rising_through(points: &[(f64, f64)]) -> (Vec<f64>, Vec<f64>) {
    let (mut xs, mut ys) = (vec![0.0], vec![0.0]);
    for &(x, y) in points {
        let (last_x, last_y) = (xs[xs.len() - 1], ys[ys.len() - 1]);
        if x > last_x + 1e-3 && y >= last_y && x < 1.0 {
            xs.push(x);
            ys.push(y);
        }
    }
    xs.push(1.0);
    ys.push(1.0);
    (xs, ys)
}

/// Piecewise-linear value at `x` of the points `(xs, ys)`; `xs` rises
/// from 0 to 1.
fn interpolate(xs: &[f64], ys: &[f64], x: f64) -> f64 {
    let j = xs.partition_point(|&v| v <= x).clamp(1, xs.len() - 1);
    let (xa, ya, xb, yb) = (xs[j - 1], ys[j - 1], xs[j], ys[j]);
    ya + (yb - ya) * (x - xa) / (xb - xa)
}

/// The plane's horizontal texture position (`uv.x` in `fisheye.wgsl`,
/// -0.5 to 1.5) from a covered measurement pixel's alpha.
fn plane_position(alpha: u8) -> f64 {
    f64::from(alpha - 1) / 254.0 * 2.0 - 0.5
}

/// Move `current` a fraction `amount` of the way to `target`.
fn ease(current: &mut ToneCurve, target: &ToneCurve, amount: f32) {
    for (c, t) in current.iter_mut().flatten().zip(target.iter().flatten()) {
        *c += (t - *c) * amount;
    }
}

/// What a frame needs to take a measurement: the stitch's vertex buffer,
/// both cameras' current texture bind groups, and the measurement view's
/// uniforms for each camera (no colour correction, no seam fade).
pub(crate) struct MeasureFrame<'a> {
    pub vertex_buffer: &'a wgpu::Buffer,
    pub textures: [&'a wgpu::BindGroup; 2],
    pub uniforms: [GpuUniforms; 2],
    /// The stitch's seam fade width, to know where the seam is.
    pub blend_width: f32,
}

/// Measurement resources and matching state for one renderer.
pub(crate) struct ColorMatch {
    enabled: bool,
    pipeline: wgpu::RenderPipeline,
    targets: [wgpu::TextureView; 2],
    target_textures: [wgpu::Texture; 2],
    uniform_buffers: [wgpu::Buffer; 2],
    uniform_bind_groups: [wgpu::BindGroup; 2],
    /// Both measurement images, left then right.
    readback: wgpu::Buffer,
    map_tx: mpsc::Sender<Result<(), wgpu::BufferAsyncError>>,
    map_rx: mpsc::Receiver<Result<(), wgpu::BufferAsyncError>>,
    /// A measurement was submitted and its readback is being mapped.
    pending: bool,
    /// Seam fade width the pending measurement was taken with.
    pending_blend_width: f64,
    frames_since_measure: u32,
    bands: Option<OverlapBands>,
    target: Option<[ToneCurve; 2]>,
    applied: Option<[ToneCurve; 2]>,
}

impl ColorMatch {
    /// `pipeline` draws one plane like the stitch does, into an
    /// `Rgba8Unorm` target without blending.
    pub(crate) fn new(
        device: &wgpu::Device,
        pipeline: wgpu::RenderPipeline,
        uniform_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let target = |side: &str| {
            device.create_texture(&wgpu::TextureDescriptor {
                label: Some(&format!("color_match_{side}")),
                size: wgpu::Extent3d {
                    width: MEASURE_WIDTH,
                    height: MEASURE_HEIGHT,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            })
        };
        let target_textures = [target("left"), target("right")];
        let targets = target_textures
            .each_ref()
            .map(|t| t.create_view(&wgpu::TextureViewDescriptor::default()));
        let uniform_buffers = ["left", "right"].map(|side| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(&format!("color_match_{side}_uniforms")),
                size: std::mem::size_of::<GpuUniforms>() as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            })
        });
        let uniform_bind_groups = uniform_buffers.each_ref().map(|buffer| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("color_match_uniform_bg"),
                layout: uniform_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: buffer.as_entire_binding(),
                }],
            })
        });
        let readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("color_match_readback"),
            size: 2 * u64::from(MEASURE_WIDTH * MEASURE_HEIGHT * 4),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let (map_tx, map_rx) = mpsc::channel();
        Self {
            enabled: std::env::var("RECO_COLOR_MATCH").map_or(true, |v| v != "0"),
            pipeline,
            targets,
            target_textures,
            uniform_buffers,
            uniform_bind_groups,
            readback,
            map_tx,
            map_rx,
            pending: false,
            pending_blend_width: 0.0,
            frames_since_measure: MEASURE_INTERVAL,
            bands: None,
            target: None,
            applied: None,
        }
    }

    /// Advance one frame: collect a finished measurement, start a new one
    /// when due, and return the curves `[left, right]` to draw this frame
    /// with (`None` = no correction yet, or matching is off).
    ///
    /// `frame` is only called when a measurement is taken. A measurement
    /// is submitted on its own, before the caller submits the frame; it
    /// reads the same input textures, which hold this frame's pictures by
    /// then (queued uploads run ahead of any later submission).
    pub(crate) fn next_frame<'a>(
        &mut self,
        gpu: &GpuContext,
        frame: impl FnOnce() -> MeasureFrame<'a>,
    ) -> Option<[ToneCurve; 2]> {
        if !self.enabled {
            return None;
        }
        if self.pending {
            self.collect(gpu, false);
        }
        self.frames_since_measure += 1;
        if !self.pending && self.frames_since_measure >= MEASURE_INTERVAL {
            self.measure(gpu, frame());
            self.frames_since_measure = 0;
            if self.target.is_none() {
                self.collect(gpu, true);
            }
        }
        let target = self.target.as_ref()?;
        let applied = self.applied.get_or_insert(*target);
        for (a, t) in applied.iter_mut().zip(target) {
            ease(a, t, CURVE_EASING);
        }
        Some(*applied)
    }

    fn measure(&mut self, gpu: &GpuContext, frame: MeasureFrame<'_>) {
        for (buffer, uniforms) in self.uniform_buffers.iter().zip(&frame.uniforms) {
            gpu.queue
                .write_buffer(buffer, 0, bytemuck::bytes_of(uniforms));
        }
        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("color_match_measure"),
            });
        for side in 0..2 {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("color_match_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.targets[side],
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_vertex_buffer(0, frame.vertex_buffer.slice(..));
            pass.set_bind_group(0, frame.textures[side], &[]);
            pass.set_bind_group(1, &self.uniform_bind_groups[side], &[]);
            pass.draw(0..6, 0..1);
        }
        for (side, texture) in self.target_textures.iter().enumerate() {
            encoder.copy_texture_to_buffer(
                wgpu::TexelCopyTextureInfo {
                    texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::TexelCopyBufferInfo {
                    buffer: &self.readback,
                    layout: wgpu::TexelCopyBufferLayout {
                        offset: side as u64 * u64::from(MEASURE_WIDTH * MEASURE_HEIGHT * 4),
                        bytes_per_row: Some(MEASURE_WIDTH * 4),
                        rows_per_image: Some(MEASURE_HEIGHT),
                    },
                },
                wgpu::Extent3d {
                    width: MEASURE_WIDTH,
                    height: MEASURE_HEIGHT,
                    depth_or_array_layers: 1,
                },
            );
        }
        gpu.queue.submit(Some(encoder.finish()));
        let tx = self.map_tx.clone();
        self.readback
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |result| {
                let _ = tx.send(result);
            });
        self.pending = true;
        self.pending_blend_width = f64::from(frame.blend_width);
    }

    /// Take a finished measurement into the running bands. With
    /// `wait`, block until the GPU has finished it.
    fn collect(&mut self, gpu: &GpuContext, wait: bool) {
        let poll = if wait {
            wgpu::PollType::wait_indefinitely()
        } else {
            wgpu::PollType::Poll
        };
        if gpu.device.poll(poll).is_err() {
            return;
        }
        let mapped = match self.map_rx.try_recv() {
            Ok(result) => result.is_ok(),
            Err(mpsc::TryRecvError::Empty) => return,
            Err(mpsc::TryRecvError::Disconnected) => false,
        };
        self.pending = false;
        if !mapped {
            return;
        }
        let measured = {
            let data = self.readback.slice(..).get_mapped_range();
            let (left, right) = data.split_at(data.len() / 2);
            OverlapBands::from_rgba(left, right, self.pending_blend_width)
        };
        self.readback.unmap();
        let Some(measured) = measured else {
            log::debug!("Colour matching: too little overlap at the seam in this frame, skipped");
            return;
        };
        if let Some(bands) = self.bands.as_mut() {
            bands.blend(&measured, BAND_WEIGHT);
        } else {
            self.bands = Some(measured);
        }
        let Some(curves) = self.bands.as_ref().map(OverlapBands::midway_curves) else {
            return;
        };
        // What the curves do to a mid grey and to a bright value (sky).
        let [mid, bright] = [TONE_CURVE_KNOTS / 2, TONE_CURVE_KNOTS * 7 / 8];
        let summary = format!(
            "{:.2} -> {:.3?} / {:.3?}, {:.2} -> {:.3?} / {:.3?} (left / right)",
            mid as f32 / (TONE_CURVE_KNOTS - 1) as f32,
            &curves[0][mid][..3],
            &curves[1][mid][..3],
            bright as f32 / (TONE_CURVE_KNOTS - 1) as f32,
            &curves[0][bright][..3],
            &curves[1][bright][..3],
        );
        if self.target.is_none() {
            log::info!("Colour matching on: {summary}");
        } else {
            log::debug!("Colour matching: {summary}");
        }
        self.target = Some(curves);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BLEND: f64 = 0.05;

    /// Measurement alpha for a pixel at plane position `u`.
    fn alpha_at(u: f64) -> u8 {
        (1.0 + 254.0 * ((u + 0.5) / 2.0)).round() as u8
    }

    /// A measurement image of `n` covered pixels at the middle of the
    /// seam fade; pixel `i` has colour `f(i)`.
    fn image(n: usize, f: impl Fn(usize) -> [u8; 3]) -> Vec<u8> {
        (0..n)
            .flat_map(|i| {
                let [r, g, b] = f(i);
                [r, g, b, alpha_at(BLEND / 2.0)]
            })
            .collect()
    }

    fn grey(v: f64) -> [u8; 3] {
        let v = v.round().clamp(0.0, 255.0) as u8;
        [v, v, v]
    }

    fn curves(left: &[u8], right: &[u8]) -> [ToneCurve; 2] {
        OverlapBands::from_rgba(left, right, BLEND)
            .unwrap()
            .midway_curves()
    }

    /// The curve's value for `channel` at `v` (0..255), in 0..255.
    fn eval(curve: &ToneCurve, channel: usize, v: f64) -> f64 {
        let pos = v / 255.0 * (TONE_CURVE_KNOTS - 1) as f64;
        let k = (pos.floor() as usize).min(TONE_CURVE_KNOTS - 2);
        let t = pos - k as f64;
        let (a, b) = (
            f64::from(curve[k][channel]),
            f64::from(curve[k + 1][channel]),
        );
        (a + (b - a) * t) * 255.0
    }

    /// A scene of values 20..230 seen by two cameras that render scene
    /// value `v` as `left(v)` and `right(v)`.
    fn scene(left: impl Fn(f64) -> f64, right: impl Fn(f64) -> f64) -> (Vec<u8>, Vec<u8>) {
        let value = |i: usize| 20.0 + (i % 211) as f64;
        (
            image(21_100, |i| grey(left(value(i)))),
            image(21_100, |i| grey(right(value(i)))),
        )
    }

    #[test]
    fn identical_cameras_get_identity_curves() {
        let (left, right) = scene(|v| v, |v| v);
        for curve in curves(&left, &right) {
            for (k, knot) in curve.iter().enumerate() {
                let x = k as f32 / (TONE_CURVE_KNOTS - 1) as f32;
                assert!(
                    knot[..3].iter().all(|v| (v - x).abs() < 0.005),
                    "knot {k}: {knot:?}"
                );
            }
        }
    }

    #[test]
    fn brighter_camera_is_pulled_down_and_darker_one_up_to_meet() {
        let (left, right) = scene(|v| v * 1.15, |v| v);
        let [l, r] = curves(&left, &right);
        for v in [40.0, 100.0, 180.0] {
            let (met_left, met_right) = (eval(&l, 1, v * 1.15), eval(&r, 1, v));
            assert!(
                (met_left - met_right).abs() < 2.5,
                "scene {v}: {met_left} vs {met_right}"
            );
            assert!(met_left < v * 1.15 && met_right > v);
        }
        for curve in [l, r] {
            assert_eq!(curve[0][1], 0.0, "black stays black");
            assert_eq!(curve[TONE_CURVE_KNOTS - 1][1], 1.0, "white stays white");
        }
    }

    #[test]
    fn correction_follows_brightness() {
        // The left camera is 20% brighter in the shadows, but both cameras
        // compress their highlights to the same values, as at the end of
        // a match where the sky agrees and the grass does not.
        let left = |v: f64| {
            if v < 120.0 {
                v * 1.2
            } else {
                144.0 + (v - 120.0) * 0.8
            }
        };
        let (l_img, r_img) = scene(left, |v| v);
        let [l, r] = curves(&l_img, &r_img);
        // Shadows meet in the middle ...
        let (dark_l, dark_r) = (eval(&l, 0, 60.0 * 1.2), eval(&r, 0, 60.0));
        assert!((dark_l - dark_r).abs() < 2.5, "{dark_l} vs {dark_r}");
        // ... and so do the highlights, which a single gain fitted to the
        // shadows would have pulled apart.
        let (hi_l, hi_r) = (eval(&l, 0, left(220.0)), eval(&r, 0, 220.0));
        assert!((hi_l - hi_r).abs() < 2.5, "{hi_l} vs {hi_r}");
    }

    #[test]
    fn white_balance_difference_is_matched_per_channel() {
        // Left camera warmer: red 15% up, blue 15% down.
        let value = |i: usize| 30.0 + (i % 181) as f64;
        let left = image(18_100, |i| {
            let v = value(i);
            [(v * 1.15) as u8, v as u8, (v / 1.15) as u8]
        });
        let right = image(18_100, |i| grey(value(i)));
        let [l, r] = curves(&left, &right);
        let v = 120.0;
        assert!((eval(&l, 0, v * 1.15) - eval(&r, 0, v)).abs() < 2.5, "red");
        assert!((eval(&l, 2, v / 1.15) - eval(&r, 2, v)).abs() < 2.5, "blue");
        assert!(
            (eval(&l, 1, v) - v).abs() < 1.0 && (eval(&r, 1, v) - v).abs() < 1.0,
            "green"
        );
    }

    #[test]
    fn uncovered_dark_and_clipped_values_are_ignored() {
        let left = image(10_000, |i| match i % 4 {
            0 => [255, 100, 100], // red clipped in the left camera only
            1 => [100, 2, 100],   // green near black in the left camera only
            2 => grey(200.0),     // not covered by the right camera, see below
            _ => grey(100.0),
        });
        let mut right = image(10_000, |_| grey(100.0));
        for px in (2..10_000).step_by(4) {
            right[px * 4 + 3] = 0;
        }
        for curve in curves(&left, &right) {
            for c in 0..3 {
                assert!((eval(&curve, c, 100.0) - 100.0).abs() < 0.5, "channel {c}");
            }
        }
    }

    #[test]
    fn only_pixels_near_the_seam_count() {
        // The lenses darken toward their edges: away from the seam the
        // right camera reads darker than the left, at the seam they agree.
        let n = 10_000;
        let left = image(n, |_| grey(100.0));
        let mut right = image(n, |_| grey(100.0));
        for px in 0..n / 2 {
            right[px * 4..px * 4 + 4].copy_from_slice(&[60, 60, 60, alpha_at(-0.3)]);
        }
        for curve in curves(&left, &right) {
            assert!((eval(&curve, 0, 100.0) - 100.0).abs() < 0.5);
        }
    }

    #[test]
    fn position_survives_the_alpha_round_trip() {
        for u in [-0.5, -0.2, 0.0, 0.025, 0.3, 1.5] {
            assert!((plane_position(alpha_at(u)) - u).abs() < 0.005, "{u}");
        }
    }

    #[test]
    fn too_small_an_overlap_is_not_trusted() {
        let img = image(MIN_SAMPLES as usize - 1, |_| grey(100.0));
        assert!(OverlapBands::from_rgba(&img, &img, BLEND).is_none());
    }

    #[test]
    fn curves_rise_and_stay_within_the_correction_limit() {
        let (left, right) = scene(|v| v, |v| v * 0.4);
        for curve in curves(&left, &right) {
            for k in 0..TONE_CURVE_KNOTS {
                let x = k as f64 / (TONE_CURVE_KNOTS - 1) as f64;
                assert!((f64::from(curve[k][0]) - x).abs() <= MAX_CORRECTION + 1e-6);
                if k > 0 {
                    assert!(curve[k][0] >= curve[k - 1][0], "falls at knot {k}");
                }
            }
        }
    }

    #[test]
    fn blending_moves_part_of_the_way() {
        let (dark, bright) = (
            image(10_000, |_| grey(51.0)),
            image(10_000, |_| grey(204.0)),
        );
        let mut running = OverlapBands::from_rgba(&dark, &dark, BLEND).unwrap();
        running.blend(
            &OverlapBands::from_rgba(&bright, &bright, BLEND).unwrap(),
            0.25,
        );
        let band = |v: usize| running.bands[0][v * 2 * BANDS / 512][0];
        assert!((band(51) - 0.75).abs() < 1e-9 && (band(204) - 0.25).abs() < 1e-9);
    }
}
