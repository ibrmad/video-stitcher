//! The Media sidebar: left and right camera videos, and calibration.
//! Module 0 shows its layout only; Module 3 wires it.

use makepad_widgets::*;

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    let AddVideos = RecoButton{
        text: "Add videos…"
        icon_walk: Walk{width: theme.reco_icon_small height: theme.reco_icon_small}
        draw_icon +: {svg: crate_resource("self:resources/icons/plus.svg")}
    }

    mod.widgets.RecoMediaPanel = SolidView{
        width: Fill height: Fill flow: Down
        draw_bg.color: theme.reco_panel
        // Scrolls, so nothing is cut off on short windows.
        ScrollYView{
            width: Fill height: Fill flow: Down spacing: theme.reco_space_m
            padding: Inset{left: theme.reco_space_l right: theme.reco_space_s top: theme.reco_space_l bottom: theme.reco_space_l}
            RecoPanelHeader{text: "Media"}
            left_camera := RecoSection{
                RecoSectionTitle{text: "LEFT CAMERA"}
                left_empty := RecoMuted{text: "No video yet"}
                add_left := AddVideos{}
            }
            right_camera := RecoSection{
                RecoSectionTitle{text: "RIGHT CAMERA"}
                right_empty := RecoMuted{text: "No video yet"}
                add_right := AddVideos{}
            }
            calibration := RecoSection{
                RecoSectionTitle{text: "CALIBRATION"}
                calibration_status := RecoMuted{text: "Add both cameras to calibrate"}
                View{
                    width: Fill height: Fit flow: Right spacing: theme.reco_space_s
                    // Disabled until both cameras have videos.
                    auto_calibrate := RecoPrimaryButton{
                        animator +: {disabled: {default: @on}}
                        text: "Auto-calibrate"
                    }
                    load_calibration := RecoButton{text: "Load…"}
                }
            }
            recent_button := ButtonFlat{
                text: "Recent files…"
                icon_walk: Walk{width: theme.reco_icon_small height: theme.reco_icon_small}
                draw_icon +: {svg: crate_resource("self:resources/icons/folder.svg")}
            }
        }
    }
}
