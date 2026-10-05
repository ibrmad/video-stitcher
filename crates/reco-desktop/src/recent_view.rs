//! Recent sessions and dropped files. The Recent menu (the Setup panel's
//! clock, and "Recent files…" on the next-step card) lists the last camera
//! pairs opened; one click restores both cameras' files and the
//! calibration. Files dropped on the window go where `Project::route_drop`
//! sends them.

use std::path::PathBuf;

use makepad_widgets::*;
use reco_app::project::{Camera, Stage};
use reco_app::settings::RecentSession;
use reco_app::toasts::Severity;

use crate::ui::menu_list::MenuEntry;
use crate::App;

/// A recent session's menu row id.
fn recent_id(index: usize) -> LiveId {
    LiveId::from_str_num("recent", index as u64)
}

impl App {
    /// The Recent menu's rows: each session, then Clear.
    pub(crate) fn recent_entries(&self) -> Vec<MenuEntry> {
        if self.settings.recent.is_empty() {
            return vec![MenuEntry::Section("No recent files".into())];
        }
        let mut rows: Vec<MenuEntry> = self
            .settings
            .recent
            .iter()
            .enumerate()
            .map(|(index, session)| MenuEntry::Item(recent_id(index), session.label()))
            .collect();
        rows.push(MenuEntry::Separator);
        rows.push(MenuEntry::Item(
            live_id!(clear_recent),
            "Clear recent files".into(),
        ));
        rows
    }

    pub(crate) fn refresh_recent(&mut self, cx: &mut Cx) {
        let entries = self.recent_entries();
        self.set_menu(cx, ids!(recent_menu_list), entries);
    }

    /// The preview opened: remember this pair and its calibration.
    pub(crate) fn remember_session(&mut self, cx: &mut Cx) {
        let Some(calibration) = self.project.calibration.clone() else {
            return;
        };
        self.settings.push_recent(RecentSession {
            left: self.project.files(Camera::Left).to_vec(),
            right: self.project.files(Camera::Right).to_vec(),
            calibration: Some(calibration),
        });
        self.save_settings();
        self.refresh_recent(cx);
    }

    /// Open the Recent menu below `anchor` (the next-step card's button).
    fn open_recent_menu(&mut self, cx: &mut Cx, anchor: Rect) {
        self.refresh_recent(cx);
        self.ui.popover(cx, ids!(recent_menu)).open_at(cx, anchor);
    }

    /// The Recent menu's picks and the next-step card's "Recent files…".
    pub(crate) fn recent_actions(&mut self, cx: &mut Cx, actions: &Actions) {
        if let Some(picked) = self.menu_pick(cx, actions, ids!(recent_menu), ids!(recent_menu_list))
        {
            if picked == live_id!(clear_recent) {
                self.settings.clear_recent();
                self.save_settings();
                self.refresh_recent(cx);
            } else if let Some(session) = (0..self.settings.recent.len())
                .find(|i| recent_id(*i) == picked)
                .map(|i| self.settings.recent[i].clone())
            {
                self.restore(cx, session);
            }
        }
        let opening = matches!(self.project.stage(), Stage::NoVideos | Stage::OneCamera(_));
        let secondary = self.ui.button(cx, ids!(next_secondary));
        if opening && secondary.clicked(actions) {
            let anchor = secondary.area().rect(cx);
            self.open_recent_menu(cx, anchor);
        }
    }

    /// Open a recent session: both cameras' files and the calibration.
    fn restore(&mut self, cx: &mut Cx, session: RecentSession) {
        self.project.set_files(Camera::Left, session.left);
        self.project.set_files(Camera::Right, session.right);
        self.project.calibration = session.calibration;
        self.calibration_failure = None;
        self.project_changed(cx);
    }

    /// Files dragged over the window: offer to copy them in.
    pub(crate) fn drag_files(&mut self, event: &DragEvent) {
        let files = event
            .items
            .iter()
            .any(|item| matches!(item, DragItem::FilePath { .. }));
        if files {
            if let Ok(mut response) = event.response.lock() {
                *response = DragResponse::Copy;
            }
        }
    }

    /// Files dropped on the window: videos to the camera row they landed on
    /// (else the first camera without videos), a JSON as the calibration.
    pub(crate) fn drop_files(&mut self, cx: &mut Cx, event: &DropEvent) {
        let paths: Vec<PathBuf> = event
            .items
            .iter()
            .filter_map(|item| match item {
                DragItem::FilePath { path, .. } => Some(PathBuf::from(path)),
                _ => None,
            })
            .collect();
        if paths.is_empty() {
            return;
        }
        let over = [
            (ids!(left_camera), Camera::Left),
            (ids!(right_camera), Camera::Right),
        ]
        .into_iter()
        .find(|(id, _)| self.ui.widget(cx, *id).area().rect(cx).contains(event.abs))
        .map(|(_, camera)| camera);
        let plan = self.project.route_drop(&paths, over);
        if let Some((camera, videos)) = plan.videos {
            self.project.add(camera, videos);
        }
        if let Some(calibration) = plan.calibration {
            self.project.calibration = Some(calibration);
            self.calibration_failure = None;
        }
        if plan.refused > 0 {
            self.toast(
                cx,
                Severity::Info,
                "Drop videos onto a camera",
                "Both cameras have videos: drop more onto the Left or Right camera row.",
            );
        }
        self.project_changed(cx);
    }
}
