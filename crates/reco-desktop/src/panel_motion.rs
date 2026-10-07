//! Panels slide open and closed: a toggle starts a slide, each frame moves the
//! splitter's bar (or sets the lanes' height), and the end folds the panel or
//! leaves it open.
//!
//! A splitter keeps a pane's width at its floor while it is open, so a slide
//! relaxes the floors for its 0.2 s and puts them back from the theme after. A
//! script apply sets a widget's other values back to its DSL, so the bar and
//! the fold are set again after each one.

use makepad_widgets::*;

use crate::motion::Motion;
use crate::shell_state::Panel;
use crate::ui::panel_box::RecoPanelBox;
use crate::App;

/// What slides.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Slide {
    Setup,
    Adjust,
    Lanes,
}

impl Slide {
    const ALL: [Slide; 3] = [Slide::Setup, Slide::Adjust, Slide::Lanes];

    pub(crate) fn of(panel: Panel) -> Self {
        match panel {
            Panel::Media => Slide::Setup,
            Panel::Inspector => Slide::Adjust,
        }
    }

    fn index(self) -> usize {
        self as usize
    }
}

/// A slide under way: from and to (points), the size it opens to, and its
/// motion once its first frame has given it a start.
#[derive(Clone, Copy, Debug)]
struct Moving {
    from: f64,
    to: f64,
    open: f64,
    motion: Option<Motion>,
}

impl Moving {
    fn value(&self, now: f64) -> f64 {
        self.motion.map_or(self.from, |m| m.value(now))
    }
}

/// The slides under way, and the frame that moves them.
#[derive(Default)]
pub(crate) struct PanelMotions {
    moving: [Option<Moving>; 3],
    frame: NextFrame,
    last_time: f64,
}

impl PanelMotions {
    pub(crate) fn moving(&self, slide: Slide) -> bool {
        self.moving[slide.index()].is_some()
    }
}

impl App {
    /// The size `slide` shows now: where a slide under way has got to, or
    /// open or folded.
    fn slide_size(&mut self, cx: &mut Cx, slide: Slide, open: bool) -> f64 {
        match self.motions.moving[slide.index()] {
            Some(moving) => moving.value(self.motions.last_time),
            None if open => self.open_size(cx, slide),
            None => 0.0,
        }
    }

    /// What `slide` opens to: the panel's width (the splitter keeps it while
    /// folded), or the lanes' height as last drawn.
    fn open_size(&mut self, cx: &mut Cx, slide: Slide) -> f64 {
        match slide {
            Slide::Setup => match self.ui.splitter(cx, ids!(main_split)).align() {
                Some(SplitterAlign::FromA(width)) => width,
                _ => 0.0,
            },
            Slide::Adjust => match self.ui.splitter(cx, ids!(inner_split)).align() {
                Some(SplitterAlign::FromB(width)) => width,
                _ => 0.0,
            },
            Slide::Lanes => self
                .ui
                .widget(cx, ids!(lanes))
                .borrow::<RecoPanelBox>()
                .map_or(0.0, |b| b.natural_height()),
        }
    }

    /// A toggle: slide from what shows to open or folded. Call before
    /// `apply_shell`, which leaves a sliding panel alone.
    pub(crate) fn start_slide(&mut self, cx: &mut Cx, slide: Slide, was_open: bool, open: bool) {
        let from = self.slide_size(cx, slide, was_open);
        let size = match self.motions.moving[slide.index()] {
            Some(moving) => moving.open,
            None => self.open_size(cx, slide),
        };
        let to = if open { size } else { 0.0 };
        if size <= 0.0 || (from - to).abs() < 0.5 {
            // Nothing to slide (the lanes not drawn yet): `apply_shell` does
            // it at once, ending a slide under way first.
            if self.motions.moving(slide) {
                self.end_slide(cx, slide);
            }
            return;
        }
        if self.motions.moving[slide.index()].is_none() {
            self.begin_slide(cx, slide, from, size);
        }
        self.motions.moving[slide.index()] = Some(Moving {
            from,
            to,
            open: size,
            motion: None,
        });
        self.motions.frame = cx.new_next_frame();
    }

    /// Ready a panel to slide: its splitter's floors relaxed and the pane
    /// unfolded at `from`, its content held at the open size.
    fn begin_slide(&mut self, cx: &mut Cx, slide: Slide, from: f64, open: f64) {
        match slide {
            Slide::Setup | Slide::Adjust => {
                let mut split = self.split_of(cx, slide);
                script_apply_eval!(cx, split, {
                    min_horizontal: 0.0 min_vertical: 0.0 max_horizontal: 0.0 max_vertical: 0.0
                });
                self.place_bar(cx, slide, from);
                self.ui
                    .splitter(cx, Self::split_id(slide))
                    .set_collapse(cx, SplitterCollapse::None);
                self.pin_box(cx, slide, Some(open));
            }
            Slide::Lanes => {
                self.set_lanes_height(cx, Some(from));
                self.ui.widget(cx, ids!(lanes)).set_visible(cx, true);
            }
        }
    }

    /// A frame: every slide moves on, and one that is over ends.
    pub(crate) fn slide_frame(&mut self, cx: &mut Cx, event: &Event) {
        let Some(frame) = self.motions.frame.is_event(event) else {
            return;
        };
        let now = frame.time;
        self.motions.last_time = now;
        for slide in Slide::ALL {
            let Some(mut moving) = self.motions.moving[slide.index()] else {
                continue;
            };
            let motion = *moving
                .motion
                .get_or_insert(Motion::panel(moving.from, moving.to, now));
            self.motions.moving[slide.index()] = Some(moving);
            let value = motion.value(now);
            match slide {
                Slide::Setup | Slide::Adjust => {
                    self.place_bar(cx, slide, value);
                    self.redraw_box(cx, slide);
                }
                Slide::Lanes => self.set_lanes_height(cx, Some(value)),
            }
            if motion.done(now) {
                self.end_slide(cx, slide);
            }
        }
        if self.motions.moving.iter().any(Option::is_some) {
            self.motions.frame = cx.new_next_frame();
        }
    }

    /// End a slide where it is going: floors back, the bar at the open
    /// size, folded or open as the shell says.
    pub(crate) fn end_slide(&mut self, cx: &mut Cx, slide: Slide) {
        let open = self.motions.moving[slide.index()]
            .map(|m| m.open)
            .filter(|open| *open > 0.0);
        self.motions.moving[slide.index()] = None;
        match slide {
            Slide::Setup | Slide::Adjust => {
                let mut split = self.split_of(cx, slide);
                match slide {
                    Slide::Setup => script_apply_eval!(cx, split, {
                        use mod.prelude.widgets.*
                        min_horizontal: theme.reco_media_min min_vertical: theme.reco_media_min
                        max_horizontal: theme.reco_viewer_min max_vertical: theme.reco_viewer_min
                    }),
                    _ => script_apply_eval!(cx, split, {
                        use mod.prelude.widgets.*
                        min_horizontal: theme.reco_viewer_min min_vertical: theme.reco_viewer_min
                        max_horizontal: theme.reco_inspector_floor max_vertical: theme.reco_inspector_floor
                    }),
                }
                if let Some(open) = open {
                    self.place_bar(cx, slide, open);
                }
                self.pin_box(cx, slide, None);
            }
            Slide::Lanes => self.set_lanes_height(cx, None),
        }
        self.apply_shell(cx);
    }

    /// Every slide ends at once (the window changed size).
    pub(crate) fn end_slides(&mut self, cx: &mut Cx) {
        for slide in Slide::ALL {
            if self.motions.moving(slide) {
                self.end_slide(cx, slide);
            }
        }
    }

    fn split_id(slide: Slide) -> &'static [LiveId] {
        match slide {
            Slide::Setup => ids!(main_split),
            _ => ids!(inner_split),
        }
    }

    fn split_of(&mut self, cx: &mut Cx, slide: Slide) -> WidgetRef {
        self.ui.widget(cx, Self::split_id(slide))
    }

    /// The bar where the sliding pane is `size` wide.
    fn place_bar(&mut self, cx: &mut Cx, slide: Slide, size: f64) {
        let align = match slide {
            Slide::Setup => SplitterAlign::FromA(size),
            _ => SplitterAlign::FromB(size),
        };
        self.ui
            .splitter(cx, Self::split_id(slide))
            .set_align(cx, align);
    }

    fn box_id(slide: Slide) -> &'static [LiveId] {
        match slide {
            Slide::Setup => ids!(setup_box),
            Slide::Adjust => ids!(adjust_box),
            Slide::Lanes => ids!(lanes),
        }
    }

    fn pin_box(&mut self, cx: &mut Cx, slide: Slide, width: Option<f64>) {
        if let Some(mut panel) = self
            .ui
            .widget(cx, Self::box_id(slide))
            .borrow_mut::<RecoPanelBox>()
        {
            panel.set_pinned_width(cx, width);
        }
    }

    /// The side panels keep their own draw lists: each frame asks them to
    /// draw again where the bar now is.
    fn redraw_box(&mut self, cx: &mut Cx, slide: Slide) {
        self.ui.widget(cx, Self::box_id(slide)).redraw(cx);
    }

    fn set_lanes_height(&mut self, cx: &mut Cx, height: Option<f64>) {
        if let Some(mut lanes) = self.ui.widget(cx, ids!(lanes)).borrow_mut::<RecoPanelBox>() {
            lanes.set_height(cx, height);
        }
    }
}
