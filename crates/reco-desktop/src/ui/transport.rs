//! The transport bar. Module 0 shows its layout only; Module 2 wires it.

use makepad_widgets::*;

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    let Time = Label{
        width: 56
        draw_text +: {color: theme.reco_text_secondary text_style: theme.font_code{font_size: 9.5}}
    }

    mod.widgets.RecoTransport = SolidView{
        width: Fill height: theme.reco_transport_height flow: Right spacing: 6
        align: Align{y: 0.5}
        padding: Inset{left: 12 right: 12}
        draw_bg.color: theme.reco_panel
        // Everything starts disabled (no flash at startup); the app enables
        // it once files are loaded.
        step_back := RecoIconButton{
            animator +: {disabled: {default: @on}}
            draw_icon +: {svg: crate_resource("self:resources/icons/step_back.svg")}
        }
        play_pause := RecoIconButton{
            animator +: {disabled: {default: @on}}
            width: 36 height: 36
            icon_walk: Walk{width: 18 height: 18}
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
                    color: #x0000 color_hover: #x0000 color_focus: #x0000
                    color_down: #x0000 color_disabled: #x0000
                }
            }
        }
        time_total := Time{text: "0:00"}
        record_button := RecoButton{
            animator +: {disabled: {default: @on}}
            text: "Rec"
            icon_walk: Walk{width: 10 height: 10}
            draw_icon +: {svg: crate_resource("self:resources/icons/record.svg") color: theme.reco_record}
        }
        aspect := DropDown{
            animator +: {disabled: {default: @on}}
            width: 84 labels: ["Auto" "16:9" "4:3" "21:9"]
        }
    }
}
