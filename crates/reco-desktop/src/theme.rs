//! The Reco look: one theme derived from Makepad's dark theme (DESIGN.md
//! Rule 5). Every colour, radius, spacing step and type size the app uses
//! is set here.
//!
//! Stock widgets copy theme values when they are registered, so this module
//! runs after `makepad_widgets::theme_mod` and before
//! `makepad_widgets::widgets_mod`. Makepad's theme values are computed once,
//! so every role a stock widget reads is set explicitly rather than derived
//! from a few knobs; the role list follows Makepad's own macOS-dark style.
//! Reco's own surfaces read the `reco_*` keys.
//!
//! Units: sizes are layout points; font sizes are typographic points
//! (Makepad renders 1 pt as 96/72 points, so 10.25 pt is about 13.7 px).
//! Makepad's `Sdf2d.box` draws a corner of twice the radius it is given, so
//! the radii below are half the rendered radius.
//!
//! Text roles: body text, values and placeholders all reach at least 4.5:1
//! on every surface; disabled text stays readable but quieter.

use makepad_widgets::*;

script_mod! {
    mod.themes.reco_dark = mod.themes.dark{
        // Geometry. Rendered radii: controls 6 pt, cards and the viewer 11 pt.
        corner_radius: 3.0
        container_corner_radius: 5.5
        textselection_corner_radius: 1.5
        beveling: 1.0
        space_factor: 8.0
        space_1: 4.0
        space_2: 8.0
        space_3: 12.0
        mspace_1: mod.turtle.Inset{top: 4.0 right: 4.0 bottom: 4.0 left: 4.0}
        font_size_p: 10.25

        // Surfaces
        color_bg_app: #x0f1115
        color_fg_app: #x0f1115
        color_app_caption_bar: #x0f1115
        color_bg_container: #x14171c
        color_bg_highlight: #x1f4d3a
        color_bg_highlight_inline: #x1f4d3a

        // Accent and focus
        color_focus: #x34d399
        color_ctrl_selected: #x34d399
        color_ctrl_active: #x34d399
        color_primary: #x34d399
        color_on_primary: #x052e1c
        color_primary_container: #x133d2c
        color_on_primary_container: #xa7f3d0
        color_text_on_accent: #x052e1c
        color_success: #x34d399
        color_warning: #xfbbf24
        color_error: #xf87171
        color_cursor: #xe6e9ef
        color_text_cursor: #xe6e9ef

        // Text, labels and icons
        color_text: #xe6e9ef
        color_text_hover: #xe6e9ef
        color_text_focus: #xe6e9ef
        color_text_active: #xe6e9ef
        color_text_down: #xe6e9ef
        color_text_disabled: #x6b7280
        color_text_placeholder: #x8b93a1
        color_text_meta: #xa6adb8
        color_text_val: #xc9ced6
        color_on_surface_variant: #xa6adb8
        color_label: #xe6e9ef
        color_label_hover: #xe6e9ef
        color_label_focus: #xe6e9ef
        color_label_active: #xe6e9ef
        color_label_down: #xe6e9ef
        color_label_disabled: #x6b7280
        color_label_inner: #xe6e9ef
        color_label_inner_hover: #xe6e9ef
        color_label_inner_focus: #xe6e9ef
        color_label_inner_active: #xe6e9ef
        color_label_inner_down: #xe6e9ef
        color_label_inner_disabled: #x6b7280
        color_label_outer: #xe6e9ef
        color_label_outer_hover: #xe6e9ef
        color_label_outer_focus: #xe6e9ef
        color_label_outer_active: #xe6e9ef
        color_label_outer_down: #xe6e9ef
        color_label_outer_disabled: #x6b7280
        color_icon: #xa6adb8
        color_icon_hover: #xe6e9ef
        color_icon_focus: #xe6e9ef
        color_icon_active: #xe6e9ef
        color_icon_down: #xe6e9ef
        color_icon_disabled: #x5c6270

        // Raised controls (buttons, dropdowns): a face lighter than the card
        // and a border that reads on every surface.
        color_outset: #x252931
        color_outset_1: #x252931
        color_outset_2: #x252931
        color_outset_hover: #x2d323b
        color_outset_1_hover: #x2d323b
        color_outset_2_hover: #x2d323b
        color_outset_focus: #x252931
        color_outset_1_focus: #x252931
        color_outset_2_focus: #x252931
        color_outset_active: #x34d399
        color_outset_1_active: #x252931
        color_outset_2_active: #x252931
        color_outset_down: #x1f232a
        color_outset_1_down: #x1f232a
        color_outset_2_down: #x1f232a
        color_outset_disabled: #x1b1e24
        color_outset_1_disabled: #x1b1e24
        color_outset_2_disabled: #x1b1e24
        color_outset_empty: #x252931
        color_outset_1_empty: #x252931
        color_outset_2_empty: #x252931
        color_outset_drag: #x2d323b
        color_outset_1_drag: #x2d323b
        color_outset_2_drag: #x2d323b

        // Sunken controls (slider tracks, checkbox wells)
        color_inset: #x101317
        color_inset_1: #x101317
        color_inset_2: #x101317
        color_inset_hover: #x13161b
        color_inset_1_hover: #x13161b
        color_inset_2_hover: #x13161b
        color_inset_focus: #x13161b
        color_inset_1_focus: #x13161b
        color_inset_2_focus: #x13161b
        color_inset_active: #x13161b
        color_inset_1_active: #x13161b
        color_inset_2_active: #x13161b
        color_inset_down: #x101317
        color_inset_1_down: #x101317
        color_inset_2_down: #x101317
        color_inset_disabled: #x15181d
        color_inset_1_disabled: #x15181d
        color_inset_2_disabled: #x15181d
        color_inset_empty: #x101317
        color_inset_1_empty: #x101317
        color_inset_2_empty: #x101317
        color_inset_drag: #x13161b
        color_inset_1_drag: #x13161b
        color_inset_2_drag: #x13161b

        // Control borders (bevels). Focus uses the accent; disabled keeps a
        // visible outline so the control's shape never disappears.
        color_bevel: #x363c46
        color_bevel_hover: #x434a56
        color_bevel_down: #x363c46
        color_bevel_focus: #x34d399
        color_bevel_disabled: #x2a2e36
        color_bevel_inset_1: #x3a404a
        color_bevel_inset_2: #x3a404a
        color_bevel_outset_1: #x363c46
        color_bevel_outset_2: #x363c46
        color_bevel_inset_1_hover: #x474e5a
        color_bevel_inset_2_hover: #x474e5a
        color_bevel_outset_1_hover: #x434a56
        color_bevel_outset_2_hover: #x434a56
        color_bevel_inset_1_focus: #x34d399
        color_bevel_inset_2_focus: #x34d399
        color_bevel_outset_1_focus: #x34d399
        color_bevel_outset_2_focus: #x34d399
        color_bevel_inset_1_active: #x3a404a
        color_bevel_inset_2_active: #x3a404a
        color_bevel_outset_1_active: #x363c46
        color_bevel_outset_2_active: #x363c46
        color_bevel_inset_1_down: #x3a404a
        color_bevel_inset_2_down: #x3a404a
        color_bevel_outset_1_down: #x363c46
        color_bevel_outset_2_down: #x363c46
        color_bevel_inset_1_disabled: #x2a2e36
        color_bevel_inset_2_disabled: #x2a2e36
        color_bevel_outset_1_disabled: #x2a2e36
        color_bevel_outset_2_disabled: #x2a2e36

        // Sliders: the filled value is state, so it takes the accent.
        color_val: #x34d399
        color_val_hover: #x5ce0a8
        color_val_focus: #x34d399
        color_val_drag: #x5ce0a8
        color_val_disabled: #x3a3f48
        color_handle: #xe6e9ef
        color_handle_hover: #xf5f7fa
        color_handle_focus: #xe6e9ef
        color_handle_drag: #xffffff
        color_handle_disabled: #x4b5160

        // Reco surfaces
        reco_panel: #x14171c
        reco_surface: #x1a1d23
        reco_hairline: #x2a2e36
        reco_hairline_width: 1.0
        reco_viewer: #x08090b
        reco_text_secondary: #xa6adb8
        reco_text_muted: #x8b93a1
        reco_record: #xef4444
        reco_transparent: #x0000
        // A focus ring that reads on the green accent as well as on dark faces.
        reco_focus_contrast: #xe6e9ef
        reco_focus_width: 2.0
        // Icon opacity on enabled and disabled controls (disabled stays ≥3:1).
        reco_icon_opacity: 1.0
        reco_disabled_icon_opacity: 0.55
        // Status dots
        reco_dot_idle: #x6b7280
        reco_dot_busy: #xfbbf24
        reco_dot_ok: #x34d399
        reco_badge_idle: #x252931

        // The panorama frame drawn in the empty states: two camera halves
        // meeting at the seam.
        reco_frame_line: #x2f343d
        reco_frame_fill: #x101317
        reco_frame_lit: #x15392a
        reco_seam: #x34d399
        reco_seam_idle: #x3a404a
        // The unfilled part of a progress bar.
        reco_progress_track: #x363c46
        reco_progress_thickness: 6.0
        // The transport's timeline: track thickness and playhead radius.
        reco_timeline_track: 4.0
        reco_timeline_knob: 6.0
        reco_panorama_aspect: 2.5
        reco_panorama_max_width: 560.0
        reco_panorama_radius: 4.0

        // Placeholder pitch shown by --look-preview until Module 1 draws the
        // stitched video.
        reco_grass_a: #x1d5a3a
        reco_grass_b: #x1a5235
        reco_pitch_line: #xcfe3d6

        // Type scale (points): four roles, each step at least 1.14x.
        reco_font_meta: 9.0
        reco_font_body: 10.25
        reco_font_title: 11.75
        reco_font_display: 17.0

        // Spacing scale (points)
        reco_space_hair: 2.0
        reco_space_xs: 4.0
        reco_space_s: 6.0
        reco_space_m: 8.0
        reco_space_l: 10.0
        reco_space_xl: 12.0
        reco_space_xxl: 16.0
        reco_space_xxxl: 24.0

        // Control and layout sizes (points)
        reco_compact_height: 24.0
        reco_control_height: 28.0
        reco_primary_height: 30.0
        reco_button_pad_x: 14.0
        reco_icon_button: 30.0
        reco_icon: 16.0
        reco_icon_medium: 14.0
        reco_icon_small: 12.0
        reco_icon_tiny: 10.0
        reco_play_button: 36.0
        reco_play_icon: 18.0
        reco_badge: 22.0
        reco_badge_radius: 11.0
        reco_dot: 8.0
        reco_dot_radius: 4.0
        reco_link_width: 2.0
        reco_link_height: 14.0
        reco_time_width: 56.0
        reco_aspect_width: 84.0
        reco_progress_width: 420.0
        // Room for the macOS window buttons at the left of the title bar.
        reco_caption_inset: 80.0
        reco_transport_height: 52.0
        reco_statusbar_height: 30.0
        reco_media_width: 260.0
        reco_media_min: 180.0
        reco_viewer_min: 360.0
        reco_inspector_width: 280.0
        // The Inspector's splitter floor includes the 6 pt bar, so 206
        // keeps the panel itself at 200.
        reco_inspector_floor: 206.0
    }
    mod.theme = mod.themes.reco_dark
}
