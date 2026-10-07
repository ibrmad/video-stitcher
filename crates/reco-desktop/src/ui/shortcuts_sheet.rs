//! The Keyboard shortcuts sheet: every key the app answers (from the table the
//! key handler is tested against), and the project's Website and Forum.
//! Escape, Close or a press outside closes it.

use makepad_widgets::*;

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    mod.widgets.RecoShortcutsSheet = Modal{
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
                    RecoStrong{text: "Keyboard shortcuts"}
                }
                View{
                    width: Fill height: Fit flow: Down
                    padding: Inset{top: theme.reco_gap bottom: theme.reco_gap}
                    shortcuts_table := RecoKeyTable{}
                }
                RecoSeparator{}
                View{
                    width: Fill height: Fit flow: Right spacing: theme.reco_gap
                    padding: theme.reco_pad
                    align: Align{y: 0.5}
                    shortcuts_website := RecoButton{text: "Website"}
                    shortcuts_forum := RecoButton{text: "Forum"}
                    View{width: Fill height: Fit}
                    shortcuts_close := RecoButton{text: "Close"}
                }
            }
        }
    }
}
