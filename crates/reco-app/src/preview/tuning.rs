//! The Adjust panel's stitch settings as the preview session applies them:
//! one change at a time (`Tuning`), and everything the panel shows
//! (`CalibrationValues`). Ranges are the Slint app's sliders.

/// Seam blend, as a share of the overlap.
pub const BLEND_RANGE: (f32, f32) = (0.0, 0.3);
/// Rig tilt in degrees.
pub const TILT_RANGE: (f32, f32) = (-30.0, 30.0);
/// Rig roll in degrees.
pub const ROLL_RANGE: (f32, f32) = (-15.0, 15.0);
/// How far the two planes overlap.
pub const INTERSECT_RANGE: (f64, f64) = (-1.0, 1.0);
/// The virtual camera's distance from the origin.
pub const AXIS_OFFSET_RANGE: (f64, f64) = (-0.6, 0.6);
/// The right plane's vertical shift.
pub const X_TY_RANGE: (f64, f64) = (-0.1, 0.1);

/// One change from the Adjust panel.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Tuning {
    /// Seam blend.
    Blend(f32),
    /// Exposure and colour matching between the cameras.
    ColorMatch(bool),
    /// Rig tilt, degrees.
    Tilt(f32),
    /// Rig roll, degrees.
    Roll(f32),
    /// Plane overlap.
    Intersect(f64),
    /// Camera axis offset.
    AxisOffset(f64),
    /// The right plane's vertical shift.
    XTy(f64),
    /// Back to the layout the calibration file had.
    ResetLayout,
}

impl Tuning {
    /// The change with its value kept inside its slider's range.
    pub fn clamped(self) -> Self {
        match self {
            Self::Blend(v) => Self::Blend(v.clamp(BLEND_RANGE.0, BLEND_RANGE.1)),
            Self::Tilt(v) => Self::Tilt(v.clamp(TILT_RANGE.0, TILT_RANGE.1)),
            Self::Roll(v) => Self::Roll(v.clamp(ROLL_RANGE.0, ROLL_RANGE.1)),
            Self::Intersect(v) => Self::Intersect(v.clamp(INTERSECT_RANGE.0, INTERSECT_RANGE.1)),
            Self::AxisOffset(v) => {
                Self::AxisOffset(v.clamp(AXIS_OFFSET_RANGE.0, AXIS_OFFSET_RANGE.1))
            }
            Self::XTy(v) => Self::XTy(v.clamp(X_TY_RANGE.0, X_TY_RANGE.1)),
            other @ (Self::ColorMatch(_) | Self::ResetLayout) => other,
        }
    }
}

/// What the Adjust panel shows: the live calibration.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CalibrationValues {
    /// Seam blend.
    pub blend: f32,
    /// Colour matching.
    pub color_match: bool,
    /// Rig tilt, degrees.
    pub tilt: f32,
    /// Rig roll, degrees.
    pub roll: f32,
    /// Plane overlap.
    pub intersect: f64,
    /// Camera axis offset.
    pub axis_offset: f64,
    /// The right plane's vertical shift.
    pub x_ty: f64,
    /// Frames between the cameras (positive: the right camera started
    /// first).
    pub sync_offset: i64,
    /// Points in the field outline, both cameras (0: none).
    pub roi_points: usize,
    /// Changed since it was loaded or saved.
    pub dirty: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn changes_stay_inside_their_sliders() {
        assert_eq!(Tuning::Blend(0.9).clamped(), Tuning::Blend(0.3));
        assert_eq!(Tuning::Tilt(-45.0).clamped(), Tuning::Tilt(-30.0));
        assert_eq!(Tuning::Roll(20.0).clamped(), Tuning::Roll(15.0));
        assert_eq!(Tuning::Intersect(-2.0).clamped(), Tuning::Intersect(-1.0));
        assert_eq!(Tuning::AxisOffset(0.7).clamped(), Tuning::AxisOffset(0.6));
        assert_eq!(Tuning::XTy(0.05).clamped(), Tuning::XTy(0.05));
        assert_eq!(
            Tuning::ColorMatch(false).clamped(),
            Tuning::ColorMatch(false)
        );
        assert_eq!(Tuning::ResetLayout.clamped(), Tuning::ResetLayout);
    }
}
