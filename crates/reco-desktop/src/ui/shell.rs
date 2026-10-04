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
            align: SplitterAlign.FromA(260.0)
            min_vertical: 180.0 max_vertical: 360.0
            min_horizontal: 180.0 max_horizontal: 360.0
            a: View{width: Fill height: Fill media_panel := RecoMediaPanel{}}
            b: View{
                width: Fill height: Fill
                inner_split := Splitter{
                    axis: SplitterAxis.Horizontal
                    align: SplitterAlign.FromB(280.0)
                    min_vertical: 360.0 max_vertical: 200.0
                    min_horizontal: 360.0 max_horizontal: 200.0
                    a: View{width: Fill height: Fill viewer := RecoViewer{}}
                    b: View{width: Fill height: Fill inspector := RecoInspector{}}
                }
            }
        }
        transport := RecoTransport{}
        status_bar := RecoStatusBar{}
    }
}
