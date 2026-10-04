//! The shell: Media | viewer | Inspector between splitters, then the
//! transport bar and the status bar.

use makepad_widgets::*;

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    mod.widgets.RecoShell = View{
        width: Fill height: Fill flow: Down
        main_split := Splitter{
            axis: SplitterAxis.Horizontal
            align: SplitterAlign.FromA(theme.reco_media_width)
            min_vertical: theme.reco_media_min max_vertical: theme.reco_viewer_min
            min_horizontal: theme.reco_media_min max_horizontal: theme.reco_viewer_min
            a: View{width: Fill height: Fill media_panel := RecoMediaPanel{}}
            b: View{
                width: Fill height: Fill
                inner_split := Splitter{
                    axis: SplitterAxis.Horizontal
                    align: SplitterAlign.FromB(theme.reco_inspector_width)
                    // The B floor includes the 6 pt bar: the Inspector itself
                    // stops at 200 pt.
                    min_vertical: theme.reco_viewer_min max_vertical: theme.reco_inspector_floor
                    min_horizontal: theme.reco_viewer_min max_horizontal: theme.reco_inspector_floor
                    a: View{width: Fill height: Fill viewer := RecoViewer{}}
                    b: View{width: Fill height: Fill inspector := RecoInspector{}}
                }
            }
        }
        transport := RecoTransport{}
        status_bar := RecoStatusBar{}
    }
}
