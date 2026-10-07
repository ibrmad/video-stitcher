//! The Report a bug sheet: what went wrong, a way to reach the person, and
//! whether to include the system's details and the log. Send goes to Reco's
//! developers with usage data on; Copy report puts the report on the clipboard
//! to post on the forum. Escape, Cancel or a press outside closes it.

use makepad_widgets::*;

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    mod.widgets.RecoBugSheet = Modal{
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
                    RecoStrong{text: "Report a bug"}
                }
                View{
                    width: Fill height: Fit flow: Down spacing: theme.reco_gap_s
                    padding: Inset{top: theme.reco_gap bottom: theme.reco_gap}
                    View{
                        width: Fill height: Fit flow: Right spacing: theme.reco_gap
                        padding: Inset{left: theme.reco_pad right: theme.reco_pad}
                        RecoLabelCell{
                            padding: Inset{top: theme.reco_gap_s}
                            RecoSubdued{text: "What went wrong"}
                        }
                        bug_message := TextInput{
                            width: Fill height: theme.reco_text_area margin: 0
                            is_multiline: true
                            empty_text: "What you did, what happened, and what you expected"
                        }
                    }
                    RecoRow{
                        Tip{text: "How the developers can reach you, if you'd like an answer."
                            RecoLabelCell{RecoSubdued{text: "Contact"}}
                        }
                        bug_contact := TextInput{
                            width: Fill height: theme.reco_button margin: 0
                            empty_text: "Forum name or email (optional)"
                        }
                    }
                    RecoRow{
                        Tip{text: "Adds the app's version, your system and GPU, the open files' names, the preview's figures and the app's recent log. Your home folder shows as ~."
                            RecoLabelCell{RecoSubdued{text: "Details"}}
                        }
                        bug_details := RecoCheckBox{
                            text: "Include system info and logs"
                            animator +: {active: {default: @on}}
                        }
                    }
                    bug_hint := View{
                        visible: false
                        width: Fill height: Fit flow: Right spacing: theme.reco_gap
                        padding: Inset{left: theme.reco_pad right: theme.reco_pad}
                        align: Align{y: 0.5}
                        RecoSubdued{width: Fill text: "Sending needs usage data on. Or copy it for the forum."}
                        bug_prefs := RecoButton{text: "Preferences…"}
                    }
                }
                RecoSeparator{}
                View{
                    width: Fill height: Fit flow: Right spacing: theme.reco_gap
                    padding: theme.reco_pad
                    align: Align{y: 0.5}
                    bug_copy := RecoButton{text: "Copy report"}
                    View{width: Fill height: Fit}
                    bug_cancel := RecoButton{text: "Cancel"}
                    bug_send := RecoPrimaryButton{text: "Send" animator +: {disabled: {default: @on}}}
                }
            }
        }
    }
}
