//! The shell: Setup | viewer | Adjust between splitters, then the time panel
//! across the full width, as in Rerun. The side panels keep their own draw
//! lists, so a playing preview doesn't redraw them.

use makepad_widgets::*;

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    // A splitter drawn as Rerun draws its panel edges: a one-point line
    // that brightens and thickens under the pointer.
    let RecoSplitter = Splitter{
        draw_bg +: {
            color_bg: uniform(theme.reco_panel)
            line_color: uniform(theme.reco_separator)
            line_hover: uniform(theme.reco_stroke)
            line_width: uniform(theme.reco_separator_width)
            pixel: fn() {
                let sdf = Sdf2d.viewport(self.pos * self.rect_size)
                sdf.clear(self.color_bg)
                let active = max(self.hover, self.drag)
                let w = self.line_width * (1.0 + active)
                if self.is_vertical > 0.5 {
                    sdf.rect(self.rect_size.x * 0.5 - w * 0.5, 0.0, w, self.rect_size.y)
                } else {
                    sdf.rect(0.0, self.rect_size.y * 0.5 - w * 0.5, self.rect_size.x, w)
                }
                sdf.fill(mix(self.line_color, self.line_hover, active))
                return sdf.result
            }
        }
    }

    mod.widgets.RecoShell = View{
        width: Fill height: Fill flow: Down
        main_split := RecoSplitter{
            axis: SplitterAxis.Horizontal
            align: SplitterAlign.FromA(theme.reco_media_width)
            // A slide relaxes these floors and panel_motion.rs puts them
            // back: change both together.
            min_vertical: theme.reco_media_min max_vertical: theme.reco_viewer_min
            min_horizontal: theme.reco_media_min max_horizontal: theme.reco_viewer_min
            // Each side panel sits in a box that holds its width while it
            // slides (panel_motion.rs).
            a: View{
                width: Fill height: Fill new_batch: true
                setup_box := RecoPanelBox{anchor_end: true media_panel := RecoMediaPanel{}}
            }
            b: View{
                width: Fill height: Fill
                inner_split := RecoSplitter{
                    axis: SplitterAxis.Horizontal
                    align: SplitterAlign.FromB(theme.reco_inspector_width)
                    // The B floor includes the 6 pt bar: the Adjust panel
                    // itself stops at 200 pt.
                    min_vertical: theme.reco_viewer_min max_vertical: theme.reco_inspector_floor
                    min_horizontal: theme.reco_viewer_min max_horizontal: theme.reco_inspector_floor
                    a: View{width: Fill height: Fill viewer := RecoViewer{}}
                    b: View{
                        width: Fill height: Fill new_batch: true
                        adjust_box := RecoPanelBox{inspector := RecoInspector{}}
                    }
                }
            }
        }
        time_panel := RecoTimePanel{}
    }
}
