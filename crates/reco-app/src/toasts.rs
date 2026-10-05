//! Toasts: short notices in the viewer's corner (PARITY.md Module 2). Info
//! stays 4 s, a warning 7 s, an error 10 s, or a custom time. At most four
//! show, the oldest leaving first and the newest at the bottom. A notice
//! identical to one on screen refreshes it instead of stacking. Pure: the
//! caller passes the time.

use std::time::{Duration, Instant};

/// How serious a toast is: its dot colour and its default time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Severity {
    /// Something happened.
    Info,
    /// Worth a look.
    Warn,
    /// Something failed.
    Error,
}

impl Severity {
    /// How long a toast of this severity stays.
    pub fn ttl(self) -> Duration {
        Duration::from_secs(match self {
            Self::Info => 4,
            Self::Warn => 7,
            Self::Error => 10,
        })
    }
}

/// One toast.
#[derive(Clone, Debug, PartialEq)]
pub struct Toast {
    /// Its id, for dismissing it.
    pub id: u64,
    /// How serious it is.
    pub severity: Severity,
    /// One line.
    pub title: String,
    /// The detail; may be empty.
    pub body: String,
    /// When it leaves.
    pub expires_at: Instant,
}

/// Toasts on screen at most.
pub const MAX_VISIBLE: usize = 4;

/// The toasts on screen, oldest first.
#[derive(Clone, Debug, Default)]
pub struct Toasts {
    shown: Vec<Toast>,
    next_id: u64,
}

impl Toasts {
    /// Show a toast for its severity's time; its id.
    pub fn push(
        &mut self,
        severity: Severity,
        title: impl Into<String>,
        body: impl Into<String>,
        now: Instant,
    ) -> u64 {
        self.push_for(severity, title, body, severity.ttl(), now)
    }

    /// Show a toast for `ttl`; its id. An identical toast on screen is
    /// refreshed and moved to the end instead.
    pub fn push_for(
        &mut self,
        severity: Severity,
        title: impl Into<String>,
        body: impl Into<String>,
        ttl: Duration,
        now: Instant,
    ) -> u64 {
        let (title, body) = (title.into(), body.into());
        let same = |t: &Toast| t.severity == severity && t.title == title && t.body == body;
        if let Some(index) = self.shown.iter().position(same) {
            let mut toast = self.shown.remove(index);
            toast.expires_at = now + ttl;
            let id = toast.id;
            self.shown.push(toast);
            return id;
        }
        self.next_id += 1;
        let id = self.next_id;
        self.shown.push(Toast {
            id,
            severity,
            title,
            body,
            expires_at: now + ttl,
        });
        if self.shown.len() > MAX_VISIBLE {
            self.shown.remove(0);
        }
        id
    }

    /// Remove the toast `id`; whether it was there.
    pub fn dismiss(&mut self, id: u64) -> bool {
        let before = self.shown.len();
        self.shown.retain(|t| t.id != id);
        self.shown.len() != before
    }

    /// Remove the toasts due by `now`; whether any left.
    pub fn expire(&mut self, now: Instant) -> bool {
        let before = self.shown.len();
        self.shown.retain(|t| t.expires_at > now);
        self.shown.len() != before
    }

    /// The toasts on screen, oldest first.
    pub fn visible(&self) -> &[Toast] {
        &self.shown
    }

    /// When the next toast leaves.
    pub fn next_expiry(&self) -> Option<Instant> {
        self.shown.iter().map(|t| t.expires_at).min()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn titles(t: &Toasts) -> Vec<&str> {
        t.visible().iter().map(|t| t.title.as_str()).collect()
    }

    #[test]
    fn ttls_follow_the_severity() {
        assert_eq!(Severity::Info.ttl(), Duration::from_secs(4));
        assert_eq!(Severity::Warn.ttl(), Duration::from_secs(7));
        assert_eq!(Severity::Error.ttl(), Duration::from_secs(10));
    }

    #[test]
    fn at_most_four_newest_last() {
        let now = Instant::now();
        let mut t = Toasts::default();
        for title in ["a", "b", "c", "d", "e"] {
            t.push(Severity::Info, title, "", now);
        }
        assert_eq!(titles(&t), ["b", "c", "d", "e"]);
    }

    #[test]
    fn expire_removes_only_the_due() {
        let now = Instant::now();
        let mut t = Toasts::default();
        t.push(Severity::Info, "info", "", now);
        t.push(Severity::Error, "error", "", now);
        assert!(!t.expire(now + Duration::from_secs(3)));
        assert!(t.expire(now + Duration::from_secs(5)));
        assert_eq!(titles(&t), ["error"]);
        assert_eq!(t.next_expiry(), Some(now + Duration::from_secs(10)));
    }

    #[test]
    fn dismiss_removes_one() {
        let now = Instant::now();
        let mut t = Toasts::default();
        let a = t.push(Severity::Info, "a", "", now);
        t.push(Severity::Info, "b", "", now);
        assert!(t.dismiss(a));
        assert!(!t.dismiss(a));
        assert_eq!(titles(&t), ["b"]);
    }

    #[test]
    fn a_repeat_refreshes_instead_of_stacking() {
        let now = Instant::now();
        let mut t = Toasts::default();
        let first = t.push(Severity::Error, "Couldn't seek", "x", now);
        t.push(Severity::Info, "other", "", now);
        let later = now + Duration::from_secs(5);
        let again = t.push(Severity::Error, "Couldn't seek", "x", later);
        assert_eq!(again, first);
        assert_eq!(titles(&t), ["other", "Couldn't seek"]);
        assert_eq!(t.visible()[1].expires_at, later + Duration::from_secs(10));
    }

    #[test]
    fn custom_times_are_kept() {
        let now = Instant::now();
        let mut t = Toasts::default();
        t.push_for(
            Severity::Info,
            "Recording saved",
            "",
            Duration::from_secs(8),
            now,
        );
        assert_eq!(t.next_expiry(), Some(now + Duration::from_secs(8)));
    }
}
