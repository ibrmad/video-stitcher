//! The top bar, drawn inside the window's title bar.

use makepad_widgets::*;

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    mod.widgets.RecoTopBar = View{
        width: Fill height: Fill flow: Right spacing: theme.reco_space_xs
        align: Align{y: 0.5}
        // Room for the macOS window buttons.
        padding: Inset{left: theme.reco_caption_inset right: theme.reco_space_l}
        Tip{text: "Media panel (⌘1)"
            toggle_media := RecoIconButton{draw_icon +: {svg: crate_resource("self:resources/icons/sidebar_left.svg")}}
        }
        View{
            width: Fit height: Fit flow: Right spacing: theme.reco_space_l align: Align{y: 0.5}
            margin: Inset{left: theme.reco_space_s}
            app_name := RecoTitle{text: "Reco"}
            project_name := RecoMuted{text: "No videos loaded"}
        }
        View{width: Fill height: Fit}
        Tip{text: "Keyboard shortcuts"
            help_button := RecoIconButton{draw_icon +: {svg: crate_resource("self:resources/icons/help.svg")}}
        }
        Tip{text: "Preferences"
            prefs_button := RecoIconButton{draw_icon +: {svg: crate_resource("self:resources/icons/settings.svg")}}
        }
        // Starts disabled (no flash at startup); enabled once files load.
        export_button := RecoPrimaryButton{
            animator +: {disabled: {default: @on}}
            text: "Export"
            margin: Inset{left: theme.reco_space_s right: theme.reco_space_s}
            icon_walk: Walk{width: theme.reco_icon_medium height: theme.reco_icon_medium}
            draw_icon +: {svg: crate_resource("self:resources/icons/export.svg")}
        }
        Tip{text: "Inspector (⌘2)"
            toggle_inspector := RecoIconButton{
                animator +: {disabled: {default: @on}}
                draw_icon +: {svg: crate_resource("self:resources/icons/sidebar_right.svg")}
            }
        }
    }
}
