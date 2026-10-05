//! The app's network uses: usage data (opt-in in Preferences), a bug report
//! sent with it, and the update check; and the one clipboard write (Copy
//! report). Checks switch them off: `RECO_DESKTOP_NO_NETWORK` turns each
//! request into a log line and `RECO_DESKTOP_NO_CLIPBOARD` each copy; when a
//! switch names a folder, what would have gone out is kept there for the
//! check to read.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::SystemTime;

use makepad_widgets::*;
use reco_app::telemetry::{self, batch_json, UsageEvent};
use reco_app::toasts::Severity;

use crate::App;

/// Set: no request leaves the app; each is logged instead.
const NO_NETWORK: &str = "RECO_DESKTOP_NO_NETWORK";
/// Set: nothing is copied to the clipboard; each copy is logged instead.
const NO_CLIPBOARD: &str = "RECO_DESKTOP_NO_CLIPBOARD";

/// `None` when the switch `var` is unset; else the folder it names, if it
/// names one.
fn switched(var: &str) -> Option<Option<PathBuf>> {
    let value = PathBuf::from(std::env::var_os(var)?);
    Some(value.is_dir().then_some(value))
}

/// Keep `body` in `folder` (a switch's) as `<name>-<n>.txt`.
fn keep(folder: Option<&Path>, name: &str, body: &str) {
    static KEPT: AtomicU64 = AtomicU64::new(0);
    if let Some(folder) = folder {
        let n = KEPT.fetch_add(1, Ordering::Relaxed);
        if let Err(e) = std::fs::write(folder.join(format!("{name}-{n}.txt")), body) {
            log!("couldn't keep {name}: {e}");
        }
    }
}

/// "macos aarch64".
pub(crate) fn os_line() -> String {
    format!("{} {}", std::env::consts::OS, std::env::consts::ARCH)
}

/// The AI tracking's state, as the Slint app reported it (tracking joins
/// this app with Module 6b).
const AI_STATE: &str = "AI: disabled (build without autocam feature)";

impl App {
    /// Send `event` when usage data is on.
    pub(crate) fn send_usage(&mut self, cx: &mut Cx, event: UsageEvent) {
        if self.settings.telemetry_enabled {
            self.post_usage(cx, event);
        }
    }

    /// The system's context, once a run, when usage data is on and the
    /// preview has named its GPU.
    pub(crate) fn send_context(&mut self, cx: &mut Cx) {
        if self.context_sent || !self.settings.telemetry_enabled {
            return;
        }
        let Some(gpu) = self.gpu_name.clone() else {
            return;
        };
        self.context_sent = true;
        self.send_usage(
            cx,
            UsageEvent::Context {
                os: os_line(),
                gpu,
                ai: AI_STATE.into(),
            },
        );
    }

    /// What a newly opened match is (sent with its first values, when the
    /// sync offset is known).
    pub(crate) fn send_source_info(&mut self, cx: &mut Cx, sync_offset: i64) {
        let Some(info) = self.live.as_ref().and_then(|l| l.info.clone()) else {
            return;
        };
        // How frames reach the picture (the preview doesn't say which
        // decoder it got).
        let decoder = if info.zero_copy {
            "zero-copy"
        } else {
            "readback"
        };
        self.send_usage(
            cx,
            UsageEvent::SourceInfo {
                width: info.width,
                height: info.height,
                fps: info.fps,
                decoder: decoder.into(),
                sync_offset,
            },
        );
    }

    /// Post `event` to the usage service (whatever the opt-in says: the
    /// caller decides).
    pub(crate) fn post_usage(&mut self, cx: &mut Cx, event: UsageEvent) {
        let had_id = self.settings.telemetry_client_id.is_some();
        let client = self.settings.client_id();
        if !had_id {
            self.save_settings();
        }
        let body = batch_json(
            &event,
            &client,
            env!("CARGO_PKG_VERSION"),
            SystemTime::now(),
            &telemetry::batch_id(),
        );
        if let Some(folder) = switched(NO_NETWORK) {
            log!(
                "network: would send {} ({} bytes)",
                event.name(),
                body.len()
            );
            keep(folder.as_deref(), event.name(), &body);
            self.usage_answered(cx, event.name(), Ok(()));
            return;
        }
        let mut request = HttpRequest::new(telemetry::ENDPOINT.into(), HttpMethod::POST);
        request.set_header("Content-Type".into(), "application/json".into());
        request.set_body(body.into_bytes());
        let id = LiveId::unique();
        self.usage_requests.insert(id, event.name());
        cx.http_request(id, request);
    }

    /// The service's answer to a usage event: a bug report says how it
    /// went; other events only log a failure.
    fn usage_answered(&mut self, cx: &mut Cx, name: &str, result: Result<(), String>) {
        match (name, result) {
            ("bug_report", Ok(())) => self.toast(
                cx,
                Severity::Info,
                "Report sent",
                "Thank you. It went to Reco's developers.",
            ),
            ("bug_report", Err(why)) => {
                log!("network: bug_report not sent ({why})");
                self.toast(
                    cx,
                    Severity::Error,
                    "The report wasn't sent",
                    "Check the connection, or copy the report and post it on the forum.",
                );
            }
            (_, Ok(())) => {}
            (name, Err(why)) => log!("network: {name} not sent ({why})"),
        }
    }

    /// Put `text` on the clipboard (logged, and kept in the switch's folder,
    /// under `NO_CLIPBOARD`).
    pub(crate) fn copy_text(&mut self, cx: &mut Cx, name: &str, text: &str) {
        if let Some(folder) = switched(NO_CLIPBOARD) {
            log!("clipboard: would copy {name} ({} bytes)", text.len());
            keep(folder.as_deref(), name, text);
            return;
        }
        cx.copy_to_clipboard(text);
    }

    /// Answers to the app's requests.
    pub(crate) fn network_responses(&mut self, cx: &mut Cx, responses: &[NetworkResponse]) {
        for response in responses {
            match response {
                NetworkResponse::HttpResponse {
                    request_id,
                    response,
                } => {
                    if let Some(name) = self.usage_requests.remove(request_id) {
                        let result = if (200..300).contains(&response.status_code) {
                            Ok(())
                        } else {
                            Err(format!("HTTP {}", response.status_code))
                        };
                        self.usage_answered(cx, name, result);
                    }
                }
                NetworkResponse::HttpError { request_id, error } => {
                    if let Some(name) = self.usage_requests.remove(request_id) {
                        self.usage_answered(cx, name, Err(error.message.clone()));
                    }
                }
                _ => {}
            }
        }
    }
}
