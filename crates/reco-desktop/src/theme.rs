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

use makepad_widgets::*;

script_mod! {
    mod.themes.reco_dark = mod.themes.dark{
        // Geometry
        corner_radius: 5.0
        container_corner_radius: 10.0
        textselection_corner_radius: 3.0
        beveling: 1.0
        space_factor: 7.0
        space_1: 3.5
        space_2: 7.0
        space_3: 10.5
        mspace_1: mod.turtle.Inset{top: 3.5 right: 3.5 bottom: 3.5 left: 3.5}

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
        color_text_placeholder: #x6b7280
        color_text_meta: #xa0a7b2
        color_on_surface_variant: #xa0a7b2
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
        color_icon: #xa0a7b2
        color_icon_hover: #xe6e9ef
        color_icon_focus: #xe6e9ef
        color_icon_active: #xe6e9ef
        color_icon_down: #xe6e9ef
        color_icon_disabled: #x4b5160

        // Controls: raised (outset) and sunken (inset) fills
        color_outset: #x23272f
        color_outset_1: #x23272f
        color_outset_2: #x23272f
        color_outset_hover: #x2b303a
        color_outset_1_hover: #x2b303a
        color_outset_2_hover: #x2b303a
        color_outset_focus: #x23272f
        color_outset_1_focus: #x23272f
        color_outset_2_focus: #x23272f
        color_outset_active: #x34d399
        color_outset_1_active: #x23272f
        color_outset_2_active: #x23272f
        color_outset_down: #x1c2026
        color_outset_1_down: #x1c2026
        color_outset_2_down: #x1c2026
        color_outset_disabled: #x181b20
        color_outset_1_disabled: #x181b20
        color_outset_2_disabled: #x181b20
        color_outset_empty: #x23272f
        color_outset_1_empty: #x23272f
        color_outset_2_empty: #x23272f
        color_outset_drag: #x2b303a
        color_outset_1_drag: #x2b303a
        color_outset_2_drag: #x2b303a
        color_inset: #x171a1f
        color_inset_1: #x171a1f
        color_inset_2: #x171a1f
        color_inset_hover: #x1b1f25
        color_inset_1_hover: #x1b1f25
        color_inset_2_hover: #x1b1f25
        color_inset_focus: #x1b1f25
        color_inset_1_focus: #x1b1f25
        color_inset_2_focus: #x1b1f25
        color_inset_active: #x1b1f25
        color_inset_1_active: #x1b1f25
        color_inset_2_active: #x1b1f25
        color_inset_down: #x171a1f
        color_inset_1_down: #x171a1f
        color_inset_2_down: #x171a1f
        color_inset_disabled: #x14171c
        color_inset_1_disabled: #x14171c
        color_inset_2_disabled: #x14171c
        color_inset_empty: #x171a1f
        color_inset_1_empty: #x171a1f
        color_inset_2_empty: #x171a1f
        color_inset_drag: #x1b1f25
        color_inset_1_drag: #x1b1f25
        color_inset_2_drag: #x1b1f25

        // Hairline borders (bevels); focus borders use the accent
        color_bevel: #x2a2f37
        color_bevel_hover: #x353b45
        color_bevel_down: #x2a2f37
        color_bevel_focus: #x34d399
        color_bevel_disabled: #x1f232a
        color_bevel_inset_1: #x2a2f37
        color_bevel_inset_2: #x2a2f37
        color_bevel_outset_1: #x2a2f37
        color_bevel_outset_2: #x2a2f37
        color_bevel_inset_1_hover: #x353b45
        color_bevel_inset_2_hover: #x353b45
        color_bevel_outset_1_hover: #x353b45
        color_bevel_outset_2_hover: #x353b45
        color_bevel_inset_1_focus: #x34d399
        color_bevel_inset_2_focus: #x34d399
        color_bevel_outset_1_focus: #x34d399
        color_bevel_outset_2_focus: #x34d399
        color_bevel_inset_1_active: #x2a2f37
        color_bevel_inset_2_active: #x2a2f37
        color_bevel_outset_1_active: #x2a2f37
        color_bevel_outset_2_active: #x2a2f37
        color_bevel_inset_1_down: #x2a2f37
        color_bevel_inset_2_down: #x2a2f37
        color_bevel_outset_1_down: #x2a2f37
        color_bevel_outset_2_down: #x2a2f37
        color_bevel_inset_1_disabled: #x1f232a
        color_bevel_inset_2_disabled: #x1f232a
        color_bevel_outset_1_disabled: #x1f232a
        color_bevel_outset_2_disabled: #x1f232a

        // Reco surfaces and sizes
        reco_panel: #x14171c
        reco_surface: #x1a1d23
        reco_hairline: #x2a2f37
        reco_viewer: #x08090b
        reco_text_secondary: #xa0a7b2
        reco_text_muted: #x6b7280
        reco_record: #xef4444
        reco_topbar_height: 44.0
        reco_transport_height: 52.0
        reco_statusbar_height: 26.0
        reco_panel_pad: 10.0
        reco_gap: 8.0
    }
    mod.theme = mod.themes.reco_dark
}
