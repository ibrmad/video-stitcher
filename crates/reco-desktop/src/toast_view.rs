//! Toasts in the App: notices pushed from anywhere, shown in the viewer's
//! four card slots, expired by one timer, dismissed by their close buttons.
//! They never touch the status line.

use std::time::{Duration, Instant};

use makepad_widgets::*;
use reco_app::toasts::Severity;

use crate::App;

/// The card slots, oldest first.
fn slots() -> [LiveId; 4] {
    [
        live_id!(toast_0),
        live_id!(toast_1),
        live_id!(toast_2),
        live_id!(toast_3),
    ]
}

impl App {
    /// Show a notice for its severity's time.
    pub(crate) fn toast(&mut self, cx: &mut Cx, severity: Severity, title: &str, body: &str) {
        self.toasts.push(severity, title, body, Instant::now());
        self.show_toasts(cx);
    }

    /// Show a notice for `ttl`.
    pub(crate) fn toast_for(
        &mut self,
        cx: &mut Cx,
        severity: Severity,
        title: &str,
        body: &str,
        ttl: Duration,
    ) {
        self.toasts
            .push_for(severity, title, body, ttl, Instant::now());
        self.show_toasts(cx);
    }

    /// Fill the slots from the model; arm the timer for the next expiry.
    fn show_toasts(&mut self, cx: &mut Cx) {
        let shown = self.toasts.visible().to_vec();
        for (index, slot) in slots().into_iter().enumerate() {
            let toast = shown.get(index);
            self.ui.widget(cx, &[slot]).set_visible(cx, toast.is_some());
            let Some(toast) = toast else { continue };
            self.ui
                .label(cx, &[slot, live_id!(title)])
                .set_text(cx, &toast.title);
            self.ui
                .label(cx, &[slot, live_id!(body)])
                .set_text(cx, &toast.body);
            self.ui
                .widget(cx, &[slot, live_id!(body_row)])
                .set_visible(cx, !toast.body.is_empty());
            for (dot, severity) in [
                (live_id!(dot_info), Severity::Info),
                (live_id!(dot_warn), Severity::Warn),
                (live_id!(dot_error), Severity::Error),
            ] {
                self.ui
                    .widget(cx, &[slot, dot])
                    .set_visible(cx, toast.severity == severity);
            }
        }
        cx.stop_timer(self.toast_timer);
        self.toast_timer = match self.toasts.next_expiry() {
            Some(at) => cx.start_timeout(
                at.saturating_duration_since(Instant::now())
                    .as_secs_f64()
                    .max(0.05),
            ),
            None => Timer::empty(),
        };
        self.ui.widget(cx, ids!(toasts)).redraw(cx);
    }

    /// The timer fired: drop the toasts that are due.
    pub(crate) fn expire_toasts(&mut self, cx: &mut Cx) {
        self.toasts.expire(Instant::now());
        self.show_toasts(cx);
    }

    /// A close button was clicked: dismiss its toast.
    pub(crate) fn toast_actions(&mut self, cx: &mut Cx, actions: &Actions) {
        let ids: Vec<u64> = self.toasts.visible().iter().map(|t| t.id).collect();
        let closed: Vec<u64> = slots()
            .into_iter()
            .zip(ids)
            .filter(|(slot, _)| {
                self.ui
                    .button(cx, &[*slot, live_id!(close)])
                    .clicked(actions)
            })
            .map(|(_, id)| id)
            .collect();
        if closed.is_empty() {
            return;
        }
        for id in closed {
            self.toasts.dismiss(id);
        }
        self.show_toasts(cx);
    }

    /// Sample notices (`--toast-demo`): five pushed, so the oldest has left.
    pub(crate) fn toast_demo(&mut self, cx: &mut Cx) {
        for (severity, title, body) in [
            (Severity::Info, "Calibration saved", ""),
            (
                Severity::Info,
                "Recording started",
                "~/Movies/reco_recording_1700000000.mp4",
            ),
            (
                Severity::Warn,
                "Low calibration confidence",
                "The cameras matched on few points; check the seam.",
            ),
            (
                Severity::Error,
                "Couldn't open the videos",
                "Invalid input path (/nonexistent/left.mp4): file not found",
            ),
            (
                Severity::Info,
                "Recording saved",
                "905 frames · reco_recording_1700000000.mp4",
            ),
        ] {
            self.toast(cx, severity, title, body);
        }
    }
}
