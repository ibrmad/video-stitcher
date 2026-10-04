//! Shared building blocks: text roles, cards, buttons, folds, badges and
//! status dots. Every screen builds from these so one control looks and
//! behaves the same everywhere.

use makepad_widgets::*;

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    // Text roles: display, title, body, meta (hints and metadata). Weight and
    // tone do the rest of the hierarchy. Text never carries padding or a
    // margin: Makepad's Label applies both twice (once to its box, once to
    // its text), so an inset label drifts off its edge. The container owns
    // every gap and inset instead.
    mod.widgets.RecoText = Label{
        padding: 0
        draw_text +: {color: theme.color_text text_style: theme.font_regular{font_size: theme.reco_font_body}}
    }
    mod.widgets.RecoDisplay = mod.widgets.RecoText{
        draw_text +: {text_style: theme.font_bold{font_size: theme.reco_font_display}}
    }
    mod.widgets.RecoTitle = mod.widgets.RecoText{
        draw_text +: {text_style: theme.font_bold{font_size: theme.reco_font_title}}
    }
    mod.widgets.RecoBody = mod.widgets.RecoText{}
    mod.widgets.RecoSecondary = mod.widgets.RecoText{
        draw_text +: {color: theme.reco_text_secondary}
    }
    mod.widgets.RecoMuted = mod.widgets.RecoText{
        draw_text +: {color: theme.reco_text_muted text_style: theme.font_regular{font_size: theme.reco_font_meta}}
    }
    // An explanation under a control. Wraps inside its card.
    mod.widgets.RecoHint = mod.widgets.RecoMuted{width: Fill}
    mod.widgets.RecoSectionTitle = mod.widgets.RecoText{
        draw_text +: {text_style: theme.font_bold{font_size: theme.reco_font_body}}
    }
    // A side panel's name, on its cards' content edge. Set `label.text`.
    mod.widgets.RecoPanelHeader = View{
        width: Fill height: Fit
        padding: Inset{left: theme.reco_space_xl top: theme.reco_space_xs bottom: theme.reco_space_xs}
        label := mod.widgets.RecoTitle{text: ""}
    }

    // A titled group inside a side panel: a card with a hairline edge.
    mod.widgets.RecoSection = RoundedView{
        width: Fill height: Fit flow: Down spacing: theme.reco_space_m
        padding: Inset{left: theme.reco_space_xl right: theme.reco_space_xl top: theme.reco_space_l bottom: theme.reco_space_xl}
        show_bg: true new_batch: true
        draw_bg +: {
            color: theme.reco_surface
            border_radius: theme.container_corner_radius
            border_size: theme.reco_hairline_width
            border_color: theme.reco_hairline
        }
    }

    // Makepad's disabled state dims a button's face and label but not its
    // icon, so Reco's buttons fade the icon as well.
    mod.widgets.RecoIconButton = ButtonIcon{
        width: theme.reco_icon_button height: theme.reco_icon_button padding: 0 margin: 0 text: ""
        icon_walk: Walk{width: theme.reco_icon height: theme.reco_icon}
        draw_icon +: {color: theme.reco_text_secondary}
        animator +: {
            disabled +: {
                off +: {apply +: {draw_icon: {opacity: theme.reco_icon_opacity}}}
                on +: {apply +: {draw_icon: {opacity: theme.reco_disabled_icon_opacity}}}
            }
        }
    }
    mod.widgets.RecoButton = Button{
        height: theme.reco_control_height margin: 0
        icon_walk: Walk{width: theme.reco_icon_small height: theme.reco_icon_small}
        draw_icon +: {color: theme.reco_text_secondary}
        animator +: {
            disabled +: {
                off +: {apply +: {draw_icon: {opacity: theme.reco_icon_opacity}}}
                on +: {apply +: {draw_icon: {opacity: theme.reco_disabled_icon_opacity}}}
            }
        }
    }
    mod.widgets.RecoFlatButton = ButtonFlat{
        height: theme.reco_control_height margin: 0
        icon_walk: Walk{width: theme.reco_icon_small height: theme.reco_icon_small}
        draw_icon +: {color: theme.reco_text_secondary}
    }
    // A quiet text action (status bar). Shows a face on hover and the accent
    // ring on keyboard focus.
    mod.widgets.RecoLinkButton = ButtonFlat{
        height: theme.reco_compact_height margin: 0
        padding: Inset{left: theme.reco_space_m right: theme.reco_space_m}
        draw_bg +: {
            color: theme.reco_transparent
            color_hover: theme.color_outset_hover
            color_focus: theme.reco_transparent
            color_down: theme.color_outset_down
            border_color: theme.reco_transparent
            border_color_hover: theme.reco_transparent
            border_color_focus: theme.color_focus
            border_color_down: theme.reco_transparent
        }
        draw_text +: {
            color: theme.reco_text_secondary
            text_style: theme.font_regular{font_size: theme.reco_font_meta}
        }
    }
    // The one primary action of a screen. Its focus ring is light so it
    // reads on the green face; disabled, its icon greys with the label.
    mod.widgets.RecoPrimaryButton = ButtonPrimary{
        height: theme.reco_primary_height margin: 0
        padding: Inset{left: theme.reco_button_pad_x right: theme.reco_button_pad_x}
        icon_walk: Walk{width: theme.reco_icon_medium height: theme.reco_icon_medium}
        draw_bg +: {
            border_size: theme.reco_focus_width
            border_color_focus: theme.reco_focus_contrast
            border_color_disabled: theme.reco_hairline
        }
        animator +: {
            disabled +: {
                off +: {apply +: {draw_icon: {color: theme.color_on_primary}}}
                on +: {apply +: {draw_icon: {color: theme.color_icon_disabled}}}
            }
        }
    }
    mod.widgets.RecoCheckBox = CheckBox{
        draw_bg +: {border_color_focus: theme.reco_focus_contrast}
    }
    // Makepad's slider sits 4 pt in from its row and 8 pt down; Reco's
    // sliders start on the content edge like everything else.
    mod.widgets.RecoSlider = Slider{margin: 0}
    // The transport's timeline. Makepad's minimal slider keeps its track at
    // the bottom of its box (room for a label the timeline does not have),
    // which leaves the track under the transport's centre line. This face
    // draws the track, the played part and a playhead centred in the box,
    // and the whole box stays the hit target.
    mod.widgets.RecoTimeline = SliderMinimal{
        width: Fill height: theme.reco_icon_button margin: 0
        text: ""
        draw_bg +: {
            track_color: uniform(theme.reco_progress_track)
            focus_color: uniform(theme.color_focus)
            track_thickness: uniform(theme.reco_timeline_track)
            knob_radius: uniform(theme.reco_timeline_knob)
            pixel: fn() {
                let sdf = Sdf2d.viewport(self.pos * self.rect_size)
                let cy = self.rect_size.y * 0.5
                let t = self.track_thickness
                // The playhead stays inside the box at both ends.
                let x0 = self.knob_radius
                let x1 = self.rect_size.x - self.knob_radius
                let x = mix(x0, x1, self.slide_pos)
                // Sdf2d.box draws twice the radius it is given.
                sdf.box(x0, cy - t * 0.5, x1 - x0, t, t * 0.25)
                sdf.fill(self.track_color)
                sdf.box(x0, cy - t * 0.5, max(t, x - x0), t, t * 0.25)
                sdf.fill(mix(self.val_color, self.val_color_disabled, self.disabled))
                sdf.circle(x, cy, self.knob_radius * (1.0 + 0.2 * self.hover))
                sdf.fill_keep(
                    mix(
                        mix(self.handle_color, self.handle_color_hover, self.hover)
                        self.handle_color_disabled
                        self.disabled
                    )
                )
                sdf.stroke(mix(vec4(0.0, 0.0, 0.0, 0.0), self.focus_color, self.focus), 1.5)
                return sdf.result
            }
        }
    }
    mod.widgets.RecoProgressBar = ProgressBar{
        height: theme.reco_progress_thickness
        draw_bg +: {
            thickness: theme.reco_progress_thickness
            track_color: theme.reco_progress_track
        }
    }
    // Makepad's dropdown paints its arrow under the face, so the arrow
    // vanishes when the face is opaque. This face draws the arrow last.
    mod.widgets.RecoDropDown = DropDown{
        height: theme.reco_control_height margin: 0
        draw_bg +: {
            pixel: fn() {
                let sdf = Sdf2d.viewport(self.pos * self.rect_size)
                sdf.box(
                    self.border_size
                    self.border_size
                    self.rect_size.x - self.border_size * 2.
                    self.rect_size.y - self.border_size * 2.
                    self.border_radius
                )
                let fill = self.color
                    .mix(self.color_focus, self.focus)
                    .mix(self.color_hover, self.hover)
                    .mix(self.color_down, self.down * self.hover)
                    .mix(self.color_disabled, self.disabled)
                let stroke = self.border_color
                    .mix(self.border_color_hover, self.hover)
                    .mix(self.border_color_focus, self.focus)
                    .mix(self.border_color_disabled, self.disabled)
                sdf.fill_keep(fill)
                sdf.stroke(stroke, self.border_size)
                let c = vec2(self.rect_size.x - 12., self.rect_size.y * 0.5)
                let sz = 3.
                sdf.move_to(c.x - sz, c.y - sz * 0.6)
                sdf.line_to(c.x + sz, c.y - sz * 0.6)
                sdf.line_to(c.x, c.y + sz * 0.6)
                sdf.close_path()
                sdf.fill(self.arrow_color.mix(self.arrow_color_hover, self.hover).mix(self.arrow_color_disabled, self.disabled))
                return sdf.result
            }
        }
    }

    // A collapsible card section, open by default. Set `header.title.text`.
    mod.widgets.RecoFold = FoldHeader{
        header: View{
            width: Fill height: Fit flow: Right spacing: theme.reco_space_s align: Align{y: 0.5}
            fold_button := FoldButton{}
            title := mod.widgets.RecoSectionTitle{text: ""}
        }
        body: View{
            width: Fill height: Fit flow: Down spacing: theme.reco_space_m
            padding: Inset{top: theme.reco_space_m}
        }
    }
    // The "Advanced" tier inside a section, closed by default (Rule 9).
    mod.widgets.RecoAdvanced = FoldHeader{
        animator +: {active: {default: @off}}
        header: View{
            width: Fill height: Fit flow: Right spacing: theme.reco_space_s align: Align{y: 0.5}
            padding: Inset{top: theme.reco_space_xs}
            fold_button := FoldButton{animator +: {active: {default: @off}}}
            mod.widgets.RecoSecondary{text: "Advanced"}
        }
        body: View{
            width: Fill height: Fit flow: Down spacing: theme.reco_space_m
            padding: Inset{top: theme.reco_space_m}
        }
    }

    // Status dots: idle, working, done.
    mod.widgets.RecoDot = RoundedView{
        width: theme.reco_dot height: theme.reco_dot
        show_bg: true
        draw_bg +: {color: theme.reco_dot_idle border_radius: theme.reco_dot_radius}
    }
    mod.widgets.RecoDotBusy = mod.widgets.RecoDot{draw_bg +: {color: theme.reco_dot_busy}}
    mod.widgets.RecoDotOk = mod.widgets.RecoDot{draw_bg +: {color: theme.reco_dot_ok}}

    // Round badges: a camera side (L/R) or a step number. Set `label.text`.
    mod.widgets.RecoBadge = RoundedView{
        width: theme.reco_badge height: theme.reco_badge
        align: Align{x: 0.5 y: 0.5}
        show_bg: true new_batch: true
        draw_bg +: {color: theme.reco_badge_idle border_radius: theme.reco_badge_radius}
        label := Label{
            padding: 0
            text: ""
            draw_text +: {color: theme.reco_text_secondary text_style: theme.font_bold{font_size: theme.reco_font_meta}}
        }
    }
    mod.widgets.RecoBadgeOn = mod.widgets.RecoBadge{
        draw_bg +: {color: theme.color_primary_container}
        label +: {draw_text +: {color: theme.color_on_primary_container}}
    }
    mod.widgets.RecoBadgeCurrent = mod.widgets.RecoBadge{
        draw_bg +: {color: theme.color_primary}
        label +: {draw_text +: {color: theme.color_on_primary}}
    }
}
