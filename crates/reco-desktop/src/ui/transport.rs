//! The transport bar. Module 0 shows its layout only; Module 2 wires it.

use makepad_widgets::*;

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    let Time = Label{
        width: theme.reco_time_width
        draw_text +: {color: theme.reco_text_secondary text_style: theme.font_code{font_size: theme.reco_font_small}}
    }

    mod.widgets.RecoTransport = SolidView{
        width: Fill height: theme.reco_transport_height flow: Right spacing: theme.reco_space_s
        align: Align{y: 0.5}
        padding: Inset{left: theme.reco_space_xl right: theme.reco_space_xl}
        draw_bg.color: theme.reco_panel
        // Everything starts disabled (no flash at startup); the app enables
        // it once files are loaded.
        step_back := RecoIconButton{
            animator +: {disabled: {default: @on}}
            draw_icon +: {svg: crate_resource("self:resources/icons/step_back.svg")}
        }
        play_pause := RecoIconButton{
            animator +: {disabled: {default: @on}}
            width: theme.reco_play_button height: theme.reco_play_button
            icon_walk: Walk{width: theme.reco_play_icon height: theme.reco_play_icon}
            draw_icon +: {svg: crate_resource("self:resources/icons/play.svg") color: theme.color_text}
        }
        step_forward := RecoIconButton{
            animator +: {disabled: {default: @on}}
            draw_icon +: {svg: crate_resource("self:resources/icons/step_forward.svg")}
        }
        time_current := Time{text: "0:00"}
        timeline := SliderMinimal{
            animator +: {disabled: {default: @on}}
            width: Fill text: "" min: 0.0 max: 1.0 default: 0.0
            // The time labels show the position; hide the slider's own value.
            text_input +: {
                is_read_only: true
                draw_text +: {
                    color: theme.reco_transparent color_hover: theme.reco_transparent
                    color_focus: theme.reco_transparent
                    color_down: theme.reco_transparent color_disabled: theme.reco_transparent
                }
            }
        }
        time_total := Time{text: "0:00"}
        record_button := RecoButton{
            animator +: {disabled: {default: @on}}
            text: "Rec"
            icon_walk: Walk{width: theme.reco_icon_tiny height: theme.reco_icon_tiny}
            draw_icon +: {svg: crate_resource("self:resources/icons/record.svg") color: theme.reco_record}
        }
        aspect := DropDown{
            animator +: {disabled: {default: @on}}
            width: theme.reco_aspect_width labels: ["Auto" "16:9" "4:3" "21:9"]
        }
    }
}
