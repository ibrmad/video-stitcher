//! Pure view math for the preview: the aspect the viewer letterboxes to,
//! the contain fit, and the render-target size in physical pixels.

/// The preview aspect the viewer letterboxes to (the dropdown's order).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum PreviewAspect {
    /// Fill the viewer.
    #[default]
    Auto,
    /// 16:9.
    Wide16x9,
    /// 4:3.
    Classic4x3,
    /// 21:9.
    Cinema21x9,
}

impl PreviewAspect {
    /// The aspect at a dropdown index (Auto, 16:9, 4:3, 21:9); Auto otherwise.
    pub fn from_index(index: usize) -> Self {
        match index {
            1 => Self::Wide16x9,
            2 => Self::Classic4x3,
            3 => Self::Cinema21x9,
            _ => Self::Auto,
        }
    }

    /// Width over height, or `None` for Auto (fill).
    pub fn ratio(self) -> Option<f64> {
        match self {
            Self::Auto => None,
            Self::Wide16x9 => Some(16.0 / 9.0),
            Self::Classic4x3 => Some(4.0 / 3.0),
            Self::Cinema21x9 => Some(21.0 / 9.0),
        }
    }
}

/// The largest box of `ratio` inside `area`, centred: `(x, y, width,
/// height)` with (0, 0) at the area's corner. `None` fills the area.
pub fn fit(area: (f64, f64), ratio: Option<f64>) -> (f64, f64, f64, f64) {
    let (aw, ah) = (area.0.max(0.0), area.1.max(0.0));
    let Some(ratio) = ratio.filter(|r| *r > 0.0) else {
        return (0.0, 0.0, aw, ah);
    };
    let (w, h) = if aw / ratio <= ah {
        (aw, aw / ratio)
    } else {
        (ah * ratio, ah)
    };
    ((aw - w) / 2.0, (ah - h) / 2.0, w, h)
}

/// The largest render target, in pixels.
const MAX_RENDER: (f64, f64) = (3840.0, 2160.0);
/// The smallest render-target side, in pixels.
const MIN_RENDER: u32 = 64;
/// Pixels a side must change by before the target is rebuilt.
const RESIZE_HYSTERESIS: u32 = 16;

/// Render-target pixels for a box of `width_pt` × `height_pt` points at
/// `dpi` pixels per point: even sizes, at least 64, and scaled down
/// uniformly to fit 3840 × 2160.
pub fn render_size(width_pt: f64, height_pt: f64, dpi: f64) -> (u32, u32) {
    let (mut w, mut h) = (width_pt.max(0.0) * dpi, height_pt.max(0.0) * dpi);
    let scale = (MAX_RENDER.0 / w.max(1.0))
        .min(MAX_RENDER.1 / h.max(1.0))
        .min(1.0);
    w *= scale;
    h *= scale;
    let even = |v: f64| ((v.floor() as u32) & !1).max(MIN_RENDER);
    (even(w), even(h))
}

/// Whether to rebuild the render target: always when there is none, else
/// only when a side changes by more than 16 pixels.
pub fn should_resize(current: Option<(u32, u32)>, wanted: (u32, u32)) -> bool {
    match current {
        None => true,
        Some((w, h)) => {
            w.abs_diff(wanted.0) > RESIZE_HYSTERESIS || h.abs_diff(wanted.1) > RESIZE_HYSTERESIS
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aspects_follow_the_dropdown_order() {
        assert_eq!(PreviewAspect::from_index(0), PreviewAspect::Auto);
        assert_eq!(PreviewAspect::from_index(1), PreviewAspect::Wide16x9);
        assert_eq!(PreviewAspect::from_index(2), PreviewAspect::Classic4x3);
        assert_eq!(PreviewAspect::from_index(3), PreviewAspect::Cinema21x9);
        assert_eq!(PreviewAspect::from_index(9), PreviewAspect::Auto);
        assert_eq!(PreviewAspect::Auto.ratio(), None);
        assert_eq!(PreviewAspect::Wide16x9.ratio(), Some(16.0 / 9.0));
    }

    #[test]
    fn fit_letterboxes_and_pillarboxes() {
        // A wide area pillarboxes 4:3.
        assert_eq!(
            fit((800.0, 300.0), Some(4.0 / 3.0)),
            (200.0, 0.0, 400.0, 300.0)
        );
        // A tall area letterboxes 16:9.
        assert_eq!(
            fit((320.0, 400.0), Some(16.0 / 9.0)),
            (0.0, 110.0, 320.0, 180.0)
        );
        // Auto fills.
        assert_eq!(fit((640.0, 480.0), None), (0.0, 0.0, 640.0, 480.0));
    }

    #[test]
    fn fit_survives_an_empty_area() {
        assert_eq!(fit((0.0, 0.0), Some(16.0 / 9.0)), (0.0, 0.0, 0.0, 0.0));
    }

    #[test]
    fn render_size_counts_physical_pixels() {
        assert_eq!(render_size(800.0, 450.0, 2.0), (1600, 900));
        // Odd pixel counts round down to even.
        assert_eq!(render_size(400.5, 225.5, 1.0), (400, 224));
    }

    #[test]
    fn render_size_caps_at_4k_keeping_the_aspect() {
        assert_eq!(render_size(2560.0, 1440.0, 2.0), (3840, 2160));
        assert_eq!(render_size(3000.0, 1000.0, 2.0), (3840, 1280));
    }

    #[test]
    fn render_size_has_a_floor() {
        assert_eq!(render_size(10.0, 5.0, 1.0), (64, 64));
    }

    #[test]
    fn resize_only_past_the_hysteresis() {
        assert!(should_resize(None, (800, 450)));
        assert!(!should_resize(Some((800, 450)), (812, 440)));
        assert!(should_resize(Some((800, 450)), (820, 450)));
        assert!(should_resize(Some((800, 450)), (800, 470)));
    }
}
