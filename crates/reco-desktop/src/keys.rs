//! The preview's keyboard shortcuts (PARITY.md Module 1, from reco-gui's
//! FocusScope): one pure mapping from a key to what it asks of the preview.

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
        KeyCode::Equals | KeyCode::NumpadAdd => KeyCommand::Zoom { degrees: -KEY_ZOOM },
        KeyCode::Minus | KeyCode::NumpadSubtract => KeyCommand::Zoom { degrees: KEY_ZOOM },
        KeyCode::LBracket => KeyCommand::SeekBy {
            seconds: -BRACKET_SEEK,
        },
        KeyCode::RBracket => KeyCommand::SeekBy {
            seconds: BRACKET_SEEK,
        },
        KeyCode::KeyR => KeyCommand::ResetView,
        KeyCode::KeyF | KeyCode::F11 => KeyCommand::Fullscreen,
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
    /// A menu's key (the macOS menu bar's: listed only there).
    pub menu: bool,
}

const fn key(keys: &'static str, does: &'static str, codes: &'static [KeyCode]) -> Shortcut {
    Shortcut {
        keys,
        does,
        codes,
        menu: false,
    }
}

const fn menu(keys: &'static str, does: &'static str) -> Shortcut {
    Shortcut {
        keys,
        does,
        codes: &[],
        menu: true,
    }
}

/// Every shortcut, in the sheet's order: the preview's keys (the table the
/// key handler is tested against), the pointer, then the menus' keys.
pub const SHORTCUTS: &[Shortcut] = &[
    key("Space", "Play or pause", &[KeyCode::Space]),
    key(
        "[  /  ]",
        "Back or forward 5 seconds",
        &[KeyCode::LBracket, KeyCode::RBracket],
    ),
    key(
        "← → ↑ ↓",
        "Pan the view",
        &[
            KeyCode::ArrowLeft,
            KeyCode::ArrowRight,
            KeyCode::ArrowUp,
            KeyCode::ArrowDown,
        ],
    ),
    key(
        "+  /  −",
        "Zoom in or out",
        &[
            KeyCode::Equals,
            KeyCode::NumpadAdd,
            KeyCode::Minus,
            KeyCode::NumpadSubtract,
        ],
    ),
    key("R", "Reset the view", &[KeyCode::KeyR]),
    key("F  /  F11", "Full screen", &[KeyCode::KeyF, KeyCode::F11]),
    key("Drag", "Pan the view", &[]),
    key("Scroll", "Zoom in or out", &[]),
    menu(
        "⌘1  ⌘2  ⌘3",
        "Show or hide the Setup, Adjust and Time panels",
    ),
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

#[cfg(test)]
mod tests {
    use super::*;

    fn plain(key: KeyCode) -> Option<KeyCommand> {
        command_for_key(key, &KeyModifiers::default())
    }

    #[test]
    fn maps_the_slint_shortcuts() {
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
        assert_eq!(
            plain(KeyCode::Equals),
            Some(KeyCommand::Zoom { degrees: -5.0 })
        );
        assert_eq!(
            plain(KeyCode::NumpadAdd),
            Some(KeyCommand::Zoom { degrees: -5.0 })
        );
        assert_eq!(
            plain(KeyCode::Minus),
            Some(KeyCommand::Zoom { degrees: 5.0 })
        );
        assert_eq!(
            plain(KeyCode::LBracket),
            Some(KeyCommand::SeekBy { seconds: -5.0 })
        );
        assert_eq!(
            plain(KeyCode::RBracket),
            Some(KeyCommand::SeekBy { seconds: 5.0 })
        );
        assert_eq!(plain(KeyCode::KeyR), Some(KeyCommand::ResetView));
        assert_eq!(plain(KeyCode::KeyF), Some(KeyCommand::Fullscreen));
        assert_eq!(plain(KeyCode::F11), Some(KeyCommand::Fullscreen));
        assert_eq!(plain(KeyCode::KeyQ), None);
    }

    #[test]
    fn shift_still_counts() {
        // '+' is Shift+'=' and '_' is Shift+'-'.
        let shift = KeyModifiers {
            shift: true,
            ..Default::default()
        };
        assert_eq!(
            command_for_key(KeyCode::Equals, &shift),
            Some(KeyCommand::Zoom { degrees: -5.0 })
        );
        assert_eq!(
            command_for_key(KeyCode::Minus, &shift),
            Some(KeyCommand::Zoom { degrees: 5.0 })
        );
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
    fn toggles_do_not_repeat() {
        assert!(!repeats(KeyCommand::TogglePlay));
        assert!(!repeats(KeyCommand::ResetView));
        assert!(!repeats(KeyCommand::Fullscreen));
        assert!(repeats(KeyCommand::Pan { dx: 20.0, dy: 0.0 }));
        assert!(repeats(KeyCommand::Zoom { degrees: 5.0 }));
        assert!(repeats(KeyCommand::SeekBy { seconds: 5.0 }));
    }
}
