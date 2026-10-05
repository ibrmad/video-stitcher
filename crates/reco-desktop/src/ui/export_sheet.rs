//! The export sheet, after the Slint app's export dialog: a dialog over the
//! window with the file to write, its size, codec and quality, the part of
//! the match to export, the replay and event extras, and AI tracking (its
//! rows show while it is on, the panner's finer knobs in a closed Advanced
//! tier). The rows scroll in a short window. Escape, Cancel or a press
//! outside closes it.

use makepad_widgets::*;

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    let TimeInput = TextInput{width: theme.reco_time_input height: theme.reco_button margin: 0 empty_text: "0:00"}
    let Value = RecoText{width: theme.reco_value_width align: Align{x: 1.0}}
    // A line of text in the control column that wraps when long.
    let NoteRow = RecoRow{height: Fit{min: theme.reco_row}}

    mod.widgets.RecoExportSheet = Modal{
        content +: {
            width: theme.reco_sheet_width height: Fit
            RoundedView{
                width: Fill height: Fit flow: Down
                show_bg: true new_batch: true
                draw_bg +: {
                    color: theme.reco_panel
                    border_radius: theme.container_corner_radius
                    border_size: theme.reco_separator_width
                    border_color: theme.reco_hover
                }
                // Rerun's band: the sheet's name on the panel colour's step up.
                RoundedView{
                    width: Fill height: theme.reco_row flow: Right align: Align{y: 0.5}
                    padding: Inset{left: theme.reco_pad right: theme.reco_pad}
                    show_bg: true
                    draw_bg +: {color: theme.reco_band border_radius: theme.container_corner_radius}
                    RecoStrong{text: "Export the stitched match"}
                }
                export_rows := ScrollYView{
                    width: Fill height: Fit{max: theme.reco_sheet_rows_max} flow: Down
                    padding: Inset{top: theme.reco_gap bottom: theme.reco_gap}
                    RecoRow{
                        RecoLabelCell{RecoSubdued{text: "File"}}
                        export_output := TextInput{
                            width: Fill height: theme.reco_button margin: 0
                            empty_text: "Pick a file, or type a path"
                        }
                        export_browse := RecoButton{text: "Save to…"}
                    }
                    RecoRow{
                        RecoLabelCell{RecoSubdued{text: "Size"}}
                        export_size := RecoDropDown{width: Fill labels: ["1080p"]}
                    }
                    RecoRow{
                        RecoLabelCell{RecoSubdued{text: "Codec"}}
                        export_codec := RecoDropDown{width: Fill labels: ["H.264"]}
                    }
                    RecoRow{
                        RecoLabelCell{RecoSubdued{text: "Quality"}}
                        export_quality := RecoDropDown{width: Fill labels: ["Fast" "Balanced" "High"]}
                    }
                    RecoRow{
                        Tip{text: "Where the export starts on the timeline."
                            RecoLabelCell{RecoSubdued{text: "Start"}}
                        }
                        range_start := RecoSlider{min: 0.0 max: 1.0 default: 0.0}
                        range_start_text := TimeInput{}
                    }
                    RecoRow{
                        Tip{text: "Where the export ends on the timeline."
                            RecoLabelCell{RecoSubdued{text: "End"}}
                        }
                        range_end := RecoSlider{min: 0.0 max: 1.0 default: 1.0}
                        range_end_text := TimeInput{}
                    }
                    RecoRow{
                        RecoLabelCell{}
                        range_length := RecoSubdued{text: ""}
                        range_empty := RecoText{
                            visible: false
                            text: "Nothing to export: the end is at the start."
                            draw_text +: {color: theme.reco_error}
                        }
                    }
                    RecoRow{
                        Tip{text: "Also writes both cameras' pictures, stacked, so the match can be stitched again later."
                            RecoLabelCell{RecoSubdued{text: "Replay"}}
                        }
                        export_replay := RecoCheckBox{text: "Record the raw input"}
                    }
                    RecoRow{
                        Tip{text: "Also writes what the pipeline decided, frame by frame, for visualize_detections.py."
                            RecoLabelCell{RecoSubdued{text: "Events"}}
                        }
                        export_events := RecoCheckBox{text: "Save the debug events (.jsonl)"}
                    }
                    RecoRow{
                        Tip{text: "Aims the exported picture at the play: a detector finds the players and the ball, and the camera pans and zooms to follow them."
                            RecoLabelCell{RecoSubdued{text: "AI tracking"}}
                        }
                        ai_enable := RecoCheckBox{
                            text: "Follow the play"
                            animator +: {disabled: {default: @on}}
                        }
                    }
                    NoteRow{
                        RecoLabelCell{}
                        ai_status := RecoSubdued{width: Fill text: "Checking whether this machine can run it…"}
                        ai_status_error := RecoText{visible: false width: Fill text: "" draw_text +: {color: theme.reco_error}}
                    }
                    ai_rows := View{
                        visible: false
                        width: Fill height: Fit flow: Down
                        RecoRow{
                            Tip{text: "The detector: an .onnx file. Choosing one here also makes it the default in Preferences."
                                RecoLabelCell{RecoSubdued{text: "Model"}}
                            }
                            ai_model := TextInput{
                                width: Fill height: theme.reco_button margin: 0
                                empty_text: "None chosen"
                            }
                            ai_model_browse := RecoButton{text: "Choose…"}
                        }
                        ai_model_problem := NoteRow{
                            visible: false
                            RecoLabelCell{}
                            ai_model_problem_text := RecoText{width: Fill text: "" draw_text +: {color: theme.reco_warning}}
                        }
                        RecoRow{
                            Tip{text: "What the camera follows. Ball only suits a model that finds just the ball; Sweep pans slowly from side to side without AI, to try an export."
                                RecoLabelCell{RecoSubdued{text: "Follow"}}
                            }
                            ai_mode := RecoDropDown{width: Fill labels: ["Players and ball" "Ball only" "Sweep (no AI)"]}
                        }
                        RecoRow{
                            Tip{text: "How often the detector looks. Less often exports faster; the camera still moves smoothly in between."
                                RecoLabelCell{RecoSubdued{text: "Detection"}}
                            }
                            ai_interval := RecoDropDown{
                                width: Fill
                                labels: ["Every frame" "Every 3 frames" "Every 5 frames" "Every 10 frames" "Every 15 frames" "Every 30 frames"]
                            }
                        }
                        RecoRow{
                            Tip{text: "How the camera moves: Broadcast is calm, Action is tighter and quicker, Frame all keeps every player in the picture. Choosing one sets the knobs below."
                                RecoLabelCell{RecoSubdued{text: "Style"}}
                            }
                            ai_preset := RecoDropDown{width: Fill labels: ["Broadcast" "Action" "Frame all"]}
                        }
                        RecoRow{
                            Tip{text: "Follow the action aims at the busiest group of players; Keep everyone in frame shows the whole team."
                                RecoLabelCell{RecoSubdued{text: "Framing"}}
                            }
                            ai_framing := RecoDropDown{width: Fill labels: ["Follow the action" "Keep everyone in frame"]}
                        }
                        RecoRow{
                            Tip{text: "Keeps the camera level: it only pans from side to side."
                                RecoLabelCell{RecoSubdued{text: "Tilt"}}
                            }
                            ai_lock_pitch := RecoCheckBox{text: "Hold it level (pan only)"}
                        }
                        RecoRow{
                            Tip{text: "Seconds of video the camera sees ahead, so it can move early and smoothly. The frames wait in the GPU's memory."
                                RecoLabelCell{RecoSubdued{text: "Lookahead"}}
                            }
                            View{
                                width: Fill height: theme.reco_row flow: Overlay
                                ai_lookahead_zones := RecoZones{}
                                ai_lookahead := RecoKnobSlider{min: 0.0 max: 2.5 default: 2.5}
                            }
                            ai_lookahead_value := Value{text: "2.5 s"}
                        }
                        ai_lookahead_notes := NoteRow{
                            visible: false
                            RecoLabelCell{}
                            ai_lookahead_note := RecoSubdued{width: Fill text: ""}
                            ai_lookahead_warning := RecoText{visible: false width: Fill text: "" draw_text +: {color: theme.reco_warning}}
                        }
                        ai_advanced := RecoAdvanced{
                            body +: {
                                RecoRow{
                                    Tip{text: "How the busiest group is found: Densest group centres on where players are closest together; Trimmed mean drops the players farthest from the average."
                                        RecoLabelCell{RecoSubdued{text: "Group"}}
                                    }
                                    ai_cluster_mode := RecoDropDown{width: Fill labels: ["Densest group" "Trimmed mean"]}
                                }
                                RecoRow{
                                    Tip{text: "How much the ball pulls the camera, against the players."
                                        RecoLabelCell{RecoSubdued{text: "Ball weight"}}
                                    }
                                    ai_ball_weight := RecoSlider{min: 0.0 max: 1.0 default: 0.6}
                                    ai_ball_weight_value := Value{text: "0.60"}
                                }
                                RecoRow{
                                    Tip{text: "How close players must be to count as one group, in radians."
                                        RecoLabelCell{RecoSubdued{text: "Group size"}}
                                    }
                                    ai_bandwidth := RecoSlider{min: 0.1 max: 0.6 default: 0.2}
                                    ai_bandwidth_value := Value{text: "0.20"}
                                }
                                RecoRow{
                                    Tip{text: "How far the play moves before the camera follows, in radians."
                                        RecoLabelCell{RecoSubdued{text: "Dead zone"}}
                                    }
                                    ai_dead_zone := RecoSlider{min: 0.0 max: 0.5 default: 0.03}
                                    ai_dead_zone_value := Value{text: "0.03"}
                                }
                                RecoRow{
                                    Tip{text: "The narrowest view the camera zooms in to."
                                        RecoLabelCell{RecoSubdued{text: "Tightest view"}}
                                    }
                                    ai_fov_tight := RecoSlider{min: 10.0 max: 40.0 default: 22.0}
                                    ai_fov_tight_value := Value{text: "22°"}
                                }
                                RecoRow{
                                    Tip{text: "The view the camera holds when nothing pulls it in or out."
                                        RecoLabelCell{RecoSubdued{text: "Usual view"}}
                                    }
                                    ai_fov_default := RecoSlider{min: 20.0 max: 60.0 default: 40.0}
                                    ai_fov_default_value := Value{text: "40°"}
                                }
                                RecoRow{
                                    Tip{text: "The widest view the camera zooms out to."
                                        RecoLabelCell{RecoSubdued{text: "Widest view"}}
                                    }
                                    ai_fov_wide := RecoSlider{min: 40.0 max: 90.0 default: 58.0}
                                    ai_fov_wide_value := Value{text: "58°"}
                                }
                            }
                        }
                    }
                    export_error := View{
                        visible: false
                        width: Fill height: Fit
                        padding: Inset{left: theme.reco_pad right: theme.reco_pad top: theme.reco_gap_s}
                        export_error_text := RecoText{width: Fill text: "" draw_text +: {color: theme.reco_error}}
                    }
                }
                RecoSeparator{}
                View{
                    width: Fill height: Fit flow: Right spacing: theme.reco_gap
                    padding: theme.reco_pad
                    align: Align{x: 1.0 y: 0.5}
                    sheet_cancel := RecoButton{text: "Cancel"}
                    sheet_export := RecoPrimaryButton{text: "Export"}
                }
            }
        }
    }
}
