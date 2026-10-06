//! The Setup panel, after Rerun's side panels: a title row, then collapsible
//! sections of 24 pt rows. Cameras: the two cameras as a linked pair.
//! Calibration: its status and actions. Module 0 shows its layout; Module 3
//! wires it.

use makepad_widgets::*;

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    // A camera with no videos offers Add; one with videos offers More,
    // which adds videos after the ones it has.
    let AddButton = RecoButton{
        text: "Add…"
        draw_icon +: {svg: crate_resource("self:resources/icons/plus.svg")}
    }
    let MoreButton = RecoRowIcon{
        visible: false
        draw_icon +: {svg: crate_resource("self:resources/icons/plus.svg")}
    }
    // A camera's files, folded away under its row until asked for.
    let CameraFold = RecoFold{
        animator +: {active: {default: @off}}
        body: View{width: Fill height: Fit flow: Down}
    }
    // A property row's value and switch, as in the Adjust panel.
    // A slider's number, typed into.
    let Value = RecoValueField{}
    let Check = RecoCheckBox{margin: Inset{left: theme.reco_slider_knob}}
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
                recent_menu := RecoMenu{
                    recent_button := RecoRowIcon{draw_icon +: {svg: crate_resource("self:resources/icons/history.svg")}}
                    content +: {recent_menu_list := RecoMenuList{}}
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
                    left_camera := CameraFold{
                        header: RecoRow{
                            left_badge := RecoBadge{label.text: "L"}
                            left_badge_on := RecoBadgeOn{visible: false label.text: "L"}
                            RecoText{text: "Left camera"}
                            left_files := CameraFiles{text: "No video yet"}
                            add_left := AddButton{}
                            Tip{text: "Add more videos to the left camera"
                                more_left := MoreButton{}
                            }
                            Tip{text: "Show the left camera's files"
                                left_fold := View{
                                    visible: false width: Fit height: Fit
                                    margin: Inset{right: (theme.reco_chevron - theme.reco_chevron_ink) * -0.5}
                                    fold_button := RecoChevron{animator +: {active: {default: @off}}}
                                }
                            }
                        }
                        body: View{
                            width: Fill height: Fit flow: Down
                            left_list := RecoFileList{}
                            View{
                                width: Fill height: Fit
                                // The button's label starts under the file names.
                                padding: Inset{left: theme.reco_file_indent - theme.reco_button_pad_x right: theme.reco_pad bottom: theme.reco_gap_s}
                                clear_left := RecoFlatButton{text: "Remove all"}
                            }
                        }
                    }
                    link_idle := Link{draw_bg.color: theme.reco_stroke}
                    link_on := Link{visible: false draw_bg.color: theme.reco_seam}
                    right_camera := CameraFold{
                        header: RecoRow{
                            right_badge := RecoBadge{label.text: "R"}
                            right_badge_on := RecoBadgeOn{visible: false label.text: "R"}
                            RecoText{text: "Right camera"}
                            right_files := CameraFiles{text: "No video yet"}
                            add_right := AddButton{}
                            Tip{text: "Add more videos to the right camera"
                                more_right := MoreButton{}
                            }
                            Tip{text: "Show the right camera's files"
                                right_fold := View{
                                    visible: false width: Fit height: Fit
                                    margin: Inset{right: (theme.reco_chevron - theme.reco_chevron_ink) * -0.5}
                                    fold_button := RecoChevron{animator +: {active: {default: @off}}}
                                }
                            }
                        }
                        body: View{
                            width: Fill height: Fit flow: Down
                            right_list := RecoFileList{}
                            View{
                                width: Fill height: Fit
                                // The button's label starts under the file names.
                                padding: Inset{left: theme.reco_file_indent - theme.reco_button_pad_x right: theme.reco_pad bottom: theme.reco_gap_s}
                                clear_right := RecoFlatButton{text: "Remove all"}
                            }
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
                    // With a calibration file it names the file, with a
                    // button to remove it.
                    View{
                        width: Fill height: Fit flow: Right spacing: theme.reco_gap align: Align{y: 0.5}
                        padding: Inset{left: theme.reco_pad + theme.reco_dot + theme.reco_gap right: theme.reco_pad bottom: theme.reco_gap_s}
                        calibration_detail := RecoSubdued{width: Fill text: "Add both cameras first"}
                        Tip{text: "Remove the calibration"
                            clear_calibration := RecoRowIcon{
                                visible: false
                                draw_icon +: {svg: crate_resource("self:resources/icons/close.svg")}
                            }
                        }
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
                        cancel_calibration := RecoButton{visible: false text: "Cancel"}
                        load_calibration := RecoButton{text: "Load file…"}
                    }
                    // How auto-calibrate works; locked while it runs.
                    calibration_advanced := RecoAdvanced{
                        body +: {
                            RecoRow{
                                Tip{text: "Frame pairs to match: more is slower and steadier."
                                    RecoLabelCell{RecoSubdued{text: "Frames"}}
                                }
                                cal_frames := RecoDropDown{
                                    width: theme.reco_quality_width labels: ["2" "4" "6" "8"]
                                }
                            }
                            RecoRow{
                                Tip{text: "Start from the cameras' motion sensors (GoPro)."
                                    RecoLabelCell{RecoSubdued{text: "IMU seeds"}}
                                }
                                cal_imu := Check{text: ""}
                            }
                            RecoRow{
                                Tip{text: "How strong a feature must be to count: lower finds more."
                                    RecoLabelCell{RecoSubdued{text: "AKAZE threshold"}}
                                }
                                cal_akaze := RecoSlider{min: 0.00005 max: 0.005 default: 0.0001}
                                cal_akaze_value := Value{text: "0.0001"}
                            }
                            RecoRow{
                                Tip{text: "Ignore features above this line (a share of the height)."
                                    RecoLabelCell{RecoSubdued{text: "Detect from"}}
                                }
                                cal_y_min := RecoSlider{min: 0.0 max: 0.5 default: 0.05}
                                cal_y_min_value := Value{text: "0.05"}
                            }
                            RecoRow{
                                Tip{text: "Ignore features below this line (a share of the height)."
                                    RecoLabelCell{RecoSubdued{text: "Detect to"}}
                                }
                                cal_y_max := RecoSlider{min: 0.5 max: 1.0 default: 0.95}
                                cal_y_max_value := Value{text: "0.95"}
                            }
                            RecoRow{
                                Tip{text: "Seconds to leave out at the end of the videos."
                                    RecoLabelCell{RecoSubdued{text: "Skip end"}}
                                }
                                cal_skip_end := RecoSlider{min: 0.0 max: 60.0 default: 0.0}
                                cal_skip_end_value := Value{text: "0 s"}
                            }
                        }
                    }
                    // The pitch's outline in each camera, for AI tracking:
                    // drawn in the browser editor, pasted back here.
                    field_outline := RecoFold{
                        animator +: {active: {default: @off}}
                        header: View{
                            width: Fill height: theme.reco_row flow: Right spacing: theme.reco_gap_s align: Align{y: 0.5}
                            padding: Inset{left: theme.reco_pad right: theme.reco_pad}
                            fold_button := RecoChevron{animator +: {active: {default: @off}}}
                            RecoSubdued{text: "Field outline"}
                            roi_status := RecoMeta{text: "None"}
                        }
                        body: View{
                            width: Fill height: Fit flow: Down
                            RecoRow{
                                Tip{text: "Opens the current frames in your browser: draw the pitch's outline there and copy it."
                                    roi_edit := RecoButton{text: "Edit in browser…"}
                                }
                            }
                            RecoRow{
                                roi_json := TextInput{
                                    width: Fill height: theme.reco_button margin: 0
                                    empty_text: "Paste the outline here"
                                }
                                roi_use := RecoButton{text: "Use"}
                            }
                            RecoRow{
                                View{width: Fill height: Fit}
                                roi_clear := RecoFlatButton{visible: false text: "Remove outline"}
                            }
                        }
                    }
                }
            }
        }
    }
}
