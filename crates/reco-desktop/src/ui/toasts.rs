//! Toasts, drawn: four card slots in a layer over the viewer's canvas,
//! bottom right, so notices stay clear of the Adjust panel and the time
//! panel. A card is a floating panel as Rerun's menus are: a dot for its
//! kind, the title, a wrapping detail, a button when the notice offers one
//! (Download), and a close button. The cards take
//! the pointer; the layer itself does not. The App fills the slots from
//! `reco_app::toasts` (toast_view.rs).

use makepad_widgets::*;

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    mod.widgets.RecoToastCard = RoundedView{
        visible: false
        width: theme.reco_toast_width height: Fit flow: Down spacing: theme.reco_gap_xs
        padding: Inset{left: theme.reco_pad right: theme.reco_gap_s top: theme.reco_gap_s bottom: theme.reco_gap}
        show_bg: true new_batch: true
        cursor: MouseCursor.Default capture_overload: true
        draw_bg +: {
            color: theme.reco_band
            border_radius: theme.container_corner_radius
            border_size: theme.reco_separator_width
            border_color: theme.reco_hover
        }
        View{
            width: Fill height: Fit flow: Right spacing: theme.reco_gap align: Align{y: 0.5}
            dot_info := RecoDot{}
            dot_warn := RecoDotBusy{visible: false}
            dot_error := RecoDotError{visible: false}
            title := RecoStrong{width: Fill text: ""}
            close := RecoRowIcon{
                draw_icon +: {svg: crate_resource("self:resources/icons/close.svg")}
            }
        }
        body_row := View{
            width: Fill height: Fit
            padding: Inset{left: theme.reco_dot + theme.reco_gap right: theme.reco_gap_s}
            body := RecoSubdued{width: Fill text: ""}
        }
        action_row := View{
            visible: false
            width: Fill height: Fit
            padding: Inset{left: theme.reco_dot + theme.reco_gap top: theme.reco_gap_s}
            action := RecoButton{text: ""}
        }
    }

    mod.widgets.RecoToastsBase = #(RecoToasts::register_widget(vm))

    mod.widgets.RecoToasts = set_type_default() do mod.widgets.RecoToastsBase{
        ..mod.widgets.View
        width: Fill height: Fill flow: Down spacing: theme.reco_gap
        align: Align{x: 1.0 y: 1.0}
        padding: Inset{right: theme.reco_pad bottom: theme.reco_pad}
        toast_0 := mod.widgets.RecoToastCard{}
        toast_1 := mod.widgets.RecoToastCard{}
        toast_2 := mod.widgets.RecoToastCard{}
        toast_3 := mod.widgets.RecoToastCard{}
    }
}

/// The layer the cards sit in. Each card keeps its own draw list, so it
/// draws over the picture; a change of the viewer's size doesn't reach those
/// lists, and a card stayed drawn where it was, under a panel that opened
/// (the owner saw it after a calibration). The layer notes its room as it
/// draws and, when it changes, asks its cards to draw again on the next frame
/// (a redraw asked for mid-draw is dropped, and a card's area is in the
/// layer's list, not its own).
#[derive(Script, ScriptHook, Widget)]
pub struct RecoToasts {
    #[source]
    source: ScriptObjectRef,
    #[deref]
    view: View,
    /// The room the cards were last drawn in.
    #[rust]
    room: Rect,
    /// The frame on which they draw again in a new room.
    #[rust]
    moved: NextFrame,
}

impl Widget for RecoToasts {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        if self.moved.is_event(event).is_some() {
            for id in [ids!(toast_0), ids!(toast_1), ids!(toast_2), ids!(toast_3)] {
                self.view.widget(cx, id).redraw(cx);
            }
        }
        self.view.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        let room = cx.turtle().inner_rect();
        if room != self.room {
            self.room = room;
            self.moved = cx.new_next_frame();
        }
        self.view.draw_walk(cx, scope, walk)
    }
}
