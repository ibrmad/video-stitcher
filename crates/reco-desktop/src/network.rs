//! The app's network uses: usage data (opt-in in Preferences), a bug report
//! sent with it, and the update check. `RECO_DESKTOP_NO_NETWORK` turns each
//! request into a log line, so checks never reach the network.

use std::time::SystemTime;

use makepad_widgets::*;
use reco_app::telemetry::{self, batch_json, UsageEvent};

use crate::App;

/// Set: no request leaves the app; each is logged instead.
const NO_NETWORK: &str = "RECO_DESKTOP_NO_NETWORK";

/// Whether requests stay in the app (checks).
pub(crate) fn offline() -> bool {
    std::env::var_os(NO_NETWORK).is_some()
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
        if offline() {
            log!(
                "network: would send {} ({} bytes)",
                event.name(),
                body.len()
            );
            return;
        }
        let mut request = HttpRequest::new(telemetry::ENDPOINT.into(), HttpMethod::POST);
        request.set_header("Content-Type".into(), "application/json".into());
        request.set_body(body.into_bytes());
        let id = LiveId::unique();
        self.usage_requests.insert(id, event.name());
        cx.http_request(id, request);
    }

    /// Answers to the app's requests.
    pub(crate) fn network_responses(&mut self, _cx: &mut Cx, responses: &[NetworkResponse]) {
        for response in responses {
            match response {
                NetworkResponse::HttpResponse {
                    request_id,
                    response,
                } => {
                    if let Some(name) = self.usage_requests.remove(request_id) {
                        if !(200..300).contains(&response.status_code) {
                            log!("network: {name} refused (HTTP {})", response.status_code);
                        }
                    }
                }
                NetworkResponse::HttpError { request_id, error } => {
                    if let Some(name) = self.usage_requests.remove(request_id) {
                        log!("network: {name} not sent ({})", error.message);
                    }
                }
                _ => {}
            }
        }
    }
}
