//! The lookahead slider's track, after the Slint app's VRAM risk slider:
//! comfortable up to one mark, tight up to the next, too long past it.
//! The zones show dimmed along the track, and the part up to the knob is
//! filled in the colour of the zone the knob is in. A knob-only slider
//! sits over it (`RecoKnobSlider`). Without a reading of the GPU's memory
//! it is the plain track every slider has, filled in the accent.

use makepad_widgets::*;

script_mod! {
    use mod.prelude.widgets_internal.*
    use mod.widgets.*

    set_type_default() do #(DrawZones::script_shader(vm)){
        ..mod.draw.DrawQuad
        value_end: 0.0
        safe_end: 0.0
        tight_end: 0.0
        zone: 0.0
        zoned: 0.0
        plain_color: uniform(theme.reco_widget)
        value_color: uniform(theme.reco_accent)
        safe_color: uniform(theme.reco_zone_safe)
        tight_color: uniform(theme.reco_zone_tight)
        risk_color: uniform(theme.reco_zone_risk)
        zone_dim: uniform(theme.reco_zone_dim)
        thickness: uniform(theme.reco_slider_track)
        // The knob's travel ends a radius in from each side.
        inset: uniform(theme.reco_slider_knob)
        pixel: fn() {
            let p = self.pos * self.rect_size
            let sdf = Sdf2d.viewport(p)
            let t = self.thickness
            let x0 = self.inset
            let w = self.rect_size.x - 2.0 * self.inset
            let along = (p.x - x0) / w
            // The zone under this pixel, and the knob's.
            let here = step(self.safe_end, along) + step(self.tight_end, along)
            let zone_here = mix(
                mix(self.safe_color, self.tight_color, step(0.5, here))
                self.risk_color
                step(1.5, here)
            )
            let zone_knob = mix(
                mix(self.safe_color, self.tight_color, step(0.5, self.zone))
                self.risk_color
                step(1.5, self.zone)
            )
            let track = mix(
                self.plain_color
                vec4(zone_here.rgb, zone_here.a * self.zone_dim)
                self.zoned
            )
            let value = mix(self.value_color, zone_knob, self.zoned)
            let filled = 1.0 - step(self.value_end, along)
            sdf.box(x0, self.rect_size.y * 0.5 - t * 0.5, w, t, t * 0.25)
            sdf.fill(mix(track, value, filled))
            return sdf.result
        }
    }

    mod.widgets.RecoZonesBase = #(RecoZones::register_widget(vm))
    mod.widgets.RecoZones = set_type_default() do mod.widgets.RecoZonesBase{
        width: Fill
        height: theme.reco_row
    }
}

/// The track's shader: the knob's place, where the comfortable and tight
/// zones end (fractions of the track), the knob's zone, and whether there
/// are zones at all.
#[derive(Script, ScriptHook)]
#[repr(C)]
pub struct DrawZones {
    #[deref]
    draw_super: DrawQuad,
    #[live]
    value_end: f32,
    #[live]
    safe_end: f32,
    #[live]
    tight_end: f32,
    #[live]
    zone: f32,
    #[live]
    zoned: f32,
}

/// A slider track in zones.
#[derive(Script, ScriptHook, Widget)]
pub struct RecoZones {
    #[uid]
    uid: WidgetUid,
    #[source]
    source: ScriptObjectRef,
    #[walk]
    walk: Walk,
    #[redraw]
    #[live]
    draw_zones: DrawZones,
    #[visible]
    #[live(true)]
    visible: bool,
}

impl Widget for RecoZones {
    fn handle_event(&mut self, _cx: &mut Cx, _event: &Event, _scope: &mut Scope) {}

    fn draw_walk(&mut self, cx: &mut Cx2d, _scope: &mut Scope, walk: Walk) -> DrawStep {
        self.draw_zones.draw_walk(cx, walk);
        DrawStep::done()
    }
}

impl RecoZones {
    /// The track for a knob at `value` (a fraction of the track): zones
    /// ending at `zones` (fractions, the tight one never before the safe
    /// one) with the knob in `zone` (0 comfortable, 1 tight, 2 too long),
    /// or the plain track for `None`.
    pub fn set_track(&mut self, cx: &mut Cx, value: f64, zones: Option<(f64, f64)>, zone: usize) {
        let (safe, tight, zoned) = match zones {
            Some((safe, tight)) => {
                let safe = safe.clamp(0.0, 1.0);
                (safe, tight.clamp(safe, 1.0), 1.0)
            }
            None => (0.0, 0.0, 0.0),
        };
        self.draw_zones.value_end = value.clamp(0.0, 1.0) as f32;
        self.draw_zones.safe_end = safe as f32;
        self.draw_zones.tight_end = tight as f32;
        self.draw_zones.zone = zone.min(2) as f32;
        self.draw_zones.zoned = zoned;
        self.draw_zones.redraw(cx);
    }
}
