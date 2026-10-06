//! The Preferences sheet: the app-wide settings only, the recording codec
//! and folder and the usage-data opt-in. (The Slint app's export and
//! recording defaults, seam blend and AI model live where they are used:
//! the export sheet, the view bar and Adjust.) Save keeps them; Cancel,
//! Escape or a press outside leaves everything as it was.

use makepad_widgets::*;

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    // A group's name, with room above it after the first.
    let GroupTitle = RecoRow{margin: Inset{top: theme.reco_gap}}
    let PathInput = TextInput{width: Fill height: theme.reco_button margin: 0}

    mod.widgets.RecoPrefsSheet = Modal{
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
                RoundedView{
                    width: Fill height: theme.reco_row flow: Right align: Align{y: 0.5}
                    padding: Inset{left: theme.reco_pad right: theme.reco_pad}
                    show_bg: true
                    draw_bg +: {color: theme.reco_band border_radius: theme.container_corner_radius}
                    RecoStrong{text: "Preferences"}
                }
                View{
                    width: Fill height: Fit flow: Down
                    padding: Inset{top: theme.reco_gap_s bottom: theme.reco_gap}
                    RecoRow{RecoStrong{text: "Recording"}}
                    RecoRow{
                        Tip{text: "The codec Record uses."
                            RecoLabelCell{RecoSubdued{text: "Codec"}}
                        }
                        prefs_record_codec := RecoDropDown{width: Fill labels: ["H.264"]}
                    }
                    RecoRow{
                        Tip{text: "Where recordings go. Empty: beside the left camera's first video."
                            RecoLabelCell{RecoSubdued{text: "Folder"}}
                        }
                        prefs_folder := PathInput{empty_text: "Beside the left video"}
                        prefs_folder_browse := RecoButton{text: "Choose…"}
                    }
                    GroupTitle{RecoStrong{text: "Privacy"}}
                    RecoRow{
                        Tip{text: "Sends the app's version, your system and figures such as frame rates and export times, under a random id: no names, paths or pictures. Bug reports are sent with it."
                            RecoLabelCell{RecoSubdued{text: "Usage data"}}
                        }
                        prefs_telemetry := RecoCheckBox{text: "Send anonymous usage data"}
                    }
                    prefs_error := View{
                        visible: false
                        width: Fill height: Fit
                        padding: Inset{left: theme.reco_pad right: theme.reco_pad top: theme.reco_gap_s}
                        prefs_error_text := RecoText{width: Fill text: "" draw_text +: {color: theme.reco_error}}
                    }
                }
                RecoSeparator{}
                View{
                    width: Fill height: Fit flow: Right spacing: theme.reco_gap
                    padding: theme.reco_pad
                    align: Align{x: 1.0 y: 0.5}
                    prefs_cancel := RecoButton{text: "Cancel"}
                    prefs_save := RecoPrimaryButton{text: "Save"}
                }
            }
        }
    }
}
