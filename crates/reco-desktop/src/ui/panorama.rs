//! The panorama frame shown in the empty states: the left and right camera
//! halves of the future stitched picture, meeting at the seam. A half lights
//! up once its camera has videos; the seam lights up while calibrating.
//! The frame keeps the panorama's aspect at any window width.

use makepad_widgets::*;

script_mod! {
    use mod.prelude.widgets_internal.*
    use mod.widgets.*

    set_type_default() do #(DrawPanorama::script_shader(vm)){
        ..mod.draw.DrawQuad
        left_on: 0.0
        right_on: 0.0
        seam_on: 0.0
        line_color: uniform(theme.reco_frame_line)
        fill_color: uniform(theme.reco_frame_fill)
        lit_color: uniform(theme.reco_frame_lit)
        seam_color: uniform(theme.reco_seam)
        seam_idle_color: uniform(theme.reco_seam_idle)
        radius: uniform(theme.reco_panorama_radius)
        pixel: fn() {
            let size = self.rect_size
            let p = self.pos * size
            let sdf = Sdf2d.viewport(p)
            let lit = mix(self.left_on, self.right_on, step(size.x * 0.5, p.x))
            sdf.box(1., 1., size.x - 2., size.y - 2., self.radius)
            sdf.fill_keep(mix(self.fill_color, self.lit_color, lit))
            sdf.stroke(self.line_color, 1.)
            let base = sdf.result
            let inside = step(3., p.y) * step(p.y, size.y - 3.)
            let seam = (1. - smoothstep(0.75, 1.75, abs(p.x - size.x * 0.5))) * inside
            let seam_rgb = mix(self.seam_idle_color, self.seam_color, self.seam_on)
            return vec4(mix(base.rgb, seam_rgb.rgb, seam), max(base.a, seam))
        }
    }

    mod.widgets.RecoPanoramaBase = #(RecoPanorama::register_widget(vm))
    mod.widgets.RecoPanorama = set_type_default() do mod.widgets.RecoPanoramaBase{
        width: Fill
        height: Fit
        aspect: theme.reco_panorama_aspect
        max_width: theme.reco_panorama_max_width
        // Subdued text stays above 4.5:1 on the lit green halves too.
        draw_label +: {
            color: theme.reco_text_subdued
            text_style: theme.font_regular{font_size: theme.reco_font_small}
        }
    }
}

/// The frame's shader: halves and seam are lit by instance values.
#[derive(Script, ScriptHook)]
#[repr(C)]
pub struct DrawPanorama {
    #[deref]
    draw_super: DrawQuad,
    #[live]
    left_on: f32,
    #[live]
    right_on: f32,
    #[live]
    seam_on: f32,
}

/// A frame of `aspect`:1 fitted into the available width (at most
/// `max_width`), with its two halves labelled.
#[derive(Script, ScriptHook, Widget)]
pub struct RecoPanorama {
    #[uid]
    uid: WidgetUid,
    #[source]
    source: ScriptObjectRef,
    #[walk]
    walk: Walk,
    #[redraw]
    #[live]
    draw_frame: DrawPanorama,
    #[live]
    draw_label: DrawText,
    #[live]
    aspect: f64,
    #[live]
    max_width: f64,
    #[rust]
    area: Area,
}

impl Widget for RecoPanorama {
    fn handle_event(&mut self, _cx: &mut Cx, _event: &Event, _scope: &mut Scope) {}

    fn draw_walk(&mut self, cx: &mut Cx2d, _scope: &mut Scope, walk: Walk) -> DrawStep {
        cx.begin_turtle(walk, Layout::default());
        let available = cx.turtle().rect().size.x.max(0.0);
        let width = available.min(self.max_width);
        let height = width / self.aspect.max(1.0);
        let row = cx.walk_turtle(Walk::fixed(available, height));
        let frame = Rect {
            pos: dvec2(row.pos.x + (available - width) / 2.0, row.pos.y),
            size: dvec2(width, height),
        };
        self.draw_frame.draw_abs(cx, frame);
        for (index, text) in ["Left camera", "Right camera"].into_iter().enumerate() {
            let laid = self
                .draw_label
                .layout(cx, 0.0, 0.0, None, false, Align::default(), text);
            let size = dvec2(
                laid.size_in_lpxs.width as f64,
                laid.size_in_lpxs.height as f64,
            );
            let centre = dvec2(
                frame.pos.x + frame.size.x * (0.25 + 0.5 * index as f64),
                frame.pos.y + frame.size.y / 2.0,
            );
            self.draw_label.draw_abs(cx, centre - size / 2.0, text);
        }
        cx.end_turtle_with_area(&mut self.area);
        DrawStep::done()
    }
}

impl RecoPanorama {
    /// Light the halves whose camera has videos, and the seam.
    pub fn set_lit(&mut self, cx: &mut Cx, left: bool, right: bool, seam: bool) {
        let on = |lit: bool| if lit { 1.0 } else { 0.0 };
        self.draw_frame.left_on = on(left);
        self.draw_frame.right_on = on(right);
        self.draw_frame.seam_on = on(seam);
        self.draw_frame.redraw(cx);
    }
}
