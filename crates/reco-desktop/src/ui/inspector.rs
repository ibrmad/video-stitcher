//! The Adjust panel: what a volunteer changes, in collapsible sections, with
//! the rarely needed controls in a closed "Advanced" tier. Lens, field ROI
//! and diagnostics join in Modules 4 and 5 (diagnostics in their own drawer).

use makepad_widgets::*;

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    mod.widgets.RecoInspector = SolidView{
        width: Fill height: Fill flow: Down
        draw_bg.color: theme.reco_panel
        ScrollShadowView{
            width: Fill height: Fill flow: Down spacing: theme.reco_space_m
            padding: theme.reco_space_l
            fade_left: false
            fade_right: false
            adjust_header := RecoPanelHeader{label.text: "Adjust"}
            RecoSection{
                view_fold := RecoFold{
                    header +: {title +: {text: "View"}}
                    body +: {
                        fov_slider := RecoSlider{text: "Field of view" min: 20.0 max: 150.0 default: 75.0 precision: 0}
                        fov_hint := RecoHint{text: "Wider shows more of the pitch; narrower zooms in."}
                        reset_view := RecoButton{text: "Reset view"}
                        view_advanced := RecoAdvanced{
                            body +: {
                                constrained_look := RecoCheckBox{text: "Stay inside the panorama"}
                                RecoHint{text: "Stops the view from turning past the edges of the stitched picture."}
                            }
                        }
                    }
                }
            }
            RecoSection{
                stitch_fold := RecoFold{
                    header +: {title +: {text: "Stitch"}}
                    body +: {
                        match_colours := RecoCheckBox{text: "Match colours" animator +: {active: {default: @on}}}
                        RecoHint{text: "Evens out brightness and colour between the two cameras."}
                        seam_blend := RecoSlider{text: "Seam blend" min: 0.0 max: 0.3 default: 0.05 precision: 2}
                        seam_hint := RecoHint{text: "How softly the two pictures merge where they meet."}
                        stitch_advanced := RecoAdvanced{
                            body +: {
                                rig_tilt := RecoSlider{text: "Tilt (°)" min: -30.0 max: 30.0 default: 0.0 precision: 1}
                                RecoHint{text: "Levels the picture if the pole leans forward or back."}
                                rig_roll := RecoSlider{text: "Roll (°)" min: -15.0 max: 15.0 default: 0.0 precision: 1}
                                RecoHint{text: "Levels the horizon if the pole leans to one side."}
                            }
                        }
                    }
                }
            }
        }
    }
}
