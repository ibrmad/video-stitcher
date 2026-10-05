//! The app menu in the App: Preferences, Keyboard shortcuts, Report a bug,
//! and this build's version.

use makepad_widgets::*;

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

impl App {
    /// The app menu's commands.
    pub(crate) fn app_menu_actions(&mut self, cx: &mut Cx, actions: &Actions) {
        let owner = self.ui.menu_button(cx, ids!(app_menu)).menu_owner();
        match menu_picked(actions, owner) {
            Some(id) if id == live_id!(preferences) => self.open_preferences(cx),
            Some(id) => log!("app menu: {id} (not wired yet)"),
            None => {}
        }
    }
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
    fn the_version_names_its_commit() {
        assert_eq!(version_text("0.5.4", Some("2fba497")), "0.5.4 (2fba497)");
        assert_eq!(version_text("0.5.4", None), "0.5.4");
        assert_eq!(version_text("0.5.4", Some("")), "0.5.4");
    }
}
