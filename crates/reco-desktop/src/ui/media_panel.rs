//! The Setup panel: the two cameras (as a joined pair) and calibration.
//! Module 0 shows its layout; Module 3 wires it.

use makepad_widgets::*;

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    // A camera with no videos offers Add; one with videos offers Change,
    // which picks that camera's files again.
    let AddButton = RecoButton{
        text: "Add…"
        draw_icon +: {svg: crate_resource("self:resources/icons/plus.svg")}
    }
    let ChangeButton = RecoButton{visible: false text: "Change…"}
    let CameraRow = View{
        width: Fill height: Fit flow: Right spacing: theme.reco_space_m align: Align{y: 0.5}
    }
    let CameraText = View{
        width: Fill height: Fit flow: Down spacing: theme.reco_space_hair
    }
    let CameraFiles = RecoMuted{
        width: Fill
        max_lines: 1
        text_overflow: TextOverflow.Ellipsis
    }
    // The join between the two cameras, centred under the badges: lit once
    // both have video.
    let Link = SolidView{
        width: theme.reco_link_width height: theme.reco_link_height
        margin: Inset{left: (theme.reco_badge - theme.reco_link_width) * 0.5}
    }

    mod.widgets.RecoMediaPanel = SolidView{
        width: Fill height: Fill flow: Down
        draw_bg.color: theme.reco_panel
        // Scrolls on short windows; an edge fades while more lies past it.
        ScrollShadowView{
            width: Fill height: Fill flow: Down spacing: theme.reco_space_m
            padding: theme.reco_space_l
            fade_left: false
            fade_right: false
            setup_header := RecoPanelHeader{label.text: "Setup"}
            cameras := RecoSection{
                cameras_title := RecoSectionTitle{text: "Cameras"}
                View{
                    width: Fill height: Fit flow: Down
                    CameraRow{
                        left_badge := RecoBadge{label.text: "L"}
                        left_badge_on := RecoBadgeOn{visible: false label.text: "L"}
                        CameraText{
                            RecoBody{text: "Left camera"}
                            left_files := CameraFiles{text: "No video yet"}
                        }
                        add_left := AddButton{}
                        change_left := ChangeButton{}
                    }
                    link_idle := Link{draw_bg.color: theme.reco_hairline}
                    link_on := Link{visible: false draw_bg.color: theme.reco_seam}
                    CameraRow{
                        right_badge := RecoBadge{label.text: "R"}
                        right_badge_on := RecoBadgeOn{visible: false label.text: "R"}
                        CameraText{
                            RecoBody{text: "Right camera"}
                            right_files := CameraFiles{text: "No video yet"}
                        }
                        add_right := AddButton{}
                        change_right := ChangeButton{}
                    }
                }
            }
            calibration := RecoSection{
                calibration_title := RecoSectionTitle{text: "Calibration"}
                calibration_hint := RecoHint{text: "Lines up the two cameras so the join between them disappears."}
                // The status, with its detail indented under the words.
                View{
                    width: Fill height: Fit flow: Down spacing: theme.reco_space_hair
                    View{
                        width: Fill height: Fit flow: Right spacing: theme.reco_space_s align: Align{y: 0.5}
                        cal_dot_idle := RecoDot{}
                        cal_dot_busy := RecoDotBusy{visible: false}
                        cal_dot_ok := RecoDotOk{visible: false}
                        calibration_status := RecoBody{text: "Not calibrated"}
                    }
                    View{
                        width: Fill height: Fit
                        padding: Inset{left: theme.reco_dot + theme.reco_space_s}
                        calibration_detail := RecoHint{text: "Add both cameras first"}
                    }
                }
                View{
                    width: Fill height: Fit flow: Right spacing: theme.reco_space_s
                    margin: Inset{top: theme.reco_space_xs}
                    // A secondary button: the viewer carries the primary
                    // call to action. Disabled until both cameras have
                    // videos; once calibrated it gives way to Recalibrate.
                    auto_calibrate := RecoButton{
                        animator +: {disabled: {default: @on}}
                        text: "Auto-calibrate"
                    }
                    recalibrate := RecoButton{visible: false text: "Recalibrate"}
                    load_calibration := RecoButton{text: "Load file…"}
                }
            }
            recent_button := RecoFlatButton{
                text: "Recent files…"
                draw_icon +: {svg: crate_resource("self:resources/icons/folder.svg")}
            }
        }
    }
}
