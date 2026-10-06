//! The Reco look: one theme derived from Makepad's dark theme (DESIGN.md
//! Rule 5). Every colour, radius, spacing step and type size the app uses
//! is set here.
//!
//! The design reference is the Rerun viewer (`re_ui`): neutral greys, Inter
//! Medium, flat panels with section bands, and one
//! accent kept for selection, focus, the primary action and progress. Reco's
//! accent is pitch green where Rerun's is blue.
//!
//! Stock widgets copy theme values when they are registered, so this module
//! runs after `makepad_widgets::theme_mod` and before
//! `makepad_widgets::widgets_mod`. Makepad's theme values are computed once,
//! so every role a stock widget reads is set explicitly rather than derived
//! from a few knobs. Reco's own surfaces read the `reco_*` keys.
//!
//! Units: sizes are layout points; font sizes are typographic points
//! (Makepad renders 1 pt as 96/72 points, so 9 pt is 12 px). Makepad's
//! `Sdf2d.box` draws a corner of twice the radius it is given, so the radii
//! below are half the rendered radius.
//!
//! Contrast on the panel (#0d0d0d): text 12.5:1, subdued text 6.1:1 (5.1:1
//! on a section band), white on the green fill 5.8:1. Disabled text is
//! quieter by design.

use makepad_widgets::*;

script_mod! {
    use mod.text.*
    use mod.res.*

    mod.themes.reco_dark = mod.themes.dark{
        // Geometry. Rendered radii: controls 4 pt, cards and menus 6 pt.
        corner_radius: 2.0
        container_corner_radius: 3.0
        textselection_corner_radius: 1.0
        beveling: 1.0
        space_factor: 8.0
        space_1: 4.0
        space_2: 8.0
        space_3: 12.0
        mspace_1: mod.turtle.Inset{top: 4.0 right: 4.0 bottom: 4.0 left: 4.0}
        font_size_p: 9.75

        // Inter, as Rerun: Medium for everything, SemiBold for the one large
        // title. Makepad ships Inter as a variable font.
        font_regular: TextStyle{
            font_family: FontFamily{
                latin := FontMember{res: crate_resource("makepad_widgets:resources/Inter.ttf") weight: 500.0 asc: 0.0 desc: 0.0}
            }
            line_spacing: 1.33
        }
        font_label: TextStyle{
            font_family: FontFamily{
                latin := FontMember{res: crate_resource("makepad_widgets:resources/Inter.ttf") weight: 500.0 asc: 0.0 desc: 0.0}
            }
            line_spacing: 1.33
        }
        font_bold: TextStyle{
            font_family: FontFamily{
                latin := FontMember{res: crate_resource("makepad_widgets:resources/Inter.ttf") weight: 600.0 asc: 0.0 desc: 0.0}
            }
            line_spacing: 1.2
        }

        // Surfaces
        color_bg_app: #x0d0d0d
        color_fg_app: #x0d0d0d
        color_app_caption_bar: #x0d0d0d
        color_bg_container: #x0d0d0d
        color_bg_highlight: #x007541
        color_bg_highlight_inline: #x007541

        // Accent and focus: bright green rings and fills, deep green faces.
        color_focus: #x34d399
        color_ctrl_selected: #x34d399
        color_ctrl_active: #x34d399
        color_primary: #x007541
        color_on_primary: #xffffff
        color_primary_container: #x0b3a24
        color_on_primary_container: #x8ff0c4
        color_text_on_accent: #xffffff
        color_success: #x34d399
        color_warning: #xfbbf24
        color_error: #xef4444
        color_cursor: #xffffff
        color_text_cursor: #xffffff

        // Text, labels and icons
        color_text: #xcfcfcf
        color_text_hover: #xffffff
        color_text_focus: #xffffff
        color_text_active: #xffffff
        color_text_down: #xffffff
        color_text_disabled: #x646363
        color_text_placeholder: #x939090
        color_text_meta: #x939090
        color_text_val: #xcfcfcf
        color_on_surface_variant: #x939090
        color_on_surface: #xcfcfcf
        // Floating surfaces (menus, popups) and their edges, as Rerun's.
        color_surface: #x0d0d0d
        color_surface_container_lowest: #x0d0d0d
        color_surface_container_low: #x171717
        color_surface_container: #x212121
        color_surface_container_high: #x212121
        color_surface_container_highest: #x2c2b2b
        color_outline: #x2c2b2b
        color_outline_variant: #x272626
        color_label: #xcfcfcf
        color_label_hover: #xffffff
        color_label_focus: #xffffff
        color_label_active: #xffffff
        color_label_down: #xffffff
        color_label_disabled: #x646363
        color_label_inner: #xcfcfcf
        color_label_inner_hover: #xffffff
        color_label_inner_focus: #xffffff
        color_label_inner_active: #xffffff
        color_label_inner_down: #xffffff
        color_label_inner_disabled: #x646363
        color_label_outer: #xcfcfcf
        color_label_outer_hover: #xffffff
        color_label_outer_focus: #xffffff
        color_label_outer_active: #xffffff
        color_label_outer_down: #xffffff
        color_label_outer_disabled: #x646363
        color_icon: #x9f9f9f
        color_icon_hover: #xffffff
        color_icon_focus: #xffffff
        color_icon_active: #xffffff
        color_icon_down: #xffffff
        color_icon_disabled: #x525151

        // Raised controls (buttons, dropdowns): a flat face, no border.
        color_outset: #x2c2b2b
        color_outset_1: #x2c2b2b
        color_outset_2: #x2c2b2b
        color_outset_hover: #x383737
        color_outset_1_hover: #x383737
        color_outset_2_hover: #x383737
        color_outset_focus: #x2c2b2b
        color_outset_1_focus: #x2c2b2b
        color_outset_2_focus: #x2c2b2b
        color_outset_active: #x007541
        color_outset_1_active: #x2c2b2b
        color_outset_2_active: #x2c2b2b
        color_outset_down: #x242323
        color_outset_1_down: #x242323
        color_outset_2_down: #x242323
        color_outset_disabled: #x1a1a1a
        color_outset_1_disabled: #x1a1a1a
        color_outset_2_disabled: #x1a1a1a
        color_outset_empty: #x2c2b2b
        color_outset_1_empty: #x2c2b2b
        color_outset_2_empty: #x2c2b2b
        color_outset_drag: #x383737
        color_outset_1_drag: #x383737
        color_outset_2_drag: #x383737

        // Sunken controls (checkbox wells, text fields)
        color_inset: #x212121
        color_inset_1: #x212121
        color_inset_2: #x212121
        color_inset_hover: #x272626
        color_inset_1_hover: #x272626
        color_inset_2_hover: #x272626
        color_inset_focus: #x272626
        color_inset_1_focus: #x272626
        color_inset_2_focus: #x272626
        color_inset_active: #x272626
        color_inset_1_active: #x272626
        color_inset_2_active: #x272626
        color_inset_down: #x212121
        color_inset_1_down: #x212121
        color_inset_2_down: #x212121
        color_inset_disabled: #x171717
        color_inset_1_disabled: #x171717
        color_inset_2_disabled: #x171717
        color_inset_empty: #x212121
        color_inset_1_empty: #x212121
        color_inset_2_empty: #x212121
        color_inset_drag: #x272626
        color_inset_1_drag: #x272626
        color_inset_2_drag: #x272626

        // Borders: raised faces have none (the border matches the face);
        // wells keep a quiet edge; focus always shows the accent.
        color_bevel: #x2c2b2b
        color_bevel_hover: #x383737
        color_bevel_down: #x242323
        color_bevel_focus: #x34d399
        color_bevel_disabled: #x1a1a1a
        color_bevel_inset_1: #x3a3939
        color_bevel_inset_2: #x3a3939
        color_bevel_outset_1: #x2c2b2b
        color_bevel_outset_2: #x2c2b2b
        color_bevel_inset_1_hover: #x525151
        color_bevel_inset_2_hover: #x525151
        color_bevel_outset_1_hover: #x383737
        color_bevel_outset_2_hover: #x383737
        color_bevel_inset_1_focus: #x34d399
        color_bevel_inset_2_focus: #x34d399
        color_bevel_outset_1_focus: #x34d399
        color_bevel_outset_2_focus: #x34d399
        color_bevel_inset_1_active: #x3a3939
        color_bevel_inset_2_active: #x3a3939
        color_bevel_outset_1_active: #x2c2b2b
        color_bevel_outset_2_active: #x2c2b2b
        color_bevel_inset_1_down: #x3a3939
        color_bevel_inset_2_down: #x3a3939
        color_bevel_outset_1_down: #x242323
        color_bevel_outset_2_down: #x242323
        color_bevel_inset_1_disabled: #x272626
        color_bevel_inset_2_disabled: #x272626
        color_bevel_outset_1_disabled: #x1a1a1a
        color_bevel_outset_2_disabled: #x1a1a1a

        // Sliders: the filled value is state, so it takes the accent.
        color_val: #x34d399
        color_val_hover: #x5ce0a8
        color_val_focus: #x34d399
        color_val_drag: #x5ce0a8
        color_val_disabled: #x3a3939
        color_handle: #xcfcfcf
        color_handle_hover: #xffffff
        color_handle_focus: #xffffff
        color_handle_drag: #xffffff
        color_handle_disabled: #x525151

        // Reco surfaces, darkest to lightest (Rerun's grey scale)
        reco_viewport: #x000000
        reco_panel: #x0d0d0d
        reco_bar: #x171717
        reco_band: #x212121
        reco_hover: #x272626
        // A selected list row: the accent at 15% (selection is green, as
        // Rerun's is blue).
        reco_selection_fill: #x34d39926
        reco_widget: #x2c2b2b
        reco_stroke: #x525151
        reco_separator: #x272626
        reco_separator_width: 1.0
        reco_transparent: #x0000

        // Reco text and icons
        reco_text: #xcfcfcf
        reco_text_strong: #xffffff
        reco_text_subdued: #x939090
        reco_text_disabled: #x646363
        reco_icon_tint: #x9f9f9f

        // Reco accent: bright green for rings, fills and lit marks; deep
        // green faces carry white text.
        reco_accent: #x34d399
        reco_accent_fill: #x007541
        reco_accent_fill_hover: #x00854a
        reco_on_accent: #xffffff
        reco_record: #xef4444
        reco_error: #xef4444
        // Text that warns without failing (a choice the export can't run).
        reco_warning: #xfbbf24
        // The lookahead track's zones (the GPU's memory): comfortable,
        // tight, too long.
        reco_zone_safe: #x34d399
        reco_zone_tight: #xfbbf24
        reco_zone_risk: #xef4444
        // How strongly the zones show past the knob (the value's side is
        // full strength).
        reco_zone_dim: 0.35
        reco_focus_width: 1.5
        // Icon opacity on enabled and disabled controls.
        reco_icon_opacity: 1.0
        reco_disabled_icon_opacity: 0.5
        // Status dots
        reco_dot_idle: #x646363
        reco_dot_busy: #xfbbf24
        reco_dot_ok: #x34d399
        reco_dot_error: #xef4444
        reco_badge_idle: #x2c2b2b

        // The panorama frame drawn in the empty states: two camera halves
        // meeting at the seam.
        reco_frame_line: #x2c2b2b
        reco_frame_fill: #x0d0d0d
        reco_frame_lit: #x0b2a1c
        reco_seam: #x34d399
        reco_seam_idle: #x525151
        reco_panorama_aspect: 2.5
        reco_panorama_max_width: 520.0
        reco_panorama_radius: 3.0

        // Placeholder pitch shown by --look-preview until Module 1 draws the
        // stitched video.
        reco_grass_a: #x1d5a3a
        reco_grass_b: #x1a5235
        reco_pitch_line: #xcfe3d6

        // The time panel: camera lanes, chapters and the playhead.
        reco_lane_fill: #x2c2b2b
        reco_lane_fill_on: #x1a7446
        reco_lane_edge: #x0d0d0d
        // The export range over the ruler and lanes: the accent at 18%.
        reco_range_tint: #x34d3992e
        reco_tick: #x3a3939
        reco_playhead: #xffffff

        // Type (points): 13 px for everything (macOS's own size), 12 px for
        // small print, one 22 px title for the next step. Rerun's 12 px
        // and 24 pt rows read too small on a 1x display (owner,
        // 2026-10-05), so the design has more room than Rerun's.
        reco_font_body: 9.75
        reco_font_small: 9.0
        reco_font_display: 16.5

        // Spacing (points): 14 pt view padding, 10 pt item spacing, 5 pt
        // between an icon and its text, 16 pt indent.
        reco_gap_xs: 2.0
        reco_gap_s: 5.0
        reco_gap: 10.0
        reco_pad: 14.0
        reco_indent: 16.0
        reco_gap_l: 18.0
        reco_gap_xl: 28.0

        // Sizes (points)
        reco_row: 28.0
        // The view bar and the panels' title rows: a 26 pt control keeps
        // 3 pt above and below (at 28 it kept 1, tight on Retina).
        reco_bar_height: 32.0
        reco_button: 26.0
        reco_button_pad_x: 10.0
        reco_icon_button: 26.0
        reco_icon: 18.0
        reco_icon_small: 14.0
        // Icons at the end of a panel row, title row or section band, and
        // their hover face: 3 pt around the icon leaves 5 pt above and below
        // in a title row and 3 pt in a band.
        reco_row_icon: 16.0
        reco_row_icon_button: 22.0
        // Record and Stop: Lucide's filled circle and square fill their box,
        // so they are drawn smaller to read as a dot and a square.
        reco_record_icon: 10.0
        // Lucide's step-back sits this far inside its 18 pt box (its tip at
        // 4 of 24, less half the stroke).
        reco_glyph_inset: 1.69
        reco_icon_tiny: 11.0
        reco_chevron: 14.0
        // The chevron's drawing inside its box, stroke included (drawn at a
        // fixed size, whatever the box): a trailing chevron's ink lines up
        // with the content edge by the difference.
        reco_chevron_ink: 7.25
        reco_badge: 18.0
        // A file row's name sits under its camera's name: the row's pad, the
        // badge and the gap after it.
        reco_file_indent: 42.0
        reco_badge_radius: 9.0
        reco_dot: 7.0
        reco_dot_radius: 3.5
        reco_link_width: 1.0
        reco_link_height: 10.0
        reco_value_width: 50.0
        reco_label_width: 124.0
        reco_aspect_width: 80.0
        reco_quality_width: 92.0
        // Record's button keeps one width recording or not ("Record" and
        // "■ 1:02:03"), so the view bar doesn't move when recording starts.
        reco_record_width: 80.0
        reco_progress_width: 440.0
        // The export sheet, and the time fields inside it.
        reco_sheet_width: 540.0
        // A sheet's rows scroll past this height: room for its band, its
        // buttons and a margin above and below in a short window.
        reco_sheet_rows_max: "calc(100vh - 160px)"
        reco_time_input: 80.0
        // The lens picker's results: ten rows, then it scrolls.
        reco_picker_list: 280.0
        // A few lines of writing (the bug report's description).
        reco_text_area: 84.0
        reco_toast_width: 360.0
        reco_progress_thickness: 4.0
        reco_slider_track: 4.0
        reco_slider_knob: 6.0
        // A checkbox's mark box, and where its text starts: after the box
        // and the 10 pt gap controls keep (Makepad's 13 assumes padding).
        reco_check_box: 16.0
        reco_check_label: 26.0
        reco_time_width: 58.0
        reco_ruler_height: 18.0
        reco_playhead_head: 6.0
        reco_chapter_gap: 2.0
        reco_controls_height: 32.0
        reco_tick_length: 6.0
        reco_tick_spacing: 80.0
        // A lane row and the file blocks inside it.
        reco_lane_row: 18.0
        reco_lane_inset: 4.0
        reco_playhead_width: 1.5
        reco_menu_width: 220.0
        // Room for the macOS window buttons at the left of the title bar.
        reco_caption_inset: 76.0
        reco_media_width: 290.0
        reco_media_min: 220.0
        reco_viewer_min: 360.0
        reco_inspector_width: 310.0
        // The Adjust panel's splitter floor includes the 6 pt bar, so 226
        // keeps the panel itself at 220.
        reco_inspector_floor: 226.0
    }
    mod.theme = mod.themes.reco_dark
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    /// The names `mod.themes.reco_dark` sets, in order: the `name:` that
    /// starts each of its own lines.
    fn tokens(source: &str) -> Vec<&str> {
        source
            .lines()
            .skip_while(|line| !line.contains("mod.themes.reco_dark = "))
            .skip(1)
            .take_while(|line| *line != "    }")
            .filter_map(|line| line.strip_prefix("        "))
            .filter_map(|line| line.split_once(':').map(|(name, _)| name))
            .filter(|name| {
                !name.is_empty()
                    && name
                        .chars()
                        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
            })
            .collect()
    }

    /// A name set twice keeps only its last value, silently: a 32 pt size
    /// once took the name of the view bar's colour, and the bar drew white.
    #[test]
    fn theme_tokens_are_set_once() {
        let names = tokens(include_str!("theme.rs"));
        assert!(names.len() > 100, "the token list was read ({})", names.len());
        let mut seen = HashSet::new();
        let twice: Vec<&str> = names.into_iter().filter(|name| !seen.insert(*name)).collect();
        assert!(twice.is_empty(), "theme tokens set more than once: {twice:?}");
    }
}
