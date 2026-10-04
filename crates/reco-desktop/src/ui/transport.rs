//! The transport bar. Module 0 shows its layout; Module 2 wires it.

use makepad_widgets::*;

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    // Timecode is measurement, so it uses the monospaced face.
    let Time = RecoText{
        width: theme.reco_time_width
        draw_text +: {color: theme.reco_text_secondary text_style: theme.font_code{font_size: theme.reco_font_meta}}
    }

    mod.widgets.RecoTransport = SolidView{
        width: Fill height: theme.reco_transport_height flow: Right spacing: theme.reco_space_s
        align: Align{y: 0.5}
        padding: Inset{left: theme.reco_space_xl right: theme.reco_space_xl}
        draw_bg.color: theme.reco_panel
        // Everything starts disabled (no flash at startup); the app enables
        // it once a stitched preview exists.
        Tip{text: "Back one frame"
            step_back := RecoIconButton{
                animator +: {disabled: {default: @on}}
                draw_icon +: {svg: crate_resource("self:resources/icons/step_back.svg")}
            }
        }
        Tip{text: "Play or pause (Space)"
            play_pause := RecoIconButton{
                animator +: {disabled: {default: @on}}
                width: theme.reco_play_button height: theme.reco_play_button
                icon_walk: Walk{width: theme.reco_play_icon height: theme.reco_play_icon}
                draw_icon +: {svg: crate_resource("self:resources/icons/play.svg") color: theme.color_text}
            }
        }
        Tip{text: "Forward one frame"
            step_forward := RecoIconButton{
                animator +: {disabled: {default: @on}}
                draw_icon +: {svg: crate_resource("self:resources/icons/step_forward.svg")}
            }
        }
        time_current := Time{text: "0:00"}
        timeline := RecoTimeline{
            animator +: {disabled: {default: @on}}
            min: 0.0 max: 1.0 default: 0.0
            // The time labels show the position; hide the slider's own
            // value field, its caret included.
            text_input +: {
                is_read_only: true
                draw_text +: {
                    color: theme.reco_transparent color_hover: theme.reco_transparent
                    color_focus: theme.reco_transparent
                    color_down: theme.reco_transparent color_disabled: theme.reco_transparent
                }
                draw_cursor +: {color: theme.reco_transparent}
            }
        }
        time_total := Time{text: "0:00"}
        Tip{text: "Record the preview as you watch"
            record_button := RecoButton{
                animator +: {disabled: {default: @on}}
                text: "Record"
                icon_walk: Walk{width: theme.reco_icon_tiny height: theme.reco_icon_tiny}
                draw_icon +: {svg: crate_resource("self:resources/icons/record.svg") color: theme.reco_record}
            }
        }
        RecoMuted{text: "Preview"}
        aspect := RecoDropDown{
            animator +: {disabled: {default: @on}}
            width: theme.reco_aspect_width labels: ["Auto" "16:9" "4:3" "21:9"]
        }
    }
}
