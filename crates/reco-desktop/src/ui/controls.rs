//! Shared building blocks, after Rerun's `re_ui`: text roles, 24 pt rows,
//! title rows, section bands, flat buttons, the slider track, badges and
//! status dots. Every screen builds from these so one control looks and
//! behaves the same everywhere.

use makepad_widgets::*;

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    // Text roles. One size (12 px Inter Medium); colour does the hierarchy:
    // strong (white) for names and titles, text for values, subdued for
    // labels and meta. Text never carries padding or a margin: Makepad's
    // Label applies both twice (once to its box, once to its text), so an
    // inset label drifts off its edge. The container owns every inset.
    mod.widgets.RecoText = Label{
        padding: 0
        draw_text +: {color: theme.reco_text text_style: theme.font_regular{font_size: theme.reco_font_body}}
    }
    mod.widgets.RecoStrong = mod.widgets.RecoText{draw_text +: {color: theme.reco_text_strong}}
    mod.widgets.RecoSubdued = mod.widgets.RecoText{draw_text +: {color: theme.reco_text_subdued}}
    mod.widgets.RecoSmall = mod.widgets.RecoText{
        draw_text +: {color: theme.reco_text_subdued text_style: theme.font_regular{font_size: theme.reco_font_small}}
    }
    mod.widgets.RecoDisplay = mod.widgets.RecoText{
        draw_text +: {color: theme.reco_text_strong text_style: theme.font_bold{font_size: theme.reco_font_display}}
    }
    // A one-line meta value that cuts with "…" instead of wrapping.
    mod.widgets.RecoMeta = mod.widgets.RecoSubdued{
        width: Fill
        max_lines: 1
        text_overflow: TextOverflow.Ellipsis
    }

    // Rows. Everything in a panel sits in 24 pt rows that start on the
    // panel's content edge (12 pt in), as Rerun's list items do.
    mod.widgets.RecoRow = View{
        width: Fill height: theme.reco_row flow: Right spacing: theme.reco_gap align: Align{y: 0.5}
        padding: Inset{left: theme.reco_pad right: theme.reco_pad}
    }
    // A panel's name with its actions on the right.
    mod.widgets.RecoTitleRow = View{
        width: Fill height: theme.reco_row flow: Right spacing: theme.reco_gap_s align: Align{y: 0.5}
        padding: Inset{left: theme.reco_pad right: theme.reco_pad}
    }
    mod.widgets.RecoSeparator = SolidView{
        width: Fill height: theme.reco_separator_width
        draw_bg.color: theme.reco_separator
    }
    // A label on the left, its control on the right: Rerun's property row.
    mod.widgets.RecoLabelCell = View{
        width: theme.reco_label_width height: Fit
    }

    // Buttons: flat faces, no borders, 22 pt tall, no outer margin (Makepad
    // adds 4 pt above and below by default). A click does not take the
    // keyboard (as on macOS), so the preview keeps its shortcuts and Space
    // never re-clicks the last button; Tab still focuses every button.
    mod.widgets.RecoButton = Button{
        height: theme.reco_button margin: 0 grab_key_focus: false
        padding: Inset{left: theme.reco_button_pad_x right: theme.reco_button_pad_x}
        spacing: theme.reco_gap_s
        icon_walk: Walk{width: theme.reco_icon_small height: theme.reco_icon_small}
        draw_text +: {text_style: theme.font_regular{font_size: theme.reco_font_body}}
        draw_icon +: {color: theme.reco_text}
        animator +: {
            disabled +: {
                off +: {apply +: {draw_icon: {opacity: theme.reco_icon_opacity}}}
                on +: {apply +: {draw_icon: {opacity: theme.reco_disabled_icon_opacity}}}
            }
        }
    }
    // The one primary action of a screen: white on deep green. Disabled, its
    // icon greys with the label.
    mod.widgets.RecoPrimaryButton = ButtonPrimary{
        height: theme.reco_button margin: 0 grab_key_focus: false
        padding: Inset{left: theme.reco_button_pad_x + theme.reco_gap_s right: theme.reco_button_pad_x + theme.reco_gap_s}
        spacing: theme.reco_gap_s
        icon_walk: Walk{width: theme.reco_icon_small height: theme.reco_icon_small}
        draw_text +: {text_style: theme.font_regular{font_size: theme.reco_font_body}}
        draw_bg +: {
            color_hover: theme.reco_accent_fill_hover
            border_size: theme.reco_focus_width
            border_color_focus: theme.reco_accent
            border_color_disabled: theme.reco_hover
        }
        animator +: {
            disabled +: {
                off +: {apply +: {draw_icon: {color: theme.reco_on_accent}}}
                on +: {apply +: {draw_icon: {color: theme.color_icon_disabled}}}
            }
        }
    }
    // An icon-only action: no face until hovered.
    mod.widgets.RecoIconButton = ButtonIcon{
        width: theme.reco_icon_button height: theme.reco_icon_button padding: 0 margin: 0 text: ""
        grab_key_focus: false
        icon_walk: Walk{width: theme.reco_icon height: theme.reco_icon}
        draw_bg +: {
            color: theme.reco_transparent
            color_focus: theme.reco_transparent
            color_hover: theme.reco_hover
            color_down: theme.reco_band
            color_disabled: theme.reco_transparent
            border_color: theme.reco_transparent
            border_color_hover: theme.reco_transparent
            border_color_down: theme.reco_transparent
            border_color_focus: theme.reco_accent
            border_color_disabled: theme.reco_transparent
            border_color_2: theme.reco_transparent
            border_color_2_hover: theme.reco_transparent
            border_color_2_down: theme.reco_transparent
            border_color_2_focus: theme.reco_accent
            border_color_2_disabled: theme.reco_transparent
        }
        draw_icon +: {color: theme.reco_icon_tint}
        animator +: {
            disabled +: {
                off +: {apply +: {draw_icon: {opacity: theme.reco_icon_opacity}}}
                on +: {apply +: {draw_icon: {opacity: theme.reco_disabled_icon_opacity}}}
            }
        }
    }
    // An icon at the end of a row, title row or band. It reaches past the
    // row's padding by its own inset, so the icon's ink (not its box) ends
    // on the content edge, in line with text and buttons above and below.
    mod.widgets.RecoRowIcon = mod.widgets.RecoIconButton{
        width: theme.reco_icon_button height: theme.reco_icon_button
        margin: Inset{right: (theme.reco_row_icon - theme.reco_icon_button) * 0.5}
        icon_walk: Walk{width: theme.reco_row_icon height: theme.reco_row_icon}
    }
    // A text action without a face, for menus and quiet commands.
    mod.widgets.RecoFlatButton = mod.widgets.RecoButton{
        draw_bg +: {
            color: theme.reco_transparent
            color_focus: theme.reco_transparent
            color_hover: theme.reco_hover
            color_down: theme.reco_band
            color_disabled: theme.reco_transparent
            border_color: theme.reco_transparent
            border_color_hover: theme.reco_transparent
            border_color_down: theme.reco_transparent
            border_color_disabled: theme.reco_transparent
            border_color_2: theme.reco_transparent
            border_color_2_hover: theme.reco_transparent
            border_color_2_down: theme.reco_transparent
            border_color_2_disabled: theme.reco_transparent
        }
        draw_text +: {color: theme.reco_text_subdued}
        draw_icon +: {color: theme.reco_text_subdued}
    }
    mod.widgets.RecoCheckBox = CheckBox{
        margin: 0
        padding: 0
        // The text starts clear of the box (Makepad's default margin counts
        // on padding this checkbox doesn't have).
        label_walk +: {margin: Inset{left: theme.reco_check_label}}
        draw_text +: {text_style: theme.font_regular{font_size: theme.reco_font_body}}
        draw_bg +: {size: theme.reco_check_box border_color_focus: theme.reco_accent}
    }
    // Makepad's dropdown paints its arrow under the face, so the arrow
    // vanishes when the face is opaque. This face draws a chevron last.
    // The menu opens below the control, as Rerun's do: its rows stay where
    // they were whatever is chosen, and a dropdown near the top never puts
    // rows in the title bar, where a press drags the window instead.
    mod.widgets.RecoDropDown = DropDown{
        height: theme.reco_button margin: 0
        popup_menu_position: #(makepad_widgets::drop_down::PopupMenuPosition::BelowInput)
        // The menu's rows and panel match Reco's own menus.
        popup_menu: PopupMenu{
            menu_item: PopupMenuItem{
                height: theme.reco_row
                align: Align{y: 0.5}
                draw_text +: {text_style: theme.font_regular{font_size: theme.reco_font_body}}
            }
            draw_bg +: {
                color: theme.reco_band
                border_color: theme.reco_widget
                border_color_2: theme.reco_widget
            }
        }
        draw_text +: {text_style: theme.font_regular{font_size: theme.reco_font_body}}
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
                let c = vec2(self.rect_size.x - 11., self.rect_size.y * 0.5)
                sdf.move_to(c.x - 3., c.y - 1.5)
                sdf.line_to(c.x, c.y + 1.5)
                sdf.line_to(c.x + 3., c.y - 1.5)
                sdf.stroke(self.arrow_color.mix(self.arrow_color_hover, self.hover).mix(self.arrow_color_disabled, self.disabled), 1.25)
                return sdf.result
            }
        }
    }

    // A disclosure chevron: › closed, ⌄ open (Rerun's collapse arrow).
    mod.widgets.RecoChevron = FoldButton{
        width: theme.reco_chevron height: theme.reco_chevron margin: 0
        draw_bg +: {
            color: uniform(theme.reco_text_subdued)
            color_hover: uniform(theme.reco_text_strong)
            color_active: uniform(theme.reco_text_subdued)
            pixel: fn() {
                let sdf = Sdf2d.viewport(self.pos * self.rect_size)
                let c = self.rect_size * 0.5
                sdf.clear(vec4(0.))
                sdf.rotate((self.active - 1.0) * 0.5 * PI, c.x, c.y)
                sdf.move_to(c.x - 3., c.y - 1.5)
                sdf.line_to(c.x, c.y + 1.5)
                sdf.line_to(c.x + 3., c.y - 1.5)
                sdf.stroke(
                    mix(
                        mix(self.color, self.color_hover, self.hover)
                        mix(self.color_active, self.color_hover, self.hover)
                        self.active
                    )
                    1.25
                )
                return sdf.result * self.fade
            }
        }
    }

    // A collapsible section: a full-width band with a chevron, a title and
    // an optional help mark, over rows. Open by default. Set
    // `header.title.text` and `header.help.text`.
    mod.widgets.RecoSection = RecoFold{
        header: SolidView{
            width: Fill height: theme.reco_row flow: Right spacing: theme.reco_gap_s align: Align{y: 0.5}
            padding: Inset{left: theme.reco_pad right: theme.reco_pad}
            draw_bg.color: theme.reco_band
            fold_button := mod.widgets.RecoChevron{}
            title := mod.widgets.RecoStrong{text: ""}
            View{width: Fill height: Fit}
            help := Tip{
                text: ""
                help_icon := mod.widgets.RecoRowIcon{
                    draw_icon +: {svg: crate_resource("self:resources/icons/help.svg")}
                }
            }
        }
        body: View{
            width: Fill height: Fit flow: Down
            padding: Inset{top: theme.reco_gap_s bottom: theme.reco_gap}
        }
    }
    // The "Advanced" tier inside a section, closed by default (Rule 9).
    mod.widgets.RecoAdvanced = RecoFold{
        animator +: {active: {default: @off}}
        header: View{
            width: Fill height: theme.reco_row flow: Right spacing: theme.reco_gap_s align: Align{y: 0.5}
            padding: Inset{left: theme.reco_pad right: theme.reco_pad}
            fold_button := mod.widgets.RecoChevron{animator +: {active: {default: @off}}}
            title := mod.widgets.RecoSubdued{text: "Advanced"}
        }
        body: View{width: Fill height: Fit flow: Down}
    }

    // A slider track for a property row: a thin centred track, the value in
    // the accent and a round knob. Makepad's minimal slider keeps its track
    // at the bottom of its box (room for a label), so this face draws its
    // own; the whole box stays the hit target. The row shows the value.
    mod.widgets.RecoSlider = SliderMinimal{
        width: Fill height: theme.reco_row margin: 0
        text: ""
        text_input +: {
            is_read_only: true
            draw_text +: {
                color: theme.reco_transparent color_hover: theme.reco_transparent
                color_focus: theme.reco_transparent
                color_down: theme.reco_transparent color_disabled: theme.reco_transparent
                // The placeholder colours too: a slider made inside a sheet
                // starts in the field's empty state.
                color_empty: theme.reco_transparent color_empty_hover: theme.reco_transparent
                color_empty_focus: theme.reco_transparent
            }
            draw_cursor +: {color: theme.reco_transparent}
        }
        draw_bg +: {
            track_color: uniform(theme.reco_widget)
            focus_color: uniform(theme.reco_accent)
            track_thickness: uniform(theme.reco_slider_track)
            knob_radius: uniform(theme.reco_slider_knob)
            pixel: fn() {
                let sdf = Sdf2d.viewport(self.pos * self.rect_size)
                let cy = self.rect_size.y * 0.5
                let t = self.track_thickness
                // The knob stays inside the box at both ends.
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
            track_color: theme.reco_widget
        }
    }

    // Status dots: idle, working, done, failed.
    mod.widgets.RecoDot = RoundedView{
        width: theme.reco_dot height: theme.reco_dot
        show_bg: true
        draw_bg +: {color: theme.reco_dot_idle border_radius: theme.reco_dot_radius}
    }
    mod.widgets.RecoDotBusy = mod.widgets.RecoDot{draw_bg +: {color: theme.reco_dot_busy}}
    mod.widgets.RecoDotOk = mod.widgets.RecoDot{draw_bg +: {color: theme.reco_dot_ok}}
    mod.widgets.RecoDotError = mod.widgets.RecoDot{draw_bg +: {color: theme.reco_dot_error}}

    // Round badges: a camera side (L/R) or a step number. Set `label.text`.
    mod.widgets.RecoBadge = RoundedView{
        width: theme.reco_badge height: theme.reco_badge
        align: Align{x: 0.5 y: 0.5}
        show_bg: true new_batch: true
        draw_bg +: {color: theme.reco_badge_idle border_radius: theme.reco_badge_radius}
        label := Label{
            padding: 0
            text: ""
            draw_text +: {color: theme.reco_text_subdued text_style: theme.font_regular{font_size: theme.reco_font_small}}
        }
    }
    mod.widgets.RecoBadgeOn = mod.widgets.RecoBadge{
        draw_bg +: {color: theme.color_primary_container}
        label +: {draw_text +: {color: theme.color_on_primary_container}}
    }
    mod.widgets.RecoBadgeCurrent = mod.widgets.RecoBadge{
        draw_bg +: {color: theme.reco_accent_fill}
        label +: {draw_text +: {color: theme.reco_on_accent}}
    }
}
