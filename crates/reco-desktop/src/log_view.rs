//! The log file (reco-app's `log_file`): opened at startup with the
//! engine's logger and the panic hook, and the app's own lines (Makepad's
//! log ring) copied in as they come.

use makepad_widgets::makepad_platform::log_ring;
use makepad_widgets::*;
use reco_app::log_file::{self, LogFile};

use crate::App;

impl App {
    /// Open the log file and send the engine's lines and panics to it.
    pub(crate) fn start_log(&mut self) {
        let Some(path) = log_file::path() else {
            return;
        };
        match LogFile::open(&path) {
            Ok(file) => {
                log_file::install_logger(file.clone());
                log_file::install_panic_hook(path.clone());
                self.log_file = Some(file);
                log!("log file: {}", path.display());
            }
            Err(e) => log!("no log file at {}: {e}", path.display()),
        }
    }

    /// Copy the app's lines logged since the last copy into the file.
    pub(crate) fn write_log(&mut self) {
        let Some(file) = self.log_file.as_ref() else {
            return;
        };
        let (next, lines) = log_ring::read_since(self.log_written, log_ring::CAP);
        self.log_written = next;
        for line in lines {
            file.line(&line.text);
        }
    }

    /// Copy the app's newest lines in and wait until the file has them all
    /// (at shutdown, and before reading it).
    pub(crate) fn flush_log(&mut self) {
        self.write_log();
        if let Some(file) = self.log_file.as_ref() {
            file.flush();
        }
    }

    /// The log's newest `n` lines, everything so far written first (the
    /// app's own lines when there is no file).
    pub(crate) fn log_tail(&mut self, n: usize) -> Vec<String> {
        self.flush_log();
        match self.log_file.as_ref() {
            Some(file) => file.tail(n),
            None => log_ring::read_since(0, n)
                .1
                .into_iter()
                .map(|line| line.text)
                .collect(),
        }
    }
}
