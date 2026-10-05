//! The window's size and full-screen state and the side panels' widths,
//! kept between runs: saved a quiet second after they change (or on quit)
//! and restored at start. `--window-size` (checks) wins over the saved size.

use makepad_widgets::*;

use crate::App;

/// Seconds of quiet before a changed layout is saved.
const QUIET: f64 = 1.0;

impl App {
    /// The saved panels and window, at start.
    pub(crate) fn restore_layout(&mut self, cx: &mut Cx) {
        if let Some(width) = self.settings.setup_width {
            self.ui
                .splitter(cx, ids!(main_split))
                .set_align(cx, SplitterAlign::FromA(width));
        }
        if let Some(width) = self.settings.adjust_width {
            self.ui
                .splitter(cx, ids!(inner_split))
                .set_align(cx, SplitterAlign::FromB(width));
        }
        if self.args.window_size.is_some() {
            return;
        }
        let window = self.ui.window(cx, ids!(main_window));
        if let Some((width, height)) = self.settings.restored_window_size() {
            log!("layout: window {width}x{height}");
            window.resize(cx, dvec2(width, height));
            // Placing the window fits it to the attached displays: a size
            // saved on a larger display shrinks to this one.
            let at = window.get_position(cx);
            window.reposition(cx, at);
        }
        if self.settings.window_maximized {
            log!("layout: full screen");
            window.maximize(cx);
        }
    }

    /// The window moved or resized: remember it.
    pub(crate) fn window_changed(&mut self, cx: &mut Cx, geom: &WindowGeom) {
        let size = (geom.outer_size.x, geom.outer_size.y);
        if self.settings.remember_window(size, geom.is_fullscreen) {
            self.save_layout_later(cx);
        }
    }

    /// A side panel's edge was dragged to a new width.
    pub(crate) fn layout_actions(&mut self, cx: &mut Cx, actions: &Actions) {
        if let Some((_, SplitterAlign::FromA(width))) =
            self.ui.splitter(cx, ids!(main_split)).settled(actions)
        {
            self.settings.setup_width = Some(width);
            self.save_layout_later(cx);
        }
        if let Some((_, SplitterAlign::FromB(width))) =
            self.ui.splitter(cx, ids!(inner_split)).settled(actions)
        {
            self.settings.adjust_width = Some(width);
            self.save_layout_later(cx);
        }
    }

    fn save_layout_later(&mut self, cx: &mut Cx) {
        cx.stop_timer(self.layout_timer);
        self.layout_timer = cx.start_timeout(QUIET);
    }

    /// Save a changed layout: after the quiet second, or now (quitting).
    pub(crate) fn save_layout(&mut self, cx: &mut Cx) {
        if self.layout_timer.is_empty() {
            return;
        }
        cx.stop_timer(self.layout_timer);
        self.layout_timer = Timer::empty();
        let s = &self.settings;
        log!(
            "layout: saved window {:?}{}, panels {:?} and {:?}",
            s.window_size,
            if s.window_maximized {
                " (full screen)"
            } else {
                ""
            },
            s.setup_width,
            s.adjust_width
        );
        self.save_settings();
    }
}
