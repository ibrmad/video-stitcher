//! Asks before quitting with unsaved calibration edits: Save saves and then
//! quits, Don't Save quits as the file was, Cancel (or Escape, or a press
//! outside) stays.

use makepad_widgets::*;

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    mod.widgets.RecoUnsavedSheet = Modal{
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
                    RecoStrong{text: "Save the calibration?"}
                }
                View{
                    width: Fill height: Fit
                    padding: theme.reco_pad
                    unsaved_body := RecoText{text: "The calibration has changes that aren't saved."}
                }
                RecoSeparator{}
                View{
                    width: Fill height: Fit flow: Right spacing: theme.reco_gap
                    padding: theme.reco_pad
                    align: Align{y: 0.5}
                    unsaved_discard := RecoButton{text: "Don't Save"}
                    View{width: Fill height: Fit}
                    unsaved_cancel := RecoButton{text: "Cancel"}
                    unsaved_save := RecoPrimaryButton{text: "Save"}
                }
            }
        }
    }
}
