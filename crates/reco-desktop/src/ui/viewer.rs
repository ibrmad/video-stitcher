//! The viewer: the preview canvas with the empty state. Module 1 draws the
//! stitched preview into it.

use makepad_widgets::*;

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    let Step = View{
        width: Fit height: Fit flow: Right spacing: 10 align: Align{y: 0.5}
        badge := RoundedView{
            width: 22 height: 22 align: Align{x: 0.5 y: 0.5}
            show_bg: true new_batch: true
            draw_bg +: {color: theme.color_primary_container border_radius: 11.0}
            number := Label{
                text: "1"
                draw_text +: {color: theme.color_on_primary_container text_style: theme.font_bold{font_size: 9.0}}
            }
        }
        label := RecoBody{text: ""}
    }

    mod.widgets.RecoViewer = View{
        width: Fill height: Fill flow: Overlay
        padding: 10
        canvas := RoundedView{
            width: Fill height: Fill flow: Overlay
            align: Align{x: 0.5 y: 0.5}
            show_bg: true new_batch: true
            draw_bg +: {color: theme.reco_viewer border_radius: theme.container_corner_radius}
            sample_frame := RoundedView{
                visible: false
                width: Fill height: Fill
                show_bg: true
                draw_bg +: {
                    border_radius: theme.container_corner_radius
                    pixel: fn() {
                        let sdf = Sdf2d.viewport(self.pos * self.rect_size)
                        sdf.box(0.0, 0.0, self.rect_size.x, self.rect_size.y, self.border_radius)
                        let top = vec4(0.11, 0.33, 0.22, 1.0)
                        let bottom = vec4(0.05, 0.19, 0.12, 1.0)
                        sdf.fill(mix(top, bottom, self.pos.y))
                        return sdf.result
                    }
                }
            }
            empty_state := View{
                width: Fit height: Fit flow: Down spacing: 16
                align: Align{x: 0.5 y: 0.5}
                Label{
                    text: "Stitch two camera videos"
                    draw_text +: {color: theme.color_text text_style: theme.font_bold{font_size: 16.0}}
                }
                View{
                    width: Fit height: Fit flow: Down spacing: 10
                    Step{badge.number.text: "1" label.text: "Add the left and right camera videos"}
                    Step{badge.number.text: "2" label.text: "Auto-calibrate, or load a calibration"}
                    Step{badge.number.text: "3" label.text: "Preview, adjust, then export"}
                }
                View{
                    width: Fit height: Fit flow: Right spacing: 8
                    empty_add := RecoPrimaryButton{text: "Add videos…"}
                    empty_recent := RecoButton{text: "Open recent…"}
                }
                empty_status := RecoMuted{text: ""}
            }
        }
    }
}
