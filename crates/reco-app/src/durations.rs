//! Each video file's length, measured off the UI thread and remembered, for
//! the Setup panel's "21 files · 1:45:00" before any preview exists.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver};

use reco_io::ffmpeg::decoder::VideoDecoder;

/// A file's length in seconds; 0 when it cannot be read. Blocking (it opens
/// the file): run it off the UI thread.
pub fn file_duration(path: &Path) -> f64 {
    VideoDecoder::open(path)
        .ok()
        .and_then(|d| d.duration_secs())
        .unwrap_or(0.0)
}

/// Measures files on short-lived threads and keeps what it learned.
pub struct DurationProbe {
    known: HashMap<PathBuf, f64>,
    asked: HashSet<PathBuf>,
    pending: Vec<Receiver<Vec<(PathBuf, f64)>>>,
    waker: Arc<dyn Fn() + Send + Sync>,
}

impl DurationProbe {
    /// A probe that runs `waker` when measurements arrive.
    pub fn new(waker: Arc<dyn Fn() + Send + Sync>) -> Self {
        Self {
            known: HashMap::new(),
            asked: HashSet::new(),
            pending: Vec::new(),
            waker,
        }
    }

    /// Measure the files in `paths` not measured or asked for yet.
    pub fn request(&mut self, paths: &[PathBuf]) {
        let new: Vec<PathBuf> = paths
            .iter()
            .filter(|p| !self.known.contains_key(*p) && self.asked.insert((*p).clone()))
            .cloned()
            .collect();
        if new.is_empty() {
            return;
        }
        let (tx, rx) = mpsc::channel();
        let waker = Arc::clone(&self.waker);
        let spawned = std::thread::Builder::new()
            .name("reco-durations".into())
            .spawn(move || {
                let measured = new.into_iter().map(|p| {
                    let secs = file_duration(&p);
                    (p, secs)
                });
                if tx.send(measured.collect()).is_ok() {
                    waker();
                }
            });
        if spawned.is_ok() {
            self.pending.push(rx);
        }
    }

    /// Take the measurements that have arrived; whether any did.
    pub fn collect(&mut self) -> bool {
        let mut arrived = false;
        self.pending.retain(|rx| match rx.try_recv() {
            Ok(measured) => {
                for (path, secs) in measured {
                    self.known.insert(path, secs);
                }
                arrived = true;
                false
            }
            Err(mpsc::TryRecvError::Empty) => true,
            Err(mpsc::TryRecvError::Disconnected) => false,
        });
        arrived
    }

    /// `path`'s length, once measured.
    pub fn duration(&self, path: &Path) -> Option<f64> {
        self.known.get(path).copied()
    }

    /// The lengths of `paths` added up, once every one is measured.
    pub fn total(&self, paths: &[PathBuf]) -> Option<f64> {
        paths.iter().map(|p| self.duration(p)).sum()
    }

    /// Whether measurements are still on their way.
    pub fn is_busy(&self) -> bool {
        !self.pending.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::*;
    use crate::preview::fixtures;

    fn settle(probe: &mut DurationProbe) {
        let deadline = Instant::now() + Duration::from_secs(20);
        while probe.is_busy() && Instant::now() < deadline {
            probe.collect();
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    #[test]
    fn files_are_measured_once_and_remembered() {
        let Some((left, right, _)) = fixtures::fast_set() else {
            return;
        };
        let missing = PathBuf::from("/nonexistent/video.mp4");
        let mut probe = DurationProbe::new(Arc::new(|| {}));
        let files = vec![left.clone(), right.clone(), missing.clone()];
        probe.request(&files);
        assert!(probe.is_busy());
        assert_eq!(probe.total(&files), None, "not measured yet");
        settle(&mut probe);
        assert!((probe.duration(&left).unwrap() - 60.0).abs() < 0.5);
        assert_eq!(probe.duration(&missing), Some(0.0));
        assert!((probe.total(&files).unwrap() - 120.0).abs() < 1.0);
        probe.request(&files);
        assert!(!probe.is_busy(), "nothing new to measure");
    }
}
