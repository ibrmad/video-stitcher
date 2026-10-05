//! The export sheet, after the Slint app's export dialog: a dialog over the
//! window with the file to write, its size, codec and quality, the part of
//! the match to export, and the replay and event extras. AI tracking joins
//! in Module 6b. Escape, Cancel or a press outside closes it.

use makepad_widgets::*;

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    let TimeInput = TextInput{width: theme.reco_time_input height: theme.reco_button margin: 0 empty_text: "0:00"}

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
                View{
                    width: Fill height: Fit flow: Down
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
