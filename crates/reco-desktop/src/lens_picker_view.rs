//! The lens picker in the App: the search on a short thread (the newest
//! query wins), its results, Apply to, and a profile file. A pick tunes the
//! chosen cameras' lens, names it, and centres the fine-tune sliders on it,
//! as the Slint app did.

use std::path::PathBuf;
use std::sync::Arc;

use makepad_widgets::makepad_platform::thread::SignalToUI;
use makepad_widgets::*;
use reco_app::lens::{
    file_lens, profile_lens, profile_line, Cameras, Lens, LensInfo, LensProfileSummary, LensSource,
};
use reco_app::preview::tuning::Tuning;
use reco_app::preview::worker::PreviewCommand;
use reco_app::toasts::Severity;

use crate::project_view::Pick;
use crate::ui::pick_list::{PickListAction, RecoPickList};
use crate::App;

/// The search's word under the list, for `found` results of `query`.
pub(crate) fn search_hint(query: &str, found: usize) -> String {
    match found {
        _ if query.trim().is_empty() => "Type to search over 4,200 camera profiles.".into(),
        0 => "No profiles match.".into(),
        1 => "1 profile".into(),
        // The database answers at most a hundred.
        100.. => "The first 100 matches; add words to narrow them.".into(),
        n => format!("{n} profiles"),
    }
}

/// The Apply to choice, as the dropdown lists it.
fn cameras_at(index: usize) -> Cameras {
    match index {
        1 => Cameras::Left,
        2 => Cameras::Right,
        _ => Cameras::Both,
    }
}

impl App {
    fn open_lens_picker(&mut self, cx: &mut Cx) {
        let query = self.ui.text_input(cx, ids!(picker_search)).text();
        self.search_profiles(cx, &query);
        self.ui.modal(cx, ids!(lens_picker)).open(cx);
    }

    /// Search for `query` (nothing typed: nothing listed).
    fn search_profiles(&mut self, cx: &mut Cx, query: &str) {
        if query.trim().is_empty() {
            self.show_profiles(cx, Vec::new());
            return;
        }
        let size = self.latest_values.as_ref().map_or((0, 0), |v| v.lens_size);
        self.profile_search
            .search(query, size, Arc::new(SignalToUI::set_ui_signal));
    }

    /// The newest search's results, once in.
    pub(crate) fn collect_profiles(&mut self, cx: &mut Cx) {
        if let Some(found) = self.profile_search.try_results() {
            self.show_profiles(cx, found);
        }
    }

    fn show_profiles(&mut self, cx: &mut Cx, found: Vec<LensProfileSummary>) {
        let query = self.ui.text_input(cx, ids!(picker_search)).text();
        self.set_label(cx, ids!(picker_hint), &search_hint(&query, found.len()));
        if let Some(mut list) = self
            .ui
            .widget(cx, ids!(picker_results))
            .borrow_mut::<RecoPickList>()
        {
            list.set_rows(cx, found.iter().map(profile_line).collect());
        }
        self.profiles = found;
    }

    /// The Lens section's profiles button and the picker's controls.
    pub(crate) fn lens_picker_actions(&mut self, cx: &mut Cx, actions: &Actions) {
        if self.ui.button(cx, ids!(lens_browse)).clicked(actions) {
            self.open_lens_picker(cx);
        }
        if let Some(query) = self.ui.text_input(cx, ids!(picker_search)).changed(actions) {
            self.search_profiles(cx, &query);
        }
        let uid = self.ui.widget(cx, ids!(picker_results)).widget_uid();
        for action in actions.filter_widget_actions_cast::<PickListAction>(uid) {
            if let PickListAction::Picked(row) = action {
                self.apply_profile(cx, row);
            }
        }
        if self.ui.button(cx, ids!(picker_file)).clicked(actions) {
            self.pick(cx, Pick::LensFile);
        }
        if self.ui.button(cx, ids!(picker_close)).clicked(actions) {
            self.ui.modal(cx, ids!(lens_picker)).close(cx);
        }
    }

    fn apply_profile(&mut self, cx: &mut Cx, row: usize) {
        let Some(profile) = self.profiles.get(row).cloned() else {
            return;
        };
        let Some(size) = self.latest_values.as_ref().map(|v| v.lens_size) else {
            return;
        };
        match profile_lens(&profile, size) {
            Some(lens) => self.apply_lens(
                cx,
                lens,
                LensInfo {
                    camera: profile.camera,
                    lens: profile.lens,
                    source: LensSource::Picker,
                },
            ),
            None => self.toast(
                cx,
                Severity::Error,
                "Couldn't load that profile",
                &profile_line(&profile),
            ),
        }
    }

    /// The lens profile file the dialog chose.
    pub(crate) fn lens_file_picked(&mut self, cx: &mut Cx, path: PathBuf) {
        let Some(size) = self.latest_values.as_ref().map(|v| v.lens_size) else {
            return;
        };
        match file_lens(&path, size) {
            Ok(lens) => {
                let name = path
                    .file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_default();
                self.apply_lens(
                    cx,
                    lens,
                    LensInfo {
                        camera: name,
                        lens: String::new(),
                        source: LensSource::File,
                    },
                );
            }
            Err(why) => self.toast(cx, Severity::Error, "Couldn't load the lens profile", &why),
        }
    }

    /// Tune the chosen cameras to `lens`, name it, centre fine-tune on it,
    /// and close the picker.
    fn apply_lens(&mut self, cx: &mut Cx, lens: Lens, info: LensInfo) {
        let cameras = cameras_at(self.ui.drop_down(cx, ids!(picker_cameras)).selected_item());
        log!("lens: {} for {cameras:?}", info.name());
        self.send_preview(PreviewCommand::Tune(Tuning::Lens { cameras, lens }));
        let (mut left, mut right) = self.lens_names.clone();
        if cameras.left() {
            left = Some(info.clone());
        }
        if cameras.right() {
            right = Some(info.clone());
        }
        self.show_lens_names(cx, left, right);
        if let Some(mut values) = self.latest_values.clone() {
            if cameras.left() {
                values.left_lens = lens;
            }
            if cameras.right() {
                values.right_lens = lens;
            }
            self.lens_base = lens;
            self.show_fine_tune(cx, &values);
        }
        self.ui.modal(cx, ids!(lens_picker)).close(cx);
        self.toast(cx, Severity::Info, "Lens profile applied", &info.name());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_hint_says_what_the_list_holds() {
        assert_eq!(
            search_hint("", 0),
            "Type to search over 4,200 camera profiles."
        );
        assert_eq!(search_hint("zzz", 0), "No profiles match.");
        assert_eq!(search_hint("hero9", 1), "1 profile");
        assert_eq!(search_hint("hero9", 12), "12 profiles");
        assert_eq!(
            search_hint("gopro", 100),
            "The first 100 matches; add words to narrow them."
        );
    }

    #[test]
    fn apply_to_reads_both_left_right() {
        assert_eq!(cameras_at(0), Cameras::Both);
        assert_eq!(cameras_at(1), Cameras::Left);
        assert_eq!(cameras_at(2), Cameras::Right);
    }
}
