//! The preview's keyboard shortcuts: one pure mapping from a key to what it
//! asks of the preview.

use makepad_widgets::*;

/// What a key asks of the preview.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum KeyCommand {
    /// Drag by points.
    Pan {
        /// Horizontal points.
        dx: f32,
        /// Vertical points.
        dy: f32,
    },
    /// Change the FOV by degrees (negative zooms in).
    Zoom {
        /// Degrees.
        degrees: f32,
    },
    /// Back to the rest pose.
    ResetView,
    /// Play or pause.
    TogglePlay,
    /// Seek by seconds.
    SeekBy {
        /// Seconds (negative goes back).
        seconds: f64,
    },
    /// Toggle fullscreen.
    Fullscreen,
}

/// Points an arrow key pans.
pub const ARROW_PAN: f32 = 20.0;
/// Degrees a zoom key changes the FOV.
pub const KEY_ZOOM: f32 = 5.0;
/// Seconds a bracket key seeks.
pub const BRACKET_SEEK: f64 = 5.0;

/// What a menu shortcut asks of the app.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AppShortcut {
    /// ⌘S: save the calibration.
    Save,
    /// ⌘,: Preferences.
    Preferences,
    /// ⌘1: the Setup panel.
    ToggleSetup,
    /// ⌘2: the Adjust panel.
    ToggleAdjust,
    /// ⌘3: the time panel's lanes.
    ToggleTime,
}

/// The menu shortcut `key` makes with ⌘ (Ctrl off macOS) and nothing else
/// held. The macOS menu bar lists these, and the app also reads them as
/// keys, wherever the keyboard is: injected keys (the checks) and a menu
/// whose key equivalents don't fire (as on some Macs) work the same.
/// A key the menu takes never reaches the window, so nothing runs twice.
pub fn app_shortcut(key: KeyCode, modifiers: &KeyModifiers) -> Option<AppShortcut> {
    let command = if cfg!(target_os = "macos") {
        modifiers.logo && !modifiers.control
    } else {
        modifiers.control && !modifiers.logo
    };
    if !command || modifiers.shift || modifiers.alt {
        return None;
    }
    Some(match key {
        KeyCode::KeyS => AppShortcut::Save,
        KeyCode::Comma => AppShortcut::Preferences,
        KeyCode::Key1 => AppShortcut::ToggleSetup,
        KeyCode::Key2 => AppShortcut::ToggleAdjust,
        KeyCode::Key3 => AppShortcut::ToggleTime,
        _ => return None,
    })
}

/// The command for `key`, or `None`. Keys held with ⌘, Ctrl or Option are
/// menu shortcuts and never drive the preview.
pub fn command_for_key(key: KeyCode, modifiers: &KeyModifiers) -> Option<KeyCommand> {
    if modifiers.logo || modifiers.control || modifiers.alt {
        return None;
    }
    Some(match key {
        KeyCode::Space => KeyCommand::TogglePlay,
        KeyCode::ArrowLeft => KeyCommand::Pan {
            dx: -ARROW_PAN,
            dy: 0.0,
        },
        KeyCode::ArrowRight => KeyCommand::Pan {
            dx: ARROW_PAN,
            dy: 0.0,
        },
        KeyCode::ArrowUp => KeyCommand::Pan {
            dx: 0.0,
            dy: -ARROW_PAN,
        },
        KeyCode::ArrowDown => KeyCommand::Pan {
            dx: 0.0,
            dy: ARROW_PAN,
        },
        KeyCode::F11 => KeyCommand::Fullscreen,
        _ => return None,
    })
}

/// The preview's command for a typed character: R, F, + = - _ [ ] and
/// the keypad's + and − are matched by what they type, so they follow the
/// keyboard's layout.
pub fn command_for_text(text: &str) -> Option<KeyCommand> {
    Some(match text {
        "+" | "=" => KeyCommand::Zoom { degrees: -KEY_ZOOM },
        "-" | "_" | "−" => KeyCommand::Zoom { degrees: KEY_ZOOM },
        "[" => KeyCommand::SeekBy {
            seconds: -BRACKET_SEEK,
        },
        "]" => KeyCommand::SeekBy {
            seconds: BRACKET_SEEK,
        },
        "r" | "R" => KeyCommand::ResetView,
        "f" | "F" => KeyCommand::Fullscreen,
        _ => return None,
    })
}

/// One row of the Keyboard shortcuts sheet.
pub struct Shortcut {
    /// The keys, as the sheet shows them.
    pub keys: &'static str,
    /// What they do.
    pub does: &'static str,
    /// The keys [`command_for_key`] answers for this row (none for the
    /// pointer's and the menus' rows). Read by the test that holds this
    /// table to the handler.
    #[cfg_attr(not(test), allow(dead_code))]
    pub codes: &'static [KeyCode],
    /// The typed characters [`command_for_text`] answers for this row.
    #[cfg_attr(not(test), allow(dead_code))]
    pub texts: &'static [&'static str],
    /// A menu's key (the macOS menu bar's: listed only there).
    pub menu: bool,
}

const fn key(
    keys: &'static str,
    does: &'static str,
    codes: &'static [KeyCode],
    texts: &'static [&'static str],
) -> Shortcut {
    Shortcut {
        keys,
        does,
        codes,
        texts,
        menu: false,
    }
}

const fn menu(keys: &'static str, does: &'static str) -> Shortcut {
    Shortcut {
        keys,
        does,
        codes: &[],
        texts: &[],
        menu: true,
    }
}

/// Every shortcut, in the sheet's order: the preview's keys (the table the
/// key handler is tested against), the pointer, then the menus' keys.
pub const SHORTCUTS: &[Shortcut] = &[
    key("Space", "Play or pause", &[KeyCode::Space], &[]),
    key("[  /  ]", "Back or forward 5 seconds", &[], &["[", "]"]),
    key(
        "← → ↑ ↓",
        "Pan the view",
        &[
            KeyCode::ArrowLeft,
            KeyCode::ArrowRight,
            KeyCode::ArrowUp,
            KeyCode::ArrowDown,
        ],
        &[],
    ),
    key("+  /  −", "Zoom in or out", &[], &["+", "=", "-", "_", "−"]),
    key("R", "Reset the view", &[], &["r", "R"]),
    key("F  /  F11", "Full screen", &[KeyCode::F11], &["f", "F"]),
    key("Drag", "Pan the view", &[], &[]),
    key("Scroll", "Zoom in or out", &[], &[]),
    menu(
        "⌘1  ⌘2  ⌘3",
        "Show or hide the Setup, Adjust and Time panels",
    ),
    menu("⌘S", "Save the calibration"),
    menu("⌘,", "Preferences"),
    menu("⌘Q", "Quit"),
];

/// Whether holding the key repeats the command: moves and seeks do; play,
/// reset and fullscreen act once per press.
pub fn repeats(command: KeyCommand) -> bool {
    matches!(
        command,
        KeyCommand::Pan { .. } | KeyCommand::Zoom { .. } | KeyCommand::SeekBy { .. }
    )
}

/// What F (or F11) does to the window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WindowStep {
    /// Maximize: on macOS this toggles full screen.
    Maximize,
    /// Back from maximized.
    Restore,
}

/// F's step: on macOS `maximize` toggles full screen (Makepad's
/// `fullscreen()` does nothing there); Windows and Linux have no full
/// screen in Makepad (FRICTION.md), so F maximizes the window and then
/// restores it.
pub fn fullscreen_step(macos: bool, full: bool) -> WindowStep {
    if macos || !full {
        WindowStep::Maximize
    } else {
        WindowStep::Restore
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn f_toggles_full_screen_or_the_maximized_window() {
        assert_eq!(
            fullscreen_step(true, false),
            WindowStep::Maximize,
            "macOS toggles itself"
        );
        assert_eq!(fullscreen_step(true, true), WindowStep::Maximize);
        assert_eq!(fullscreen_step(false, false), WindowStep::Maximize);
        assert_eq!(
            fullscreen_step(false, true),
            WindowStep::Restore,
            "and back"
        );
    }

    fn plain(key: KeyCode) -> Option<KeyCommand> {
        command_for_key(key, &KeyModifiers::default())
    }

    #[test]
    fn maps_the_preview_shortcuts() {
        assert_eq!(plain(KeyCode::Space), Some(KeyCommand::TogglePlay));
        assert_eq!(
            plain(KeyCode::ArrowLeft),
            Some(KeyCommand::Pan { dx: -20.0, dy: 0.0 })
        );
        assert_eq!(
            plain(KeyCode::ArrowRight),
            Some(KeyCommand::Pan { dx: 20.0, dy: 0.0 })
        );
        assert_eq!(
            plain(KeyCode::ArrowUp),
            Some(KeyCommand::Pan { dx: 0.0, dy: -20.0 })
        );
        assert_eq!(
            plain(KeyCode::ArrowDown),
            Some(KeyCommand::Pan { dx: 0.0, dy: 20.0 })
        );
        assert_eq!(plain(KeyCode::F11), Some(KeyCommand::Fullscreen));
        for zoom_in in ["+", "="] {
            assert_eq!(
                command_for_text(zoom_in),
                Some(KeyCommand::Zoom { degrees: -5.0 })
            );
        }
        for zoom_out in ["-", "_", "−"] {
            assert_eq!(
                command_for_text(zoom_out),
                Some(KeyCommand::Zoom { degrees: 5.0 })
            );
        }
        assert_eq!(
            command_for_text("["),
            Some(KeyCommand::SeekBy { seconds: -5.0 })
        );
        assert_eq!(
            command_for_text("]"),
            Some(KeyCommand::SeekBy { seconds: 5.0 })
        );
        for reset in ["r", "R"] {
            assert_eq!(command_for_text(reset), Some(KeyCommand::ResetView));
        }
        for full in ["f", "F"] {
            assert_eq!(command_for_text(full), Some(KeyCommand::Fullscreen));
        }
        assert_eq!(command_for_text(" "), None, "Space is a key, not text");
        assert_eq!(command_for_text("ff"), None);
    }

    #[test]
    fn keys_that_type_follow_the_layout() {
        // Their places differ by layout ("+" is the US "]" key on a German
        // keyboard): the typed character decides.
        for key in [
            KeyCode::Equals,
            KeyCode::Minus,
            KeyCode::LBracket,
            KeyCode::RBracket,
            KeyCode::KeyR,
            KeyCode::KeyF,
            KeyCode::NumpadAdd,
            KeyCode::NumpadSubtract,
        ] {
            assert_eq!(plain(key), None, "{key:?}");
        }
    }

    #[test]
    fn modified_keys_are_ignored() {
        for modifiers in [
            KeyModifiers {
                logo: true,
                ..Default::default()
            },
            KeyModifiers {
                control: true,
                ..Default::default()
            },
            KeyModifiers {
                alt: true,
                ..Default::default()
            },
        ] {
            assert_eq!(command_for_key(KeyCode::Equals, &modifiers), None);
            assert_eq!(command_for_key(KeyCode::Space, &modifiers), None);
        }
    }

    #[test]
    fn the_sheet_lists_every_key_the_preview_handles() {
        let typed: Vec<&str> = SHORTCUTS.iter().flat_map(|s| s.texts).copied().collect();
        for c in (' '..='~').chain(['−']) {
            let text = c.to_string();
            let handled = command_for_text(&text).is_some();
            assert_eq!(
                handled,
                typed.contains(&text.as_str()),
                "{text:?}: handled {handled}"
            );
        }
        let listed: Vec<KeyCode> = SHORTCUTS.iter().flat_map(|s| s.codes).copied().collect();
        for key in makepad_key_code::KEYCODE_VARIANTS {
            let handled = command_for_key(key, &KeyModifiers::default()).is_some();
            assert_eq!(
                handled,
                listed.contains(&key),
                "{key:?}: handled {handled}, listed {}",
                listed.contains(&key)
            );
        }
        assert!(
            SHORTCUTS.iter().any(|s| s.keys == "Scroll"),
            "the pointer's rows too"
        );
        assert!(
            SHORTCUTS.iter().any(|s| s.keys == "⌘,"),
            "the menus' keys too"
        );
    }

    #[test]
    fn command_keys_are_the_menu_shortcuts() {
        let mac = cfg!(target_os = "macos");
        let command = KeyModifiers {
            logo: mac,
            control: !mac,
            ..KeyModifiers::default()
        };
        for (key, shortcut, listed) in [
            (KeyCode::KeyS, AppShortcut::Save, "⌘S"),
            (KeyCode::Comma, AppShortcut::Preferences, "⌘,"),
            (KeyCode::Key1, AppShortcut::ToggleSetup, "⌘1"),
            (KeyCode::Key2, AppShortcut::ToggleAdjust, "⌘2"),
            (KeyCode::Key3, AppShortcut::ToggleTime, "⌘3"),
        ] {
            assert_eq!(app_shortcut(key, &command), Some(shortcut), "{listed}");
            assert!(
                SHORTCUTS
                    .iter()
                    .any(|s| s.menu && s.keys.split_whitespace().any(|k| k == listed)),
                "the sheet lists {listed} with the menus' keys"
            );
        }
        assert_eq!(
            app_shortcut(KeyCode::KeyS, &KeyModifiers::default()),
            None,
            "S alone does nothing"
        );
        assert_eq!(app_shortcut(KeyCode::KeyA, &command), None);
        let shifted = KeyModifiers {
            shift: true,
            ..command
        };
        assert_eq!(
            app_shortcut(KeyCode::KeyS, &shifted),
            None,
            "⇧⌘S isn't Save"
        );
        let option = KeyModifiers {
            alt: true,
            ..command
        };
        assert_eq!(
            app_shortcut(KeyCode::Key1, &option),
            None,
            "⌥⌘1 isn't Setup"
        );
    }

    #[test]
    fn toggles_do_not_repeat() {
        assert!(!repeats(KeyCommand::TogglePlay));
        assert!(!repeats(KeyCommand::ResetView));
        assert!(!repeats(KeyCommand::Fullscreen));
        assert!(repeats(KeyCommand::Pan { dx: 20.0, dy: 0.0 }));
        assert!(repeats(KeyCommand::Zoom { degrees: 5.0 }));
        assert!(repeats(KeyCommand::SeekBy { seconds: 5.0 }));
    }
}
