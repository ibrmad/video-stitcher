//! The status bar: status line, version and Report bug.

use makepad_widgets::*;

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    mod.widgets.RecoStatusBar = SolidView{
        width: Fill height: theme.reco_statusbar_height flow: Right spacing: 12
        align: Align{y: 0.5}
        padding: Inset{left: 12 right: 8}
        draw_bg.color: theme.color_bg_app
        status_text := RecoMuted{width: Fill text: "Ready"}
        version_text := RecoMuted{text: ""}
        report_bug := ButtonFlatter{
            text: "Report bug"
            draw_text +: {color: theme.reco_text_secondary}
        }
    }
}
