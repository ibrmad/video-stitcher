//! The Media sidebar: left and right camera videos, and calibration.
//! Module 0 shows its layout only; Module 3 wires it.

use makepad_widgets::*;

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    let AddVideos = RecoButton{
        text: "Add videos…"
        icon_walk: Walk{width: 12 height: 12}
        draw_icon +: {svg: crate_resource("self:resources/icons/plus.svg")}
    }

    mod.widgets.RecoMediaPanel = SolidView{
        width: Fill height: Fill flow: Down spacing: theme.reco_gap
        padding: Inset{left: 10 right: 6 top: 10 bottom: 10}
        draw_bg.color: theme.reco_panel
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
                width: Fill height: Fit flow: Right spacing: 6
                auto_calibrate := RecoPrimaryButton{text: "Auto-calibrate"}
                load_calibration := RecoButton{text: "Load…"}
            }
        }
        View{width: Fill height: Fill}
        recent_button := ButtonFlat{
            text: "Recent files…"
            icon_walk: Walk{width: 12 height: 12}
            draw_icon +: {svg: crate_resource("self:resources/icons/folder.svg")}
        }
    }
}
