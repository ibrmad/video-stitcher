//! Toasts, drawn: four card slots in a layer over the viewer's canvas,
//! bottom right, so notices stay clear of the Adjust panel and the time
//! panel. A card is a floating panel as Rerun's menus are: a dot for its
//! kind, the title, a wrapping detail and a close button. The cards take
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
    }

    mod.widgets.RecoToasts = View{
        width: Fill height: Fill flow: Down spacing: theme.reco_gap
        align: Align{x: 1.0 y: 1.0}
        padding: Inset{right: theme.reco_pad bottom: theme.reco_pad}
        toast_0 := mod.widgets.RecoToastCard{}
        toast_1 := mod.widgets.RecoToastCard{}
        toast_2 := mod.widgets.RecoToastCard{}
        toast_3 := mod.widgets.RecoToastCard{}
    }
}
