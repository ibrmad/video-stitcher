//! The whole-field panorama: a fixed Mercator picture of the field in
//! Reco's own yaw/pitch.
//!
//! Columns are even in yaw (yaw decreases to the right, as everywhere in
//! Reco) and rows follow the Mercator ordinate `asinh(tan(pitch))`, so
//! shapes keep their proportions from the far touchline down to the
//! players near the camera. Pixel `(i, j)` has its centre at
//! `(i + 0.5, j + 0.5)`; `(0, 0)` is the top-left corner of the picture.

use nalgebra::{Point3, Unit, UnitQuaternion, Vector3};
use serde::{Deserialize, Serialize};

use crate::calibration::MatchCalibration;
use crate::detect::detector::CameraId;
use crate::render::scene::SceneGeometry;

/// Margin left and right of the field outline (radians, 2°).
const YAW_MARGIN: f64 = 2.0 * std::f64::consts::PI / 180.0;
/// Margin above and below the field outline (radians, 1.5°).
const PITCH_MARGIN: f64 = 1.5 * std::f64::consts::PI / 180.0;
/// Without an outline, the picture stops this far above the horizon (15°).
const FALLBACK_TOP: f64 = 15.0 * std::f64::consts::PI / 180.0;
/// Points sampled along each outline edge.
const OUTLINE_STEPS: u32 = 16;
/// Rows between the seam's points.
const SEAM_ROW_STEP: u32 = 32;

/// The Mercator ordinate of `pitch` (radians): `asinh(tan(pitch))`.
pub fn mercator(pitch: f64) -> f64 {
    pitch.tan().asinh()
}

/// The pitch (radians) at Mercator ordinate `m`: `atan(sinh(m))`.
pub fn inverse_mercator(m: f64) -> f64 {
    m.sinh().atan()
}

/// How much detail a whole-field panorama keeps.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PanoramaDetail {
    /// Half the lenses' density, rendered with 2×2 samples per pixel.
    #[default]
    Half,
    /// The lenses' density at the image centre: every camera pixel kept.
    Full,
}

/// The angles a panorama spans, in radians. `yaw_left > yaw_right`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PanoramaBounds {
    /// Yaw at the picture's left edge.
    pub yaw_left: f64,
    /// Yaw at the picture's right edge.
    pub yaw_right: f64,
    /// Pitch at the picture's top edge.
    pub pitch_top: f64,
    /// Pitch at the picture's bottom edge.
    pub pitch_bottom: f64,
}

/// A whole-field panorama's size and its mapping between pixels and
/// Reco's yaw/pitch.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct PanoramaLayout {
    /// Width in pixels (a multiple of 16).
    pub width: u32,
    /// Height in pixels (a multiple of 8).
    pub height: u32,
    /// Pixels per radian, along the horizon and down the picture's centre.
    pub px_per_rad: f64,
    /// Yaw at the left edge (radians).
    pub yaw_left: f64,
    /// Pitch at the top edge (radians).
    pub pitch_top: f64,
    /// Samples per pixel along each axis (2 at Half, 1 at Full).
    pub samples: u32,
}

impl PanoramaLayout {
    /// The widest picture the encoders take.
    pub const MAX_WIDTH: u32 = 8192;

    /// Size a panorama over `bounds` at `detail`, for lenses with focal
    /// length `fx` (pixels per radian at the image centre).
    pub fn fit(bounds: PanoramaBounds, detail: PanoramaDetail, fx: f64) -> Self {
        let (mut px_per_rad, samples) = match detail {
            PanoramaDetail::Half => (fx / 2.0, 2),
            PanoramaDetail::Full => (fx, 1),
        };
        let span = bounds.yaw_left - bounds.yaw_right;
        if span * px_per_rad > f64::from(Self::MAX_WIDTH) {
            px_per_rad = f64::from(Self::MAX_WIDTH) / span;
        }
        let rows = (mercator(bounds.pitch_top) - mercator(bounds.pitch_bottom)) * px_per_rad;
        Self {
            width: ceil_to(span * px_per_rad, 16),
            height: ceil_to(rows, 8),
            px_per_rad,
            yaw_left: bounds.yaw_left,
            pitch_top: bounds.pitch_top,
            samples,
        }
    }

    /// Yaw at the right edge (radians).
    pub fn yaw_right(&self) -> f64 {
        self.yaw_left - f64::from(self.width) / self.px_per_rad
    }

    /// Pitch at the bottom edge (radians).
    pub fn pitch_bottom(&self) -> f64 {
        self.pixel_to_yaw_pitch(0.0, f64::from(self.height)).1
    }

    /// The yaw and pitch (radians) at pixel coordinates `(x, y)`.
    pub fn pixel_to_yaw_pitch(&self, x: f64, y: f64) -> (f64, f64) {
        let yaw = self.yaw_left - x / self.px_per_rad;
        let pitch = inverse_mercator(mercator(self.pitch_top) - y / self.px_per_rad);
        (yaw, pitch)
    }

    /// The pixel coordinates of yaw and pitch (radians).
    pub fn yaw_pitch_to_pixel(&self, yaw: f64, pitch: f64) -> (f64, f64) {
        let x = (self.yaw_left - yaw) * self.px_per_rad;
        let y = (mercator(self.pitch_top) - mercator(pitch)) * self.px_per_rad;
        (x, y)
    }
}

/// The scene a calibration describes: its two planes, sized by the left
/// camera's aspect, and the virtual camera's eye.
pub(crate) fn scene_of(calibration: &MatchCalibration) -> SceneGeometry {
    let aspect = calibration.left.width as f32 / calibration.left.height as f32;
    SceneGeometry::from_layout_with_aspect(&calibration.layout, aspect)
}

/// Directions in Reco's yaw/pitch, with the rig's tilt and roll: the
/// direction a follow camera posed at (yaw, pitch) looks along.
#[derive(Clone, Copy, Debug)]
pub struct PanoramaBasis {
    /// The virtual camera's eye (world).
    pub(crate) eye: Vector3<f32>,
    /// Forward at yaw = pitch = 0, after the rig tilt.
    pub(crate) forward: Vector3<f32>,
    /// The base right axis (pitch turns about it, after yaw).
    pub(crate) right: Vector3<f32>,
    /// Up after the rig tilt and roll (yaw turns about it).
    pub(crate) up: Vector3<f32>,
}

impl PanoramaBasis {
    /// The basis for a calibration (its scene, rig tilt and rig roll).
    pub fn new(calibration: &MatchCalibration) -> Self {
        Self::with_rig(
            &scene_of(calibration).camera_position,
            calibration.rig_tilt as f32,
            calibration.rig_roll as f32,
        )
    }

    /// The basis for a virtual camera at `position` with this rig tilt and roll.
    pub fn with_rig(position: &[f32; 3], rig_tilt: f32, rig_roll: f32) -> Self {
        let camera = super::VirtualCamera::new(position);
        let (forward, up) = camera.rig_axes(rig_tilt, rig_roll);
        Self {
            eye: camera.eye,
            forward,
            right: camera.base_right,
            up,
        }
    }

    /// The world direction at yaw and pitch (radians): yaw about the up
    /// axis, then pitch about the yaw-turned right axis, as the follow
    /// camera's view turns.
    pub fn direction(&self, yaw: f64, pitch: f64) -> Vector3<f64> {
        let (forward, right, up) = (
            self.forward.cast::<f64>(),
            self.right.cast::<f64>(),
            self.up.cast::<f64>(),
        );
        let turn = UnitQuaternion::from_axis_angle(&Unit::new_normalize(up), yaw);
        let tip = UnitQuaternion::from_axis_angle(&Unit::new_normalize(turn * right), pitch);
        (tip * turn * forward).normalize()
    }

    /// The yaw and pitch (radians) of a world direction.
    ///
    /// Read off the up, forward and right axes, which is exact while they
    /// are square to each other (no roll), then refined by Gauss-Newton.
    pub fn yaw_pitch(&self, dir: &Vector3<f64>) -> (f64, f64) {
        let (forward, right) = (self.forward.cast::<f64>(), self.right.cast::<f64>());
        let up = self.up.cast::<f64>().normalize();
        let d = dir.normalize();
        let level = d - up * d.dot(&up);
        let mut angles = [
            (-level.dot(&right)).atan2(level.dot(&forward)),
            d.dot(&up).clamp(-1.0, 1.0).asin(),
        ];
        for _ in 0..20 {
            let miss = self.direction(angles[0], angles[1]) - d;
            if miss.norm() < 1e-14 {
                break;
            }
            const H: f64 = 1e-7;
            let column = |k: usize| {
                let (mut a, mut b) = (angles, angles);
                a[k] += H;
                b[k] -= H;
                (self.direction(a[0], a[1]) - self.direction(b[0], b[1])) / (2.0 * H)
            };
            let (j0, j1) = (column(0), column(1));
            let (a, b, c) = (j0.dot(&j0), j0.dot(&j1), j1.dot(&j1));
            let (g0, g1) = (j0.dot(&miss), j1.dot(&miss));
            let det = a * c - b * b;
            if det.abs() < 1e-18 {
                break;
            }
            angles[0] -= (c * g0 - b * g1) / det;
            angles[1] -= (a * g1 - b * g0) / det;
        }
        (angles[0], angles[1])
    }

    /// The eye as a point (world).
    fn eye_point(&self) -> Point3<f32> {
        Point3::from(self.eye)
    }
}

/// The ray from the virtual camera's eye through camera pixel `(x, y)`
/// (normalized), as a world direction.
fn camera_ray(
    calibration: &MatchCalibration,
    scene: &SceneGeometry,
    camera: CameraId,
    x: f64,
    y: f64,
) -> Option<Vector3<f64>> {
    let params = match camera {
        CameraId::Left => &calibration.left,
        CameraId::Right => &calibration.right,
    };
    let uv = super::inverse_fisheye(x, y, params)?;
    let world = super::plane_uv_to_world(uv, camera, scene);
    let eye = Point3::from(scene.camera_position);
    Some((world - eye).cast::<f64>().normalize())
}

/// Points along a polygon's edges, `OUTLINE_STEPS` per edge (the closing
/// edge included, each corner once). Polygons under 3 points give none.
fn along_edges(polygon: &[[f64; 2]]) -> Vec<[f64; 2]> {
    if polygon.len() < 3 {
        return Vec::new();
    }
    let mut points = Vec::with_capacity(polygon.len() * OUTLINE_STEPS as usize);
    for (k, a) in polygon.iter().enumerate() {
        let b = polygon[(k + 1) % polygon.len()];
        for step in 0..OUTLINE_STEPS {
            let t = f64::from(step) / f64::from(OUTLINE_STEPS);
            points.push([a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]);
        }
    }
    points
}

/// The camera, its extended plane UV and whether it shows (its lens maps
/// the point into the frame), for a world direction.
fn plane_point(
    calibration: &MatchCalibration,
    scene: &SceneGeometry,
    basis: &PanoramaBasis,
    camera: CameraId,
    dir: &Vector3<f64>,
) -> Option<(f64, f64, f32, f32)> {
    let (u, v) = super::plane_uv(&basis.eye_point(), &dir.cast::<f32>(), camera, scene)?;
    let params = match camera {
        CameraId::Left => &calibration.left,
        CameraId::Right => &calibration.right,
    };
    let (nx, ny) = super::forward_fisheye(u, v, params);
    ((0.0..=1.0).contains(&nx) && (0.0..=1.0).contains(&ny)).then_some((u, v, nx as f32, ny as f32))
}

/// The right camera's weight over the left at plane coordinate `u`: the
/// stitch shader's `smoothstep(0, blend, u)` (full weight with no blend).
fn seam_weight(u: f64, blend: f32) -> f64 {
    if blend <= 0.0 {
        return 1.0;
    }
    let t = (u / f64::from(blend)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// How a whole-field export maps its pixels: written beside the video as
/// `<output>.panorama.json`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PanoramaSidecar {
    /// Always `"mercator"`.
    pub projection: String,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Pixels per radian.
    pub px_per_rad: f64,
    /// Yaw at the left edge (radians).
    pub yaw_left: f64,
    /// Yaw at the right edge (radians).
    pub yaw_right: f64,
    /// Pitch at the top edge (radians).
    pub pitch_top: f64,
    /// Pitch at the bottom edge (radians).
    pub pitch_bottom: f64,
    /// Always `"left_input_frame"`: frame n is the left video's frame n.
    pub frame_basis: String,
    /// Frames per second.
    pub fps: f64,
    /// The field outline in panorama pixels, when the calibration has one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub field_outline: Option<SidecarOutline>,
    /// Where the picture changes from one camera to the other: points in
    /// panorama pixels, one every 32 rows where the cameras overlap.
    pub seam: Vec<[f64; 2]>,
    /// The mapping between pixels and yaw/pitch, as a formula.
    pub mapping: String,
}

/// Each camera's field outline in panorama pixels.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SidecarOutline {
    /// The left camera's outline.
    pub left: Vec<[f64; 2]>,
    /// The right camera's outline.
    pub right: Vec<[f64; 2]>,
}

impl PanoramaLayout {
    /// The whole-field layout for a match: the field outline's angles
    /// plus margins, or the cameras' coverage when there is no outline.
    pub fn for_field(calibration: &MatchCalibration, detail: PanoramaDetail) -> Self {
        let scene = scene_of(calibration);
        let basis = PanoramaBasis::new(calibration);
        let bounds = field_bounds(calibration, &scene, &basis)
            .unwrap_or_else(|| coverage_bounds(calibration, &scene));
        Self::fit(bounds, detail, calibration.left.fx)
    }

    /// The camera the picture shows at pixel `(x, y)` and the normalized
    /// pixel in that camera, for a seam blend of `blend`.
    pub fn pixel_to_camera(
        &self,
        basis: &PanoramaBasis,
        calibration: &MatchCalibration,
        blend: f32,
        x: f64,
        y: f64,
    ) -> Option<(CameraId, f32, f32)> {
        let scene = scene_of(calibration);
        let (yaw, pitch) = self.pixel_to_yaw_pitch(x, y);
        let dir = basis.direction(yaw, pitch);
        let left = plane_point(calibration, &scene, basis, CameraId::Left, &dir);
        let right = plane_point(calibration, &scene, basis, CameraId::Right, &dir);
        match (left, right) {
            (_, Some((u, _, nx, ny))) if seam_weight(u, blend) >= 0.5 => {
                Some((CameraId::Right, nx, ny))
            }
            (Some((_, _, nx, ny)), _) => Some((CameraId::Left, nx, ny)),
            (None, Some((u, _, nx, ny))) if seam_weight(u, blend) > 0.0 => {
                Some((CameraId::Right, nx, ny))
            }
            _ => None,
        }
    }

    /// Where camera pixel `(nx, ny)` (normalized, as detections are)
    /// appears in the panorama, in pixel coordinates.
    pub fn camera_to_pixel(
        &self,
        basis: &PanoramaBasis,
        calibration: &MatchCalibration,
        camera: CameraId,
        nx: f64,
        ny: f64,
    ) -> Option<(f64, f64)> {
        let dir = camera_ray(calibration, &scene_of(calibration), camera, nx, ny)?;
        let (yaw, pitch) = basis.yaw_pitch(&dir);
        Some(self.yaw_pitch_to_pixel(yaw, pitch))
    }

    /// The sidecar describing this layout for a match exported at `fps`
    /// with a seam blend of `blend`.
    pub fn sidecar(&self, calibration: &MatchCalibration, blend: f32, fps: f64) -> PanoramaSidecar {
        let scene = scene_of(calibration);
        let basis = PanoramaBasis::new(calibration);
        let outline = |camera: CameraId, polygon: &[[f64; 2]]| -> Vec<[f64; 2]> {
            along_edges(polygon)
                .into_iter()
                .filter_map(|[x, y]| camera_ray(calibration, &scene, camera, x, y))
                .map(|dir| {
                    let (yaw, pitch) = basis.yaw_pitch(&dir);
                    let (x, y) = self.yaw_pitch_to_pixel(yaw, pitch);
                    [x, y]
                })
                .collect()
        };
        PanoramaSidecar {
            projection: "mercator".into(),
            width: self.width,
            height: self.height,
            px_per_rad: self.px_per_rad,
            yaw_left: self.yaw_left,
            yaw_right: self.yaw_right(),
            pitch_top: self.pitch_top,
            pitch_bottom: self.pitch_bottom(),
            frame_basis: "left_input_frame".into(),
            fps,
            field_outline: calibration.field_roi.as_ref().map(|roi| SidecarOutline {
                left: outline(CameraId::Left, &roi.left),
                right: outline(CameraId::Right, &roi.right),
            }),
            seam: self.seam(calibration, &scene, &basis, blend),
            mapping: MAPPING.into(),
        }
    }

    /// Points where the right camera's seam weight is 0.5 (its plane's
    /// u = blend / 2) with the left camera still in view: one row in
    /// every `SEAM_ROW_STEP`, plus the last row.
    fn seam(
        &self,
        calibration: &MatchCalibration,
        scene: &SceneGeometry,
        basis: &PanoramaBasis,
        blend: f32,
    ) -> Vec<[f64; 2]> {
        let target = f64::from(blend.max(0.0)) / 2.0;
        let mut rows: Vec<f64> = (0..self.height / SEAM_ROW_STEP)
            .map(|k| f64::from(k * SEAM_ROW_STEP + SEAM_ROW_STEP / 2))
            .collect();
        rows.push(f64::from(self.height) - 0.5);
        let width = f64::from(self.width);
        let u_at = |x: f64, y: f64| -> Option<f64> {
            let (yaw, pitch) = self.pixel_to_yaw_pitch(x, y);
            let dir = basis.direction(yaw, pitch);
            plane_point(calibration, scene, basis, CameraId::Right, &dir).map(|p| p.0 - target)
        };
        let mut seam = Vec::new();
        for y in rows {
            const SAMPLES: u32 = 512;
            let mut previous: Option<(f64, f64)> = None;
            for k in 0..=SAMPLES {
                let x = width * f64::from(k) / f64::from(SAMPLES);
                let Some(f) = u_at(x, y) else {
                    previous = None;
                    continue;
                };
                if let Some((x0, f0)) = previous
                    && f0 < 0.0
                    && f >= 0.0
                {
                    let (mut lo, mut hi) = (x0, x);
                    for _ in 0..50 {
                        let mid = (lo + hi) / 2.0;
                        match u_at(mid, y) {
                            Some(m) if m < 0.0 => lo = mid,
                            _ => hi = mid,
                        }
                    }
                    let x = (lo + hi) / 2.0;
                    let (yaw, pitch) = self.pixel_to_yaw_pitch(x, y);
                    let dir = basis.direction(yaw, pitch);
                    if plane_point(calibration, scene, basis, CameraId::Left, &dir).is_some() {
                        seam.push([x, y]);
                    }
                    break;
                }
                previous = Some((x, f));
            }
        }
        seam
    }
}

/// The pixel ↔ yaw/pitch mapping, as written in the sidecar.
const MAPPING: &str = "x = (yaw_left - yaw) * px_per_rad; y = (m(pitch_top) - m(pitch)) * px_per_rad; m(p) = asinh(tan(p)); yaw and pitch are reco_core::projection's virtual-camera angles";

/// The field outline's angles plus margins, or `None` without an outline
/// (fewer than 3 points in both cameras, or nothing the lenses map).
fn field_bounds(
    calibration: &MatchCalibration,
    scene: &SceneGeometry,
    basis: &PanoramaBasis,
) -> Option<PanoramaBounds> {
    let roi = calibration.field_roi.as_ref()?;
    let angles: Vec<(f64, f64)> = [(CameraId::Left, &roi.left), (CameraId::Right, &roi.right)]
        .into_iter()
        .flat_map(|(camera, polygon)| {
            along_edges(polygon)
                .into_iter()
                .filter_map(move |[x, y]| camera_ray(calibration, scene, camera, x, y))
        })
        .map(|dir| basis.yaw_pitch(&dir))
        .collect();
    if angles.is_empty() {
        return None;
    }
    let (mut yaw_min, mut yaw_max) = (f64::INFINITY, f64::NEG_INFINITY);
    let (mut pitch_min, mut pitch_max) = (f64::INFINITY, f64::NEG_INFINITY);
    for (yaw, pitch) in angles {
        yaw_min = yaw_min.min(yaw);
        yaw_max = yaw_max.max(yaw);
        pitch_min = pitch_min.min(pitch);
        pitch_max = pitch_max.max(pitch);
    }
    Some(PanoramaBounds {
        yaw_left: yaw_max + YAW_MARGIN,
        yaw_right: yaw_min - YAW_MARGIN,
        pitch_top: pitch_max + PITCH_MARGIN,
        pitch_bottom: pitch_min - PITCH_MARGIN,
    })
}

/// The cameras' whole coverage, from the lowest covered pitch up to at
/// most `FALLBACK_TOP`.
fn coverage_bounds(calibration: &MatchCalibration, scene: &SceneGeometry) -> PanoramaBounds {
    let coverage = super::CoverageBoundary::from_calibration(calibration, scene);
    let (yaw_lo, yaw_hi) = coverage.yaw_range();
    let (pitch_lo, pitch_hi) = coverage.pitch_range();
    PanoramaBounds {
        yaw_left: f64::from(yaw_hi),
        yaw_right: f64::from(yaw_lo),
        pitch_top: f64::from(pitch_hi).min(FALLBACK_TOP),
        pitch_bottom: f64::from(pitch_lo),
    }
}

/// The smallest multiple of `k` that is at least `v` (and at least `k`).
fn ceil_to(v: f64, k: u32) -> u32 {
    let k = f64::from(k);
    ((v / k).ceil().max(1.0) * k) as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layout() -> PanoramaLayout {
        PanoramaLayout {
            width: 800,
            height: 400,
            px_per_rad: 1200.0,
            yaw_left: 1.5,
            pitch_top: 0.2,
            samples: 1,
        }
    }

    fn bounds(yaw: f64, top: f64, bottom: f64) -> PanoramaBounds {
        PanoramaBounds {
            yaw_left: yaw,
            yaw_right: -yaw,
            pitch_top: top,
            pitch_bottom: bottom,
        }
    }

    use crate::projection::{camera_to_panorama, panorama_to_camera};

    /// The 2026-10-03 match: two HERO9s at 5K, with a field outline.
    const MATCH: &str = r#"{"left_uniforms": {"width": 5120, "height": 2880, "fx": 2395.0944275859074, "fy": 2396.2970312304265, "cx": 2559.163154635708, "cy": 1417.5621242076068, "d": [0.034213889574164644, 0.06767320765357862, -0.07408969996955275, 0.029944425249175583]}, "right_uniforms": {"width": 5120, "height": 2880, "fx": 2395.0944275859074, "fy": 2396.2970312304265, "cx": 2559.163154635708, "cy": 1417.5621242076068, "d": [0.034213889574164644, 0.06767320765357862, -0.07408969996955275, 0.029944425249175583]}, "params": {"cameraAxisOffset": 0.24046068217848612, "intersect": 0.5366085162916893, "xTy": -0.008866441029113884, "xRz": -0.04072817616983833, "zRx": -0.04439910063184806, "xRx": 0.0, "zRz": 0.0}, "rig_tilt": 0.0, "rig_roll": 0.0, "sync_offset": 0, "field_roi": {"left": [[0.1531, 0.5083], [0.6016, 0.3778], [0.7031, 0.3653], [1.0, 0.3444], [1.0, 0.9694], [0.5445, 0.9694]], "right": [[0.0, 0.3833], [0.2344, 0.3944], [0.457, 0.4278], [0.9062, 0.5333], [0.5281, 0.9694], [0.0, 0.9694]]}, "lens_correction_amount": 1.0, "blend_width": 0.05}"#;

    fn match_calibration() -> MatchCalibration {
        serde_json::from_str(MATCH).expect("calibration")
    }

    /// Yaw and pitch extremes of an outline sampled through
    /// `camera_to_panorama` (the frame of a level rig).
    fn outline_extents(cal: &MatchCalibration) -> (f64, f64, f64, f64) {
        let scene = scene_of(cal);
        let roi = cal.field_roi.as_ref().unwrap();
        let (mut yaw_min, mut yaw_max) = (f64::INFINITY, f64::NEG_INFINITY);
        let (mut pitch_min, mut pitch_max) = (f64::INFINITY, f64::NEG_INFINITY);
        for (camera, polygon) in [(CameraId::Left, &roi.left), (CameraId::Right, &roi.right)] {
            if polygon.len() < 3 {
                continue;
            }
            for (k, a) in polygon.iter().enumerate() {
                let b = polygon[(k + 1) % polygon.len()];
                for step in 0..=16 {
                    let t = f64::from(step) / 16.0;
                    let (x, y) = (a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t);
                    if let Some(p) = camera_to_panorama(camera, x as f32, y as f32, cal, &scene) {
                        yaw_min = yaw_min.min(f64::from(p.yaw));
                        yaw_max = yaw_max.max(f64::from(p.yaw));
                        pitch_min = pitch_min.min(f64::from(p.pitch));
                        pitch_max = pitch_max.max(f64::from(p.pitch));
                    }
                }
            }
        }
        (yaw_min, yaw_max, pitch_min, pitch_max)
    }

    #[test]
    fn basis_without_tilt_is_the_virtual_camera() {
        let cal = match_calibration();
        let basis = PanoramaBasis::new(&cal);
        let camera = super::super::VirtualCamera::new(&scene_of(&cal).camera_position);
        for i in -6..=6 {
            for j in -4..=2 {
                let (yaw, pitch) = (f64::from(i) * 0.25, f64::from(j) * 0.2);
                let d = basis.direction(yaw, pitch);
                let v = camera.yaw_pitch_to_direction(yaw as f32, pitch as f32);
                assert!(
                    (d - v.cast::<f64>()).norm() < 1e-6,
                    "yaw {yaw} pitch {pitch}"
                );
            }
        }
    }

    #[test]
    fn basis_round_trips_with_tilt_and_roll() {
        let cal = match_calibration();
        let basis = PanoramaBasis::with_rig(&scene_of(&cal).camera_position, 0.05, -0.03);
        for i in -6..=6 {
            for j in -4..=1 {
                let (yaw, pitch) = (f64::from(i) * 0.25, f64::from(j) * 0.2 + 0.1);
                let (y2, p2) = basis.yaw_pitch(&basis.direction(yaw, pitch));
                assert!(
                    (y2 - yaw).abs() < 1e-9 && (p2 - pitch).abs() < 1e-9,
                    "{yaw} {pitch} -> {y2} {p2}"
                );
            }
        }
    }

    #[test]
    fn outline_trims_the_band() {
        let cal = match_calibration();
        let (yaw_min, yaw_max, pitch_min, pitch_max) = outline_extents(&cal);
        let l = PanoramaLayout::for_field(&cal, PanoramaDetail::Half);
        let px = l.px_per_rad;
        // Outline extents (f32 inside camera_to_panorama) plus the margins.
        assert!(
            (l.yaw_left - (yaw_max + YAW_MARGIN)).abs() * px < 1.0,
            "{l:?}"
        );
        assert!(
            (l.pitch_top - (pitch_max + PITCH_MARGIN)).abs() * px < 1.0,
            "{l:?}"
        );
        // The right and bottom edges are rounded out to the encoder's sizes.
        let right = (yaw_min - YAW_MARGIN - l.yaw_right()) * px;
        assert!(
            (-1.0..16.0).contains(&right),
            "right edge {right} px past the margin"
        );
        let bottom = (mercator(pitch_min - PITCH_MARGIN) - mercator(l.pitch_bottom())) * px;
        assert!(
            (-1.0..8.0).contains(&bottom),
            "bottom edge {bottom} px past the margin"
        );
        assert_eq!(px, cal.left.fx / 2.0);
    }

    #[test]
    fn one_camera_outline_is_enough() {
        let mut cal = match_calibration();
        cal.field_roi.as_mut().unwrap().left.truncate(2);
        let l = PanoramaLayout::for_field(&cal, PanoramaDetail::Half);
        let mut right_only = match_calibration();
        right_only.field_roi.as_mut().unwrap().left.clear();
        let (_, yaw_max, _, _) = outline_extents(&right_only);
        assert!(
            (l.yaw_left - (yaw_max + YAW_MARGIN)).abs() * l.px_per_rad < 1.0,
            "{l:?}"
        );
    }

    #[test]
    fn falls_back_without_outline() {
        let mut cal = match_calibration();
        cal.field_roi = None;
        let l = PanoramaLayout::for_field(&cal, PanoramaDetail::Full);
        let coverage = super::super::CoverageBoundary::from_calibration(&cal, &scene_of(&cal));
        let (yaw_lo, yaw_hi) = coverage.yaw_range();
        assert!((l.yaw_left - f64::from(yaw_hi)).abs() < 1e-6, "{l:?}");
        assert!(l.yaw_right() <= f64::from(yaw_lo) + 1e-6);
        assert!(l.pitch_top <= FALLBACK_TOP + 1e-9);
        assert!(l.width > 0 && l.height > 0);
    }

    #[test]
    fn pixel_to_camera_agrees_with_panorama_to_camera() {
        let cal = match_calibration();
        let scene = scene_of(&cal);
        let basis = PanoramaBasis::new(&cal);
        let l = PanoramaLayout::for_field(&cal, PanoramaDetail::Half);
        let mut seen = [0, 0];
        for x in (8..l.width).step_by(97) {
            for y in (8..l.height).step_by(61) {
                let (px, py) = (f64::from(x) + 0.5, f64::from(y) + 0.5);
                let Some((camera, nx, ny)) = l.pixel_to_camera(&basis, &cal, 0.05, px, py) else {
                    continue;
                };
                seen[usize::from(camera == CameraId::Right)] += 1;
                let (yaw, pitch) = l.pixel_to_yaw_pitch(px, py);
                let (ex, ey) = panorama_to_camera(yaw as f32, pitch as f32, camera, &cal, &scene)
                    .expect("the camera chosen sees the point");
                assert!(
                    (nx - ex).abs() < 1e-4 && (ny - ey).abs() < 1e-4,
                    "({x}, {y})"
                );
            }
        }
        assert!(
            seen[0] > 20 && seen[1] > 20,
            "both cameras appear: {seen:?}"
        );
    }

    #[test]
    fn seam_runs_down_the_overlap() {
        let cal = match_calibration();
        let basis = PanoramaBasis::new(&cal);
        let l = PanoramaLayout::for_field(&cal, PanoramaDetail::Half);
        let seam = l.sidecar(&cal, 0.05, 29.97).seam;
        assert!(
            seam.len() as u32 >= l.height / SEAM_ROW_STEP / 2,
            "{} points",
            seam.len()
        );
        for [x, y] in seam {
            // Just left of the seam the left camera shows, just right of it the right.
            let left = l
                .pixel_to_camera(&basis, &cal, 0.05, x - 2.0, y)
                .map(|c| c.0);
            let right = l
                .pixel_to_camera(&basis, &cal, 0.05, x + 2.0, y)
                .map(|c| c.0);
            assert_eq!(
                (left, right),
                (Some(CameraId::Left), Some(CameraId::Right)),
                "at ({x}, {y})"
            );
        }
    }

    #[test]
    fn sidecar_json_shape() {
        let cal = match_calibration();
        let l = PanoramaLayout::for_field(&cal, PanoramaDetail::Half);
        let json = serde_json::to_value(l.sidecar(&cal, 0.05, 29.97)).unwrap();
        for key in [
            "projection",
            "width",
            "height",
            "px_per_rad",
            "yaw_left",
            "yaw_right",
            "pitch_top",
            "pitch_bottom",
            "frame_basis",
            "fps",
            "field_outline",
            "seam",
            "mapping",
        ] {
            assert!(json.get(key).is_some(), "missing {key}");
        }
        assert_eq!(json["projection"], "mercator");
        assert_eq!(json["frame_basis"], "left_input_frame");
        assert!(json["field_outline"]["left"].as_array().unwrap().len() > 6);
    }

    #[test]
    fn mercator_known_points() {
        assert_eq!(mercator(0.0), 0.0);
        assert!((mercator(0.5) - 0.5_f64.tan().asinh()).abs() < 1e-15);
        for i in -12..=12 {
            let p = f64::from(i) / 10.0;
            assert!(
                (inverse_mercator(mercator(p)) - p).abs() < 1e-12,
                "pitch {p}"
            );
        }
    }

    #[test]
    fn pixels_and_angles_round_trip() {
        let l = layout();
        for x in (0..=800).step_by(50) {
            for y in (0..=400).step_by(50) {
                let (yaw, pitch) = l.pixel_to_yaw_pitch(f64::from(x), f64::from(y));
                let (bx, by) = l.yaw_pitch_to_pixel(yaw, pitch);
                assert!((bx - f64::from(x)).abs() < 1e-9 && (by - f64::from(y)).abs() < 1e-9);
            }
        }
    }

    #[test]
    fn yaw_decreases_to_the_right() {
        let l = layout();
        assert_eq!(l.pixel_to_yaw_pitch(0.0, 10.0).0, 1.5);
        let right = l.pixel_to_yaw_pitch(800.0, 10.0).0;
        assert!((right - l.yaw_right()).abs() < 1e-12 && right < 1.5);
        assert!((l.yaw_right() - (1.5 - 800.0 / 1200.0)).abs() < 1e-12);
        assert!((l.pixel_to_yaw_pitch(10.0, 0.0).1 - 0.2).abs() < 1e-12);
        let bottom = l.pixel_to_yaw_pitch(10.0, 400.0).1;
        assert!((bottom - l.pitch_bottom()).abs() < 1e-12 && bottom < 0.2);
    }

    #[test]
    fn full_keeps_the_lens_density() {
        let l = PanoramaLayout::fit(bounds(1.5, 0.2, -0.8), PanoramaDetail::Full, 2395.0);
        assert_eq!(l.px_per_rad, 2395.0);
        assert_eq!(l.samples, 1);
        assert_eq!(l.width, 7200);
        let height = ((mercator(0.2) - mercator(-0.8)) * 2395.0 / 8.0).ceil() as u32 * 8;
        assert_eq!(l.height, height);
        assert_eq!((l.yaw_left, l.pitch_top), (1.5, 0.2));
    }

    #[test]
    fn half_is_half_with_two_samples() {
        let l = PanoramaLayout::fit(bounds(1.5, 0.2, -0.8), PanoramaDetail::Half, 2395.0);
        assert_eq!(l.px_per_rad, 1197.5);
        assert_eq!(l.samples, 2);
        assert_eq!(l.width, 3600);
    }

    #[test]
    fn width_is_capped_at_8192() {
        let l = PanoramaLayout::fit(bounds(1.8, 0.2, -0.8), PanoramaDetail::Full, 2395.0);
        assert_eq!(l.width, 8192);
        assert!((l.px_per_rad - 8192.0 / 3.6).abs() < 1e-9);
    }

    #[test]
    fn sizes_suit_the_encoder() {
        for k in 1..=20 {
            let t = f64::from(k);
            let b = bounds(0.3 + 0.07 * t, 0.05 + 0.01 * t, -0.2 - 0.03 * t);
            for detail in [PanoramaDetail::Half, PanoramaDetail::Full] {
                let l = PanoramaLayout::fit(b, detail, 1000.0 + 77.0 * t);
                assert!(
                    l.width.is_multiple_of(16) && l.width > 0 && l.width <= 8192,
                    "{l:?}"
                );
                assert!(l.height.is_multiple_of(8) && l.height > 0, "{l:?}");
            }
        }
    }
}
