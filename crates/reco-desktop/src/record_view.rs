//! The view bar's record menu: the recording quality (a ✓ on the one in
//! use) and the way to the codec and folder (Preferences).

use makepad_widgets::*;
use reco_app::recording::RecordingQuality;

use crate::cli::LookPreview;
use crate::ui::menu_list::MenuEntry;
use crate::App;

/// The qualities, in the menu's order, with their labels.
const QUALITIES: [(RecordingQuality, &str); 3] = [
    (RecordingQuality::Fast, "Fast"),
    (RecordingQuality::Balanced, "Balanced"),
    (RecordingQuality::High, "High"),
];

/// A quality's row id.
fn quality_id(quality: RecordingQuality) -> LiveId {
    match quality {
        RecordingQuality::Fast => live_id!(fast),
        RecordingQuality::Balanced => live_id!(balanced),
        RecordingQuality::High => live_id!(high),
    }
}

/// The record menu's rows, the quality in use marked.
pub(crate) fn record_menu_entries(current: RecordingQuality) -> Vec<MenuEntry> {
    let mut entries = vec![MenuEntry::Section("Quality".into())];
    for (quality, label) in QUALITIES {
        entries.push(MenuEntry::Choice(
            quality_id(quality),
            label.into(),
            quality == current,
        ));
    }
    entries.push(MenuEntry::Separator);
    entries.push(MenuEntry::Item(
        live_id!(codec_and_folder),
        "Codec and folder…".into(),
    ));
    entries
}

/// Whether the record menu takes input: there is something to record,
/// and neither an export nor a recording is running (a recording's quality
/// can't change; an export stops a recording, and its answer arrives
/// during the export).
pub(crate) fn menu_on(loaded: bool, exporting: bool, recording: bool) -> bool {
    loaded && !exporting && !recording
}

/// The quality a picked row names, if it names one.
pub(crate) fn quality_of(id: LiveId) -> Option<RecordingQuality> {
    QUALITIES
        .into_iter()
        .map(|(quality, _)| quality)
        .find(|quality| quality_id(*quality) == id)
}

impl App {
    /// The record menu on or off, by `menu_on`.
    pub(crate) fn show_record_menu_enabled(&mut self, cx: &mut Cx) {
        // The look preview's exporting state counts, as for Record itself.
        let exporting = self.preview == Some(LookPreview::Exporting) || self.exporting();
        let on = menu_on(
            self.shell.files_loaded(),
            exporting,
            self.live.as_ref().is_some_and(|l| l.recording.is_some()),
        );
        self.set_button_enabled(cx, ids!(record_menu_button), on);
    }

    /// The record menu's rows, its ✓ on the quality in use.
    pub(crate) fn show_record_menu(&mut self, cx: &mut Cx) {
        let entries = record_menu_entries(self.settings.quality());
        self.set_menu(cx, ids!(record_menu_list), entries);
    }

    /// A quality picked in the record menu (the next recordings'), or
    /// Codec and folder… (Preferences).
    pub(crate) fn record_menu_actions(&mut self, cx: &mut Cx, actions: &Actions) {
        let Some(picked) = self.menu_pick(cx, actions, ids!(record_menu), ids!(record_menu_list))
        else {
            return;
        };
        if let Some(quality) = quality_of(picked) {
            self.settings.set_quality(quality);
            self.save_settings();
            log!("recording quality: {}", quality.name());
            self.show_record_menu(cx);
            // The menu had the keyboard: give it back to the preview's keys.
            cx.set_key_focus(Area::Empty);
        } else if picked == live_id!(codec_and_folder) {
            self.open_preferences(cx);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_record_menu_marks_the_quality_in_use() {
        assert_eq!(
            record_menu_entries(RecordingQuality::High),
            vec![
                MenuEntry::Section("Quality".into()),
                MenuEntry::Choice(live_id!(fast), "Fast".into(), false),
                MenuEntry::Choice(live_id!(balanced), "Balanced".into(), false),
                MenuEntry::Choice(live_id!(high), "High".into(), true),
                MenuEntry::Separator,
                MenuEntry::Item(live_id!(codec_and_folder), "Codec and folder…".into()),
            ]
        );
    }

    #[test]
    fn the_menu_is_on_only_while_it_can_change_something() {
        assert!(menu_on(true, false, false));
        assert!(!menu_on(false, false, false), "no videos");
        assert!(!menu_on(true, true, false), "exporting");
        assert!(!menu_on(true, false, true), "recording");
    }

    #[test]
    fn a_picked_row_names_its_quality() {
        assert_eq!(quality_of(live_id!(fast)), Some(RecordingQuality::Fast));
        assert_eq!(quality_of(live_id!(high)), Some(RecordingQuality::High));
        assert_eq!(quality_of(live_id!(codec_and_folder)), None);
    }
}
