//! Shared building blocks: text roles, panel headers, sections, buttons.

use makepad_widgets::*;

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    mod.widgets.RecoTitle = Label{
        draw_text +: {color: theme.color_text text_style: theme.font_bold{font_size: theme.reco_font_title}}
    }
    mod.widgets.RecoBody = Label{
        draw_text +: {color: theme.color_text text_style: theme.font_regular{font_size: theme.reco_font_body}}
    }
    mod.widgets.RecoMuted = Label{
        draw_text +: {color: theme.reco_text_muted text_style: theme.font_regular{font_size: theme.reco_font_small}}
    }
    mod.widgets.RecoSectionTitle = Label{
        draw_text +: {color: theme.reco_text_secondary text_style: theme.font_bold{font_size: theme.reco_font_caption}}
    }
    mod.widgets.RecoPanelHeader = Label{
        margin: Inset{left: theme.reco_space_xs top: theme.reco_space_hair bottom: theme.reco_space_hair}
        draw_text +: {color: theme.color_text text_style: theme.font_bold{font_size: theme.reco_font_heading}}
    }
    mod.widgets.RecoSection = RoundedView{
        width: Fill height: Fit flow: Down spacing: theme.reco_space_m
        padding: Inset{left: theme.reco_space_xl right: theme.reco_space_xl top: theme.reco_space_l bottom: theme.reco_space_xl}
        show_bg: true new_batch: true
        draw_bg +: {color: theme.reco_surface border_radius: theme.container_corner_radius}
    }
    // Makepad's disabled state dims a button's face and label but not its
    // icon, so Reco's buttons fade the icon as well.
    mod.widgets.RecoIconButton = ButtonIcon{
        width: theme.reco_icon_button height: theme.reco_icon_button padding: 0 margin: 0 text: ""
        icon_walk: Walk{width: theme.reco_icon height: theme.reco_icon}
        draw_icon +: {color: theme.reco_text_secondary}
        animator +: {
            disabled +: {
                off +: {apply +: {draw_icon: {opacity: theme.reco_icon_opacity}}}
                on +: {apply +: {draw_icon: {opacity: theme.reco_disabled_icon_opacity}}}
            }
        }
    }
    mod.widgets.RecoButton = Button{
        height: theme.reco_control_height
        animator +: {
            disabled +: {
                off +: {apply +: {draw_icon: {opacity: theme.reco_icon_opacity}}}
                on +: {apply +: {draw_icon: {opacity: theme.reco_disabled_icon_opacity}}}
            }
        }
    }
    mod.widgets.RecoPrimaryButton = ButtonPrimary{
        height: theme.reco_primary_height padding: Inset{left: theme.reco_button_pad_x right: theme.reco_button_pad_x}
    }
}
