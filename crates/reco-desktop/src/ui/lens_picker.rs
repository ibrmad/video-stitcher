//! The lens picker, after the Slint app's: a dialog over the window to
//! search Reco's lens profiles (camera, lens, size) and apply one to both
//! cameras or one, or load a profile file. Escape, Close or a press outside
//! closes it.

use makepad_widgets::*;

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    mod.widgets.RecoLensPicker = Modal{
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
                    RecoStrong{text: "Lens profiles"}
                }
                View{
                    width: Fill height: Fit flow: Down
                    padding: Inset{top: theme.reco_gap bottom: theme.reco_gap}
                    RecoRow{
                        RecoLabelCell{RecoSubdued{text: "Apply to"}}
                        picker_cameras := RecoDropDown{width: Fill labels: ["Both cameras" "Left camera" "Right camera"]}
                    }
                    RecoRow{
                        RecoLabelCell{RecoSubdued{text: "Search"}}
                        picker_search := TextInput{
                            width: Fill height: theme.reco_button margin: 0
                            empty_text: "Camera, lens or size, e.g. hero9 wide"
                        }
                    }
                    View{
                        width: Fill height: Fit
                        padding: Inset{left: theme.reco_pad right: theme.reco_pad top: theme.reco_gap_s bottom: theme.reco_gap_s}
                        picker_hint := RecoSubdued{width: Fill text: ""}
                    }
                    picker_scroll := ScrollYView{
                        width: Fill height: theme.reco_picker_list
                        picker_results := RecoPickList{}
                    }
                }
                RecoSeparator{}
                View{
                    width: Fill height: Fit flow: Right spacing: theme.reco_gap
                    padding: theme.reco_pad
                    align: Align{y: 0.5}
                    picker_file := RecoButton{text: "Load from file…"}
                    View{width: Fill height: Fit}
                    picker_close := RecoButton{text: "Close"}
                }
            }
        }
    }
}
