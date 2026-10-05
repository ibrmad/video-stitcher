//! The app menu in the App: Preferences, Keyboard shortcuts, Report a bug,
//! and this build's version.

use makepad_widgets::*;
use reco_app::help;
use reco_app::reveal::open_in_browser;
use reco_app::toasts::Severity;

use crate::keys::SHORTCUTS;
use crate::ui::key_table::RecoKeyTable;
use crate::App;

/// This build's version, and its commit when known: "0.5.4 (2fba497)".
pub(crate) fn version_line() -> String {
    version_text(env!("CARGO_PKG_VERSION"), option_env!("RECO_GIT_HASH"))
}

/// The app menu: what Rerun keeps under its logo.
pub(crate) fn app_menu_rows() -> Vec<MenuRow> {
    vec![
        MenuRow::new(live_id!(shortcuts), "Keyboard shortcuts"),
        MenuRow::new(live_id!(preferences), "Preferences…"),
        MenuRow::separator(),
        MenuRow::new(live_id!(report_bug), "Report a bug…"),
        MenuRow::separator(),
        MenuRow::section(&format!("Reco {}", version_line())),
    ]
}

/// Set: links are logged, not opened (checks).
const NO_BROWSER: &str = "RECO_DESKTOP_NO_BROWSER";

impl App {
    /// The app menu's commands.
    pub(crate) fn app_menu_actions(&mut self, cx: &mut Cx, actions: &Actions) {
        let owner = self.ui.menu_button(cx, ids!(app_menu)).menu_owner();
        match menu_picked(actions, owner) {
            Some(id) if id == live_id!(preferences) => self.open_preferences(cx),
            Some(id) if id == live_id!(shortcuts) => self.open_shortcuts(cx),
            Some(id) if id == live_id!(report_bug) => self.open_bug_report(cx),
            Some(id) => log!("app menu: {id} (not wired yet)"),
            None => {}
        }
        if self.ui.button(cx, ids!(shortcuts_website)).clicked(actions) {
            self.open_link(cx, help::WEBSITE);
        }
        if self.ui.button(cx, ids!(shortcuts_forum)).clicked(actions) {
            self.open_link(cx, help::FORUM);
        }
        if self.ui.button(cx, ids!(shortcuts_close)).clicked(actions) {
            self.ui.modal(cx, ids!(shortcuts_sheet)).close(cx);
        }
    }

    fn open_shortcuts(&mut self, cx: &mut Cx) {
        if let Some(mut table) = self
            .ui
            .widget(cx, ids!(shortcuts_table))
            .borrow_mut::<RecoKeyTable>()
        {
            table.set_rows(cx, shortcut_rows(cfg!(target_os = "macos")));
        }
        self.ui.modal(cx, ids!(shortcuts_sheet)).open(cx);
    }

    /// Open `url` (a link or a page) in the browser, logged instead under
    /// `NO_BROWSER`; false when the browser couldn't start (a notice says
    /// so).
    pub(crate) fn open_link(&mut self, cx: &mut Cx, url: &str) -> bool {
        if std::env::var_os(NO_BROWSER).is_some() {
            log!("browser: would open {url}");
            return true;
        }
        if let Err(e) = open_in_browser(url) {
            self.toast(
                cx,
                Severity::Error,
                "Couldn't open the browser",
                &e.to_string(),
            );
            return false;
        }
        true
    }
}

/// The shortcuts sheet's rows: (keys, what they do). The menus' keys only
/// where the app has a menu bar (macOS).
fn shortcut_rows(with_menus: bool) -> Vec<(String, String)> {
    SHORTCUTS
        .iter()
        .filter(|s| with_menus || !s.menu)
        .map(|s| (s.keys.to_string(), s.does.to_string()))
        .collect()
}

fn version_text(version: &str, commit: Option<&str>) -> String {
    match commit.filter(|c| !c.is_empty()) {
        Some(commit) => format!("{version} ({commit})"),
        None => version.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sheet_lists_the_menu_keys_only_with_the_menu_bar() {
        let with = shortcut_rows(true);
        let without = shortcut_rows(false);
        assert_eq!(
            with.first(),
            Some(&("Space".to_string(), "Play or pause".to_string()))
        );
        assert!(with.iter().any(|(keys, _)| keys == "⌘,"));
        assert!(!without.iter().any(|(keys, _)| keys.starts_with('⌘')));
        assert_eq!(with.len(), SHORTCUTS.len());
    }

    #[test]
    fn the_version_names_its_commit() {
        assert_eq!(version_text("0.5.4", Some("2fba497")), "0.5.4 (2fba497)");
        assert_eq!(version_text("0.5.4", None), "0.5.4");
        assert_eq!(version_text("0.5.4", Some("")), "0.5.4");
    }
}
