//! The cameras' lenses as the Adjust panel shows and tunes them: the
//! intrinsics a slider moves (`Lens`), the fine-tune sliders' ranges, and a
//! lens profile scaled to the videos' size. The rules are the Slint app's
//! (`set_lens_sliders`, the lens picker).

use reco_core::calibration::CameraParams;

/// A camera's lens: focal lengths and principal point in pixels, and the
/// four fisheye (KB4) distortion terms. The size it was modelled at stays
/// with the calibration.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Lens {
    /// Focal length, x (pixels).
    pub fx: f64,
    /// Focal length, y (pixels).
    pub fy: f64,
    /// Principal point, x (pixels).
    pub cx: f64,
    /// Principal point, y (pixels).
    pub cy: f64,
    /// Distortion k1–k4.
    pub k: [f64; 4],
}

impl Lens {
    /// The lens in `params`.
    pub fn of(params: &CameraParams) -> Self {
        Self {
            fx: params.fx,
            fy: params.fy,
            cx: params.cx,
            cy: params.cy,
            k: params.d,
        }
    }

    /// `params` with this lens (its size kept).
    pub fn applied_to(&self, params: &CameraParams) -> CameraParams {
        CameraParams {
            fx: self.fx,
            fy: self.fy,
            cx: self.cx,
            cy: self.cy,
            d: self.k,
            ..params.clone()
        }
    }
}

/// Which cameras a lens change is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cameras {
    /// The left camera.
    Left,
    /// The right camera.
    Right,
    /// Both, in step.
    Both,
}

impl Cameras {
    /// Whether the left camera is included.
    pub fn left(self) -> bool {
        matches!(self, Self::Left | Self::Both)
    }

    /// Whether the right camera is included.
    pub fn right(self) -> bool {
        matches!(self, Self::Right | Self::Both)
    }
}

/// The fine-tune sliders' ranges, centred on a lens.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FineTuneRanges {
    /// fx (pixels).
    pub fx: (f64, f64),
    /// fy (pixels).
    pub fy: (f64, f64),
    /// cx (pixels).
    pub cx: (f64, f64),
    /// cy (pixels).
    pub cy: (f64, f64),
    /// k1–k4.
    pub k: (f64, f64),
}

impl FineTuneRanges {
    /// The Slint app's ranges around `lens` for a `width`×`height` picture:
    /// fx and fy ±15% of the larger focal length, cx and cy ±10% of the
    /// picture, at least 5 px each; k1–k4 ±0.3.
    pub fn around(lens: &Lens, width: u32, height: u32) -> Self {
        let f = (lens.fx.max(lens.fy) * 0.15).max(5.0);
        let x = (f64::from(width.max(1)) * 0.10).max(5.0);
        let y = (f64::from(height.max(1)) * 0.10).max(5.0);
        Self {
            fx: (lens.fx - f, lens.fx + f),
            fy: (lens.fy - f, lens.fy + f),
            cx: (lens.cx - x, lens.cx + x),
            cy: (lens.cy - y, lens.cy + y),
            k: (-0.3, 0.3),
        }
    }
}

/// A lens profile (from the database or a file) scaled to a
/// `width`×`height` video, as the Slint app's picker did.
pub fn scaled_to(profile: &CameraParams, width: u32, height: u32) -> CameraParams {
    if profile.width == 0 || profile.height == 0 {
        return profile.clone();
    }
    let sx = f64::from(width) / f64::from(profile.width);
    let sy = f64::from(height) / f64::from(profile.height);
    CameraParams {
        width,
        height,
        fx: profile.fx * sx,
        fy: profile.fy * sy,
        cx: profile.cx * sx,
        cy: profile.cy * sy,
        d: profile.d,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn params() -> CameraParams {
        CameraParams {
            width: 1920,
            height: 1080,
            fx: 900.0,
            fy: 880.0,
            cx: 960.0,
            cy: 540.0,
            d: [0.03, 0.06, -0.07, 0.02],
        }
    }

    #[test]
    fn a_lens_round_trips_through_its_camera() {
        let lens = Lens::of(&params());
        assert_eq!(
            lens,
            Lens {
                fx: 900.0,
                fy: 880.0,
                cx: 960.0,
                cy: 540.0,
                k: [0.03, 0.06, -0.07, 0.02]
            }
        );
        let moved = Lens { fx: 950.0, ..lens }.applied_to(&params());
        assert_eq!((moved.fx, moved.width, moved.height), (950.0, 1920, 1080));
        assert_eq!(Lens::of(&moved).k, lens.k);
    }

    #[test]
    fn which_cameras() {
        assert!(Cameras::Both.left() && Cameras::Both.right());
        assert!(Cameras::Left.left() && !Cameras::Left.right());
        assert!(!Cameras::Right.left() && Cameras::Right.right());
    }

    #[test]
    fn fine_tune_ranges_are_the_slint_apps() {
        let r = FineTuneRanges::around(&Lens::of(&params()), 1920, 1080);
        // ±15% of max(fx, fy) = 135.
        assert_eq!(r.fx, (765.0, 1035.0));
        assert_eq!(r.fy, (745.0, 1015.0));
        // ±10% of the width and height.
        assert_eq!(r.cx, (768.0, 1152.0));
        assert_eq!(r.cy, (432.0, 648.0));
        assert_eq!(r.k, (-0.3, 0.3));
        let tiny = FineTuneRanges::around(
            &Lens {
                fx: 10.0,
                fy: 10.0,
                ..Lens::default()
            },
            20,
            20,
        );
        assert_eq!(tiny.fx, (5.0, 15.0), "at least 5 px");
        assert_eq!(tiny.cx, (-5.0, 5.0), "at least 5 px");
    }

    #[test]
    fn a_profile_is_scaled_to_the_videos() {
        let scaled = scaled_to(&params(), 3840, 2160);
        assert_eq!((scaled.width, scaled.height), (3840, 2160));
        assert_eq!((scaled.fx, scaled.fy), (1800.0, 1760.0));
        assert_eq!((scaled.cx, scaled.cy), (1920.0, 1080.0));
        assert_eq!(scaled.d, params().d, "distortion has no size");
        let no_size = CameraParams {
            width: 0,
            height: 0,
            ..params()
        };
        assert_eq!(
            scaled_to(&no_size, 3840, 2160).fx,
            900.0,
            "no size: as it is"
        );
    }
}
