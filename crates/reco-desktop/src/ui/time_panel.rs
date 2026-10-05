//! The time panel, after Rerun's: a control row (step, play, the time and a
//! status line), then the time ruler with one lane per camera. A lane shows
//! the camera's files as blocks with a gap at each file boundary; the white
//! playhead crosses ruler and lanes. It shows only what exists: the lanes
//! once a camera has video, the time once there is a stitch to play. The
//! lanes also fold away with the panel toggle. Module 0 shows sample data;
//! Module 2 wires playback and scrubbing.

use crate::time_ruler::{clock, ticks};
use makepad_widgets::*;

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    mod.widgets.RecoTimeRulerBase = #(RecoTimeRuler::register_widget(vm))
    mod.widgets.RecoTimeRuler = set_type_default() do mod.widgets.RecoTimeRulerBase{
        width: Fill
        height: theme.reco_ruler_height + theme.reco_lane_row * 2.0
        ruler_height: theme.reco_ruler_height
        lane_height: theme.reco_lane_row
        lane_inset: theme.reco_lane_inset
        chapter_gap: theme.reco_chapter_gap
        tick_length: theme.reco_tick_length
        tick_spacing: theme.reco_tick_spacing
        label_offset: theme.reco_gap_s
        playhead_width: theme.reco_playhead_width
        playhead_head: theme.reco_playhead_head
        lane_color: theme.reco_lane_fill_on
        lane_empty_color: theme.reco_bar
        tick_color: theme.reco_tick
        playhead_color: theme.reco_playhead
        // A file block: rounded at the corners. Sdf2d.box draws twice the
        // radius it is given.
        draw_block +: {
            radius: uniform(theme.corner_radius)
            pixel: fn() {
                let sdf = Sdf2d.viewport(self.pos * self.rect_size)
                sdf.box(0., 0., self.rect_size.x, self.rect_size.y, min(self.radius, self.rect_size.x * 0.25))
                sdf.fill(self.color)
                return sdf.result
            }
        }
        draw_line +: {
            pixel: fn() {
                return Pal.premul(self.color)
            }
        }
        // The playhead's handle in the ruler: a small downward triangle.
        draw_head +: {
            pixel: fn() {
                let sdf = Sdf2d.viewport(self.pos * self.rect_size)
                sdf.move_to(0., 0.)
                sdf.line_to(self.rect_size.x, 0.)
                sdf.line_to(self.rect_size.x * 0.5, self.rect_size.y)
                sdf.close_path()
                sdf.fill(self.color)
                return sdf.result
            }
        }
        draw_label +: {
            color: theme.reco_text_subdued
            text_style: theme.font_regular{font_size: theme.reco_font_small}
        }
    }

    // A lane's badge; the Setup panel names the cameras, a tooltip here.
    let LaneRow = View{
        width: Fit height: theme.reco_lane_row flow: Right align: Align{y: 0.5}
        padding: Inset{left: theme.reco_pad right: theme.reco_gap}
    }

    mod.widgets.RecoTimePanel = SolidView{
        width: Fill height: Fit flow: Down
        draw_bg.color: theme.reco_panel
        RecoSeparator{}
        controls := View{
            width: Fill height: theme.reco_controls_height flow: Right spacing: theme.reco_gap_s
            align: Align{y: 0.5}
            // The first glyph's ink starts on the content edge, above the
            // lane badges: less the button's inset and the glyph's own.
            padding: Inset{left: theme.reco_pad - (theme.reco_icon_button - theme.reco_icon) * 0.5 - theme.reco_glyph_inset right: theme.reco_pad}
            // Everything starts disabled (no flash at startup); the app
            // enables it once a stitched preview exists.
            Tip{text: "Back one frame"
                step_back := RecoIconButton{
                    animator +: {disabled: {default: @on}}
                    draw_icon +: {svg: crate_resource("self:resources/icons/step_back.svg")}
                }
            }
            Tip{text: "Play or pause (Space)"
                play_pause := RecoIconButton{
                    animator +: {disabled: {default: @on}}
                    draw_icon +: {svg: crate_resource("self:resources/icons/play.svg") color: theme.reco_text_strong}
                }
            }
            Tip{text: "Forward one frame"
                step_forward := RecoIconButton{
                    animator +: {disabled: {default: @on}}
                    draw_icon +: {svg: crate_resource("self:resources/icons/step_forward.svg")}
                }
            }
            time_display := View{
                width: Fit height: Fit flow: Right spacing: theme.reco_gap_s
                margin: Inset{left: theme.reco_gap}
                time_current := RecoStrong{text: "0:00"}
                RecoSubdued{text: "/"}
                time_total := RecoSubdued{text: "0:00"}
            }
            View{width: Fill height: Fit}
            status_text := RecoSubdued{text: ""}
        }
        lanes := View{
            width: Fill height: Fit flow: Right
            padding: Inset{right: theme.reco_pad bottom: theme.reco_gap_s}
            View{
                width: Fit height: Fit flow: Down
                View{width: Fit height: theme.reco_ruler_height}
                LaneRow{
                    Tip{text: "Left camera"
                        View{
                            width: Fit height: Fit
                            lane_left_badge := RecoBadge{label.text: "L"}
                            lane_left_badge_on := RecoBadgeOn{visible: false label.text: "L"}
                        }
                    }
                }
                LaneRow{
                    Tip{text: "Right camera"
                        View{
                            width: Fit height: Fit
                            lane_right_badge := RecoBadge{label.text: "R"}
                            lane_right_badge_on := RecoBadgeOn{visible: false label.text: "R"}
                        }
                    }
                }
            }
            // Full path: this block's `use mod.widgets.*` predates the
            // registration above.
            timeline := mod.widgets.RecoTimeRuler{}
        }
    }
}

/// The ruler and the camera lanes, drawn in one pass: ticks with clock
/// labels, each camera's files as blocks, and the playhead.
#[derive(Script, ScriptHook, Widget)]
pub struct RecoTimeRuler {
    #[uid]
    uid: WidgetUid,
    #[source]
    source: ScriptObjectRef,
    #[walk]
    walk: Walk,
    #[live]
    draw_block: DrawColor,
    #[live]
    draw_line: DrawColor,
    #[live]
    draw_head: DrawColor,
    #[live]
    draw_label: DrawText,
    #[live]
    ruler_height: f64,
    #[live]
    lane_height: f64,
    #[live]
    lane_inset: f64,
    #[live]
    chapter_gap: f64,
    #[live]
    tick_length: f64,
    #[live]
    tick_spacing: f64,
    #[live]
    label_offset: f64,
    #[live]
    playhead_width: f64,
    #[live]
    playhead_head: f64,
    #[live]
    lane_color: Vec4f,
    #[live]
    lane_empty_color: Vec4f,
    #[live]
    tick_color: Vec4f,
    #[live]
    playhead_color: Vec4f,
    /// Seconds the ruler spans.
    #[rust]
    duration: f64,
    /// Where the playhead stands, in seconds.
    #[rust]
    playhead: f64,
    /// Per lane, each file's (start, end) in seconds; empty for no video.
    #[rust]
    lanes: Vec<Vec<(f64, f64)>>,
    #[rust]
    disabled: bool,
    /// The whole ruler: what redraws, and what the remote snapshot reports.
    #[redraw]
    #[rust]
    area: Area,
}

impl RecoTimeRuler {
    /// Show a timeline: its length, the playhead, and each lane's files.
    pub fn set_timeline(
        &mut self,
        cx: &mut Cx,
        duration: f64,
        playhead: f64,
        lanes: Vec<Vec<(f64, f64)>>,
    ) {
        self.duration = duration;
        self.playhead = playhead;
        self.lanes = lanes;
        self.area.redraw(cx);
    }

    fn line(&mut self, cx: &mut Cx2d, rect: Rect, color: Vec4f) {
        self.draw_line.color = color;
        self.draw_line.draw_abs(cx, rect);
    }
}

impl Widget for RecoTimeRuler {
    fn handle_event(&mut self, _cx: &mut Cx, _event: &Event, _scope: &mut Scope) {}

    fn set_disabled(&mut self, cx: &mut Cx, disabled: bool) {
        self.disabled = disabled;
        self.area.redraw(cx);
    }

    fn disabled(&self, _cx: &Cx) -> bool {
        self.disabled
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, _scope: &mut Scope, walk: Walk) -> DrawStep {
        cx.begin_turtle(walk, Layout::default());
        let rect = cx.turtle().rect();
        let (x0, y0) = (rect.pos.x, rect.pos.y);
        let (width, height) = (rect.size.x.max(0.0), rect.size.y.max(0.0));
        cx.walk_turtle(Walk::fixed(width, height));
        let span = self.duration.max(1.0);
        let to_x = |t: f64| x0 + (t / span).clamp(0.0, 1.0) * width;

        // Ruler: a tick and a clock label every step, labels kept inside.
        let tick_top = y0 + self.ruler_height - self.tick_length;
        for t in ticks(self.duration, width, self.tick_spacing) {
            let x = to_x(t).round();
            self.line(
                cx,
                Rect {
                    pos: dvec2(x, tick_top),
                    size: dvec2(1.0, self.tick_length),
                },
                self.tick_color,
            );
            let text = clock(t);
            let laid = self
                .draw_label
                .layout(cx, 0.0, 0.0, None, false, Align::default(), &text);
            let label_width = laid.size_in_lpxs.width as f64;
            let label_x = x + self.label_offset;
            if label_x + label_width <= x0 + width {
                self.draw_label
                    .draw_abs(cx, dvec2(label_x, y0 + self.label_offset * 0.5), &text);
            }
        }

        // Lanes: each file a block, a gap at each boundary; an empty track
        // for a camera with no video.
        let lanes = std::mem::take(&mut self.lanes);
        for (index, files) in lanes.iter().enumerate() {
            let top = y0 + self.ruler_height + index as f64 * self.lane_height + self.lane_inset;
            let lane_h = (self.lane_height - 2.0 * self.lane_inset).max(1.0);
            if files.is_empty() {
                self.draw_block.color = self.lane_empty_color;
                self.draw_block.draw_abs(
                    cx,
                    Rect {
                        pos: dvec2(x0, top),
                        size: dvec2(width, lane_h),
                    },
                );
            }
            for &(start, end) in files {
                let (a, b) = (to_x(start), to_x(end));
                self.draw_block.color = self.lane_color;
                self.draw_block.draw_abs(
                    cx,
                    Rect {
                        pos: dvec2(a, top),
                        size: dvec2((b - a - self.chapter_gap).max(1.0), lane_h),
                    },
                );
            }
        }
        self.lanes = lanes;

        // The playhead, only while there is something to play.
        if !self.disabled && self.duration > 0.0 {
            let x = to_x(self.playhead);
            let head = self.playhead_head;
            self.line(
                cx,
                Rect {
                    pos: dvec2(x - self.playhead_width * 0.5, y0),
                    size: dvec2(self.playhead_width, height),
                },
                self.playhead_color,
            );
            self.draw_head.color = self.playhead_color;
            self.draw_head.draw_abs(
                cx,
                Rect {
                    pos: dvec2(x - head, y0),
                    size: dvec2(head * 2.0, head * 1.4),
                },
            );
        }
        cx.end_turtle_with_area(&mut self.area);
        DrawStep::done()
    }
}
