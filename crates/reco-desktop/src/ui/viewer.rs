//! The viewer. Until a stitched preview exists it shows the next step of the
//! job under the panorama frame (add cameras, then calibrate), with a stepper
//! that says where the user is; calibration shows its progress in that
//! column. An export shows a progress card over the picture. Both have
//! Cancel. Module 1 draws the stitched video where the placeholder pitch is.

use makepad_widgets::*;

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    let DoneBadge = RoundedView{
        width: theme.reco_badge height: theme.reco_badge
        align: Align{x: 0.5 y: 0.5}
        show_bg: true
        draw_bg +: {color: theme.color_primary_container border_radius: theme.reco_badge_radius}
        Icon{
            icon_walk: Walk{width: theme.reco_icon_small height: theme.reco_icon_small}
            draw_icon +: {svg: crate_resource("self:resources/icons/check.svg") color: theme.color_on_primary_container}
        }
    }
    let StepRow = View{
        width: Fit height: Fit flow: Right spacing: theme.reco_space_s align: Align{y: 0.5}
    }

    mod.widgets.RecoViewer = View{
        width: Fill height: Fill flow: Overlay
        padding: theme.reco_space_l
        canvas := RoundedView{
            width: Fill height: Fill flow: Overlay
            align: Align{x: 0.5 y: 0.5}
            show_bg: true new_batch: true
            draw_bg +: {color: theme.reco_viewer border_radius: theme.container_corner_radius}

            // Placeholder pitch (a letterboxed panorama) for --look-preview,
            // until Module 1 draws the stitched video here.
            sample_frame := View{
                visible: false
                width: Fill height: Fill
                show_bg: true
                draw_bg +: {
                    grass_a: uniform(theme.reco_grass_a)
                    grass_b: uniform(theme.reco_grass_b)
                    line_color: uniform(theme.reco_pitch_line)
                    aspect: uniform(theme.reco_panorama_aspect)
                    pixel: fn() {
                        let size = self.rect_size
                        let w = min(size.x, size.y * self.aspect)
                        let h = w / self.aspect
                        let o = (size - vec2(w, h)) * 0.5
                        let p = self.pos * size
                        let q = p - o
                        let sdf = Sdf2d.viewport(p)
                        let stripe = step(0.5, fract(q.x / w * 7.))
                        sdf.box(o.x, o.y, w, h, 2.)
                        sdf.fill(mix(self.grass_a, self.grass_b, stripe))
                        let m = h * 0.08
                        let lw = max(1., h * 0.006)
                        let lc = vec4(self.line_color.rgb, 0.7)
                        sdf.rect(o.x + m, o.y + m, w - m * 2., h - m * 2.)
                        sdf.stroke(lc, lw)
                        sdf.move_to(o.x + w * 0.5, o.y + m)
                        sdf.line_to(o.x + w * 0.5, o.y + h - m)
                        sdf.stroke(lc, lw)
                        sdf.circle(o.x + w * 0.5, o.y + h * 0.5, h * 0.17)
                        sdf.stroke(lc, lw)
                        let bw = w * 0.11
                        let bh = h * 0.56
                        sdf.rect(o.x + m, o.y + (h - bh) * 0.5, bw, bh)
                        sdf.stroke(lc, lw)
                        sdf.rect(o.x + w - m - bw, o.y + (h - bh) * 0.5, bw, bh)
                        sdf.stroke(lc, lw)
                        return sdf.result
                    }
                }
            }

            // The next step of the job. Text wraps and centres inside the
            // viewer; long tasks show their progress in the same column.
            empty_state := View{
                width: Fill height: Fit flow: Down spacing: theme.reco_space_xl
                align: Align{x: 0.5 y: 0.5}
                padding: Inset{left: theme.reco_space_xxl right: theme.reco_space_xxl}
                panorama := RecoPanorama{}
                View{
                    width: Fill{max: theme.reco_panorama_max_width} height: Fit flow: Down spacing: theme.reco_space_s
                    margin: Inset{top: theme.reco_space_s}
                    next_title := RecoDisplay{
                        width: Fill align: Align{x: 0.5}
                        text: "Add your two camera videos"
                    }
                    next_body := RecoSecondary{
                        width: Fill align: Align{x: 0.5}
                        text: "Start with the left camera. All the GoPro files from one camera go in together."
                    }
                }
                next_actions := View{
                    width: Fit height: Fit flow: Right spacing: theme.reco_space_m
                    next_primary := RecoPrimaryButton{text: "Add left camera…"}
                    next_secondary := RecoButton{text: "Recent files…"}
                }
                calibrate_progress := View{
                    visible: false
                    width: Fill{max: theme.reco_progress_width} height: Fit flow: Down spacing: theme.reco_space_s
                    calibrate_bar := RecoProgressBar{}
                    View{
                        width: Fill height: Fit flow: Right spacing: theme.reco_space_m align: Align{y: 0.5}
                        calibrate_eta := RecoMuted{width: Fill text: ""}
                        calibrate_cancel := RecoButton{text: "Cancel"}
                    }
                }
                stepper := View{
                    width: Fit height: Fit flow: Right spacing: theme.reco_space_xxl
                    margin: Inset{top: theme.reco_space_m}
                    StepRow{
                        step1_todo := RecoBadge{visible: false label.text: "1"}
                        step1_current := RecoBadgeCurrent{label.text: "1"}
                        step1_done := DoneBadge{visible: false}
                        RecoSecondary{text: "Add cameras"}
                    }
                    StepRow{
                        step2_todo := RecoBadge{label.text: "2"}
                        step2_current := RecoBadgeCurrent{visible: false label.text: "2"}
                        step2_done := DoneBadge{visible: false}
                        RecoSecondary{text: "Calibrate"}
                    }
                    StepRow{
                        step3_todo := RecoBadge{label.text: "3"}
                        step3_current := RecoBadgeCurrent{visible: false label.text: "3"}
                        step3_done := DoneBadge{visible: false}
                        RecoSecondary{text: "Export"}
                    }
                }
            }

            // An export: what is being written, how far, how long, and
            // Cancel, floating over the bottom of the picture.
            export_card := View{
                visible: false
                width: Fill height: Fill
                align: Align{x: 0.5 y: 1.0}
                padding: Inset{left: theme.reco_space_xl right: theme.reco_space_xl bottom: theme.reco_space_xxxl}
                RoundedView{
                    width: Fill{max: theme.reco_progress_width} height: Fit flow: Down spacing: theme.reco_space_s
                    padding: theme.reco_space_xxl
                    show_bg: true new_batch: true
                    draw_bg +: {
                        color: theme.reco_surface
                        border_radius: theme.container_corner_radius
                        border_size: theme.reco_hairline_width
                        border_color: theme.reco_hairline
                    }
                    export_title := RecoSectionTitle{text: ""}
                    export_detail := RecoSecondary{text: ""}
                    export_bar := RecoProgressBar{margin: Inset{top: theme.reco_space_xs}}
                    View{
                        width: Fill height: Fit flow: Right spacing: theme.reco_space_m align: Align{y: 0.5}
                        export_eta := RecoMuted{width: Fill text: ""}
                        export_cancel := RecoButton{text: "Cancel"}
                    }
                }
            }

            // Shown when the window got too narrow for the Adjust panel.
            fold_hint := View{
                visible: false
                width: Fill height: Fill
                align: Align{x: 1.0 y: 0.0}
                padding: theme.reco_space_m
                fold_hint_button := RecoFlatButton{text: "Adjust panel hidden · ⌘2"}
            }
        }
    }
}
