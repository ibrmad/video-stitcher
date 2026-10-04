//! Which side panels are open, and the rules that fold them on narrow
//! windows. Pure state, so the rules are unit-tested without a window.

/// Below this window width (points) the Inspector folds by itself.
pub const INSPECTOR_FOLD_WIDTH: f64 = 960.0;
/// Below this window width (points) the Media sidebar folds as well.
pub const MEDIA_FOLD_WIDTH: f64 = 700.0;

/// The two side panels of the shell.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Panel {
    /// The Media sidebar on the left (videos and calibration).
    Media,
    /// The Inspector on the right (view, stitching, lens, stats).
    Inspector,
}

/// Open/closed state of the side panels and whether files are loaded.
#[derive(Clone, Debug, PartialEq)]
pub struct ShellState {
    media_open: bool,
    inspector_open: bool,
    media_auto_folded: bool,
    inspector_auto_folded: bool,
    files_loaded: bool,
    last_width: Option<f64>,
}

impl Default for ShellState {
    /// Nothing loaded: Media open, because loading starts there; Inspector
    /// closed, because there is nothing to adjust yet.
    fn default() -> Self {
        Self {
            media_open: true,
            inspector_open: false,
            media_auto_folded: false,
            inspector_auto_folded: false,
            files_loaded: false,
            last_width: None,
        }
    }
}

impl ShellState {
    /// Whether a panel is currently shown.
    pub fn is_open(&self, panel: Panel) -> bool {
        match panel {
            Panel::Media => self.media_open,
            Panel::Inspector => self.inspector_open,
        }
    }

    /// Whether the panel is closed because the window got narrow (not by the
    /// user), so the shell can say how to bring it back.
    pub fn auto_folded(&self, panel: Panel) -> bool {
        match panel {
            Panel::Media => self.media_auto_folded,
            Panel::Inspector => self.inspector_auto_folded,
        }
    }

    /// Whether left/right videos are loaded (gates Export, transport and
    /// the Inspector toggle).
    pub fn files_loaded(&self) -> bool {
        self.files_loaded
    }

    /// Whether the panel's toggle is enabled.
    pub fn can_toggle(&self, panel: Panel) -> bool {
        match panel {
            Panel::Media => true,
            Panel::Inspector => self.files_loaded,
        }
    }

    /// Flip a panel by hand. Returns false (and changes nothing) when the
    /// toggle is disabled. A hand toggle cancels any automatic fold.
    pub fn toggle(&mut self, panel: Panel) -> bool {
        if !self.can_toggle(panel) {
            return false;
        }
        match panel {
            Panel::Media => {
                self.media_open = !self.media_open;
                self.media_auto_folded = false;
            }
            Panel::Inspector => {
                self.inspector_open = !self.inspector_open;
                self.inspector_auto_folded = false;
            }
        }
        true
    }

    /// Record that files were loaded or unloaded. Loading opens the
    /// Inspector, unless the window is already narrower than
    /// [`INSPECTOR_FOLD_WIDTH`]: then it stays folded and reopens when the
    /// window widens past the limit.
    pub fn set_files_loaded(&mut self, loaded: bool) {
        if loaded == self.files_loaded {
            return;
        }
        self.files_loaded = loaded;
        let narrow = self
            .last_width
            .is_some_and(|width| width < INSPECTOR_FOLD_WIDTH);
        self.inspector_open = loaded && !narrow;
        self.inspector_auto_folded = loaded && narrow;
    }

    /// Apply the width rule after a resize. A panel folds when the window
    /// crosses below its limit (or starts below it) and reopens when the
    /// window crosses back above it, unless the user toggled it meanwhile.
    pub fn fit_width(&mut self, width: f64) {
        let prev = self.last_width.replace(width);
        let below = |limit: f64| width < limit && prev.is_none_or(|p| p >= limit);
        let above = |limit: f64| width >= limit && prev.is_some_and(|p| p < limit);

        if below(INSPECTOR_FOLD_WIDTH) && self.inspector_open {
            self.inspector_open = false;
            self.inspector_auto_folded = true;
        } else if above(INSPECTOR_FOLD_WIDTH) && self.inspector_auto_folded {
            self.inspector_open = true;
            self.inspector_auto_folded = false;
        }
        if below(MEDIA_FOLD_WIDTH) && self.media_open {
            self.media_open = false;
            self.media_auto_folded = true;
        } else if above(MEDIA_FOLD_WIDTH) && self.media_auto_folded {
            self.media_open = true;
            self.media_auto_folded = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn startup_opens_media_and_closes_inspector() {
        let s = ShellState::default();
        assert!(s.is_open(Panel::Media));
        assert!(!s.is_open(Panel::Inspector));
        assert!(!s.files_loaded());
    }

    #[test]
    fn media_toggles_by_hand() {
        let mut s = ShellState::default();
        assert!(s.toggle(Panel::Media));
        assert!(!s.is_open(Panel::Media));
        assert!(s.toggle(Panel::Media));
        assert!(s.is_open(Panel::Media));
    }

    #[test]
    fn inspector_toggle_refused_before_load() {
        let mut s = ShellState::default();
        assert!(!s.can_toggle(Panel::Inspector));
        assert!(!s.toggle(Panel::Inspector));
        assert!(!s.is_open(Panel::Inspector));
    }

    #[test]
    fn loading_opens_inspector_and_unloading_closes_it() {
        let mut s = ShellState::default();
        s.set_files_loaded(true);
        assert!(s.is_open(Panel::Inspector));
        assert!(s.can_toggle(Panel::Inspector));
        s.set_files_loaded(false);
        assert!(!s.is_open(Panel::Inspector));
    }

    #[test]
    fn narrowing_folds_inspector_then_media_and_widening_restores_both() {
        let mut s = ShellState::default();
        s.set_files_loaded(true);
        s.fit_width(1280.0);
        s.fit_width(900.0);
        assert!(!s.is_open(Panel::Inspector));
        assert!(s.is_open(Panel::Media));
        s.fit_width(650.0);
        assert!(!s.is_open(Panel::Media));
        s.fit_width(1280.0);
        assert!(s.is_open(Panel::Inspector));
        assert!(s.is_open(Panel::Media));
    }

    #[test]
    fn first_width_below_limit_folds() {
        let mut s = ShellState::default();
        s.set_files_loaded(true);
        s.fit_width(720.0);
        assert!(!s.is_open(Panel::Inspector));
        assert!(s.is_open(Panel::Media));
    }

    #[test]
    fn hand_reopen_survives_further_narrow_resizes() {
        let mut s = ShellState::default();
        s.set_files_loaded(true);
        s.fit_width(1280.0);
        s.fit_width(900.0);
        assert!(s.toggle(Panel::Inspector));
        s.fit_width(880.0);
        s.fit_width(860.0);
        assert!(s.is_open(Panel::Inspector));
    }

    #[test]
    fn hand_closed_panel_stays_closed_when_widening() {
        let mut s = ShellState::default();
        s.fit_width(1280.0);
        assert!(s.toggle(Panel::Media));
        s.fit_width(650.0);
        s.fit_width(1280.0);
        assert!(!s.is_open(Panel::Media));
    }

    #[test]
    fn reports_which_panels_folded_by_themselves() {
        let mut s = ShellState::default();
        s.set_files_loaded(true);
        s.fit_width(1280.0);
        s.fit_width(900.0);
        assert!(s.auto_folded(Panel::Inspector));
        assert!(!s.auto_folded(Panel::Media));
        assert!(s.toggle(Panel::Inspector));
        assert!(!s.auto_folded(Panel::Inspector));
    }

    #[test]
    fn loading_on_narrow_window_keeps_inspector_folded() {
        let mut s = ShellState::default();
        s.fit_width(1280.0);
        s.fit_width(900.0);
        s.set_files_loaded(true);
        assert!(!s.is_open(Panel::Inspector));
    }

    #[test]
    fn loading_on_window_that_started_narrow_keeps_inspector_folded() {
        let mut s = ShellState::default();
        s.fit_width(720.0);
        s.set_files_loaded(true);
        assert!(!s.is_open(Panel::Inspector));
    }

    #[test]
    fn reloading_on_narrow_window_keeps_inspector_folded() {
        let mut s = ShellState::default();
        s.set_files_loaded(true);
        s.fit_width(1280.0);
        s.fit_width(900.0);
        s.set_files_loaded(false);
        s.set_files_loaded(true);
        assert!(!s.is_open(Panel::Inspector));
    }

    #[test]
    fn inspector_folded_by_a_narrow_load_reopens_when_widening() {
        let mut s = ShellState::default();
        s.fit_width(900.0);
        s.set_files_loaded(true);
        s.fit_width(1280.0);
        assert!(s.is_open(Panel::Inspector));
    }
}
