//! The Setup panel, after Rerun's side panels: a title row, then collapsible
//! sections of 24 pt rows. Cameras: the two cameras as a linked pair.
//! Calibration: its status and actions. Module 0 shows its layout; Module 3
//! wires it.

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
    let ChangeButton = RecoRowIcon{
        visible: false
        draw_icon +: {svg: crate_resource("self:resources/icons/folder.svg")}
    }
    // The camera's file count and length, right-aligned into a column.
    let CameraFiles = RecoMeta{align: Align{x: 1.0}}
    // The join between the two cameras, centred under the badges: lit once
    // both have video.
    let Link = SolidView{
        width: theme.reco_link_width height: theme.reco_link_height
        margin: Inset{left: theme.reco_pad + (theme.reco_badge - theme.reco_link_width) * 0.5}
    }

    mod.widgets.RecoMediaPanel = SolidView{
        width: Fill height: Fill flow: Down
        draw_bg.color: theme.reco_panel
        RecoTitleRow{
            setup_header := RecoStrong{text: "Setup"}
            View{width: Fill height: Fit}
            Tip{text: "Recent files"
                recent_menu := MenuButton{
                    place: Below
                    button := RecoRowIcon{draw_icon +: {svg: crate_resource("self:resources/icons/history.svg")}}
                }
            }
        }
        // Scrolls on short windows; an edge fades while more lies past it.
        ScrollShadowView{
            width: Fill height: Fill flow: Down
            fade_left: false
            fade_right: false
            cameras := RecoSection{
                header +: {
                    title +: {text: "Cameras"}
                    help +: {text: "Add all the GoPro files from each camera. Reco plays a camera's files in order as one video."}
                }
                body +: {
                    left_row := RecoRow{
                        left_badge := RecoBadge{label.text: "L"}
                        left_badge_on := RecoBadgeOn{visible: false label.text: "L"}
                        RecoText{text: "Left camera"}
                        left_files := CameraFiles{text: "No video yet"}
                        add_left := AddButton{}
                        Tip{text: "Change the left camera's videos"
                            change_left := ChangeButton{}
                        }
                    }
                    link_idle := Link{draw_bg.color: theme.reco_stroke}
                    link_on := Link{visible: false draw_bg.color: theme.reco_seam}
                    right_row := RecoRow{
                        right_badge := RecoBadge{label.text: "R"}
                        right_badge_on := RecoBadgeOn{visible: false label.text: "R"}
                        RecoText{text: "Right camera"}
                        right_files := CameraFiles{text: "No video yet"}
                        add_right := AddButton{}
                        Tip{text: "Change the right camera's videos"
                            change_right := ChangeButton{}
                        }
                    }
                }
            }
            calibration := RecoSection{
                header +: {
                    title +: {text: "Calibration"}
                    help +: {text: "Lines up the two cameras so the join between them disappears."}
                }
                body +: {
                    status_row := RecoRow{
                        spacing: theme.reco_gap
                        cal_dot_idle := RecoDot{}
                        cal_dot_busy := RecoDotBusy{visible: false}
                        cal_dot_ok := RecoDotOk{visible: false}
                        cal_dot_error := RecoDotError{visible: false}
                        calibration_status := RecoText{text: "Not calibrated"}
                    }
                    // The detail sits under the status words, after the dot.
                    View{
                        width: Fill height: Fit
                        padding: Inset{left: theme.reco_pad + theme.reco_dot + theme.reco_gap right: theme.reco_pad bottom: theme.reco_gap_s}
                        calibration_detail := RecoSubdued{width: Fill text: "Add both cameras first"}
                    }
                    RecoRow{
                        spacing: theme.reco_gap_s
                        // Disabled until both cameras have videos; once
                        // calibrated it gives way to Recalibrate.
                        auto_calibrate := RecoButton{
                            animator +: {disabled: {default: @on}}
                            text: "Auto-calibrate"
                        }
                        recalibrate := RecoButton{visible: false text: "Recalibrate"}
                        load_calibration := RecoButton{text: "Load file…"}
                    }
                }
            }
        }
    }
}
