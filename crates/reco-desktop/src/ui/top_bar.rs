//! The top bar, drawn inside the window's title bar, after Rerun's: the app
//! menu and the project on the left; Export and the panel toggles (Setup,
//! time panel, Adjust) on the right.

use makepad_widgets::*;

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    let PanelToggle = RecoIconButton{}

    mod.widgets.RecoTopBar = View{
        width: Fill height: Fill flow: Right spacing: theme.reco_gap_s
        align: Align{y: 0.5}
        // Room for the macOS window buttons.
        padding: Inset{left: theme.reco_caption_inset right: theme.reco_gap_s}
        // The app menu holds what Rerun keeps there: help, preferences, the
        // bug report and the version.
        app_menu := MenuButton{
            place: Below
            button := RecoFlatButton{
                text: "Reco"
                padding: Inset{left: theme.reco_gap_s right: theme.reco_gap_s}
                spacing: theme.reco_gap_xs
                draw_text +: {color: theme.reco_text_strong}
                icon_end_walk: Walk{width: theme.reco_icon_small height: theme.reco_icon_small}
                draw_icon_end +: {svg: crate_resource("self:resources/icons/chevron_down.svg") color: theme.reco_text_subdued}
            }
        }
        project_name := RecoSubdued{text: "No videos yet"}
        View{width: Fill height: Fit}
        // Starts disabled (no flash at startup); enabled once there is a
        // stitched preview to export.
        export_button := RecoPrimaryButton{
            animator +: {disabled: {default: @on}}
            text: "Export"
            margin: Inset{right: theme.reco_gap}
            draw_icon +: {svg: crate_resource("self:resources/icons/export_video.svg")}
        }
        Tip{text: "Setup panel (⌘1)"
            toggle_media := PanelToggle{draw_icon +: {svg: crate_resource("self:resources/icons/sidebar_left.svg")}}
        }
        Tip{text: "Time panel (⌘3)"
            toggle_timeline := PanelToggle{draw_icon +: {svg: crate_resource("self:resources/icons/panel_bottom.svg")}}
        }
        Tip{text: "Adjust panel (⌘2)"
            toggle_inspector := PanelToggle{
                animator +: {disabled: {default: @on}}
                draw_icon +: {svg: crate_resource("self:resources/icons/sidebar_right.svg")}
            }
        }
    }
}
