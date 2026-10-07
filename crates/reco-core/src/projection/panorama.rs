//! The whole-field panorama: a fixed Mercator picture of the field in
//! Reco's own yaw/pitch.
//!
//! Columns are even in yaw (yaw decreases to the right, as everywhere in
//! Reco) and rows follow the Mercator ordinate `asinh(tan(pitch))`, so
//! shapes keep their proportions from the far touchline down to the
//! players near the camera. Pixel `(i, j)` has its centre at
//! `(i + 0.5, j + 0.5)`; `(0, 0)` is the top-left corner of the picture.

use serde::{Deserialize, Serialize};

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
