//! The Inspector: view, stitching, lens and stats. Module 0 shows its layout
//! only; Modules 4 and 5 wire it.

use makepad_widgets::*;

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    mod.widgets.RecoInspector = SolidView{
        width: Fill height: Fill flow: Down
        draw_bg.color: theme.reco_panel
        ScrollYView{
            width: Fill height: Fill flow: Down spacing: theme.reco_gap
            padding: Inset{left: 6 right: 10 top: 10 bottom: 10}
            RecoPanelHeader{text: "Adjust"}
            RecoSection{
                RecoSectionTitle{text: "VIEW"}
                fov_slider := Slider{text: "Field of view" min: 20.0 max: 150.0 default: 75.0 precision: 0}
                constrained_look := CheckBox{text: "Constrained look"}
                reset_view := RecoButton{text: "Reset view"}
            }
            RecoSection{
                RecoSectionTitle{text: "STITCHING"}
                seam_blend := Slider{text: "Seam blend" min: 0.0 max: 0.3 default: 0.05 precision: 2}
                match_colours := CheckBox{text: "Match colours" animator +: {active: {default: @on}}}
                rig_tilt := Slider{text: "Rig tilt (°)" min: -30.0 max: 30.0 default: 0.0 precision: 1}
                rig_roll := Slider{text: "Rig roll (°)" min: -15.0 max: 15.0 default: 0.0 precision: 1}
            }
            RecoSection{
                RecoSectionTitle{text: "LENS"}
                RecoMuted{text: "Auto-calibrate to detect the lens"}
            }
            RecoSection{
                RecoSectionTitle{text: "STATS"}
                RecoMuted{text: "No playback yet"}
            }
        }
    }
}
