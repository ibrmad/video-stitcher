//! Shared building blocks: text roles, panel headers, sections, buttons.

use makepad_widgets::*;

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    mod.widgets.RecoTitle = Label{
        draw_text +: {color: theme.color_text text_style: theme.font_bold{font_size: 11.0}}
    }
    mod.widgets.RecoBody = Label{
        draw_text +: {color: theme.color_text text_style: theme.font_regular{font_size: 10.0}}
    }
    mod.widgets.RecoMuted = Label{
        draw_text +: {color: theme.reco_text_muted text_style: theme.font_regular{font_size: 9.5}}
    }
    mod.widgets.RecoSectionTitle = Label{
        draw_text +: {color: theme.reco_text_secondary text_style: theme.font_bold{font_size: 8.5}}
    }
    mod.widgets.RecoPanelHeader = Label{
        margin: Inset{left: 4 top: 2 bottom: 2}
        draw_text +: {color: theme.color_text text_style: theme.font_bold{font_size: 12.0}}
    }
    mod.widgets.RecoSection = RoundedView{
        width: Fill height: Fit flow: Down spacing: theme.reco_gap
        padding: Inset{left: 12 right: 12 top: 10 bottom: 12}
        show_bg: true new_batch: true
        draw_bg +: {color: theme.reco_surface border_radius: theme.container_corner_radius}
    }
    // Makepad's disabled state dims a button's face and label but not its
    // icon, so Reco's buttons fade the icon as well.
    mod.widgets.RecoIconButton = ButtonIcon{
        width: 30 height: 30 padding: 0 margin: 0 text: ""
        icon_walk: Walk{width: 16 height: 16}
        draw_icon +: {color: theme.reco_text_secondary}
        animator +: {
            disabled +: {
                off +: {apply +: {draw_icon: {opacity: 1.0}}}
                on +: {apply +: {draw_icon: {opacity: theme.reco_disabled_icon_opacity}}}
            }
        }
    }
    mod.widgets.RecoButton = Button{
        height: 28
        animator +: {
            disabled +: {
                off +: {apply +: {draw_icon: {opacity: 1.0}}}
                on +: {apply +: {draw_icon: {opacity: theme.reco_disabled_icon_opacity}}}
            }
        }
    }
    mod.widgets.RecoPrimaryButton = ButtonPrimary{
        height: 30 padding: Inset{left: 14 right: 14}
    }
}
