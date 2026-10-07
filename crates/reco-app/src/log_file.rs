//! The app's log file: engine lines (the `log` crate) and the app's own
//! lines, kept across restarts and started afresh once over 2 MB, filtered
//! by `RUST_LOG`, with panics written to it; the bug report attaches its
//! tail.

use std::fs::{File, OpenOptions};
use std::io::{self, BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::mpsc::{self, Sender};
use std::time::{SystemTime, UNIX_EPOCH};

use log::{Level, LevelFilter, Log, Metadata, Record};

/// Names the file instead (the checks keep theirs out of the person's).
pub const LOG_FILE_VAR: &str = "RECO_DESKTOP_LOG_FILE";

/// A file this large is started afresh when the app starts.
pub const CUT_AT: u64 = 2_000_000;

/// The filter when `RUST_LOG` isn't set: ONNX Runtime's chatter held to
/// warnings.
pub const DEFAULT_FILTER: &str = "info,ort=warn";

/// Which lines are kept: `RUST_LOG`'s `level` and `target=level`
/// directives, the longest matching target winning.
#[derive(Clone, Debug, PartialEq)]
pub struct Filter {
    default: LevelFilter,
    targets: Vec<(String, LevelFilter)>,
}

impl Filter {
    /// Read `spec`; directives that don't read are left out, and the level
    /// is Info until one says otherwise.
    pub fn parse(spec: &str) -> Self {
        let mut filter = Self {
            default: LevelFilter::Info,
            targets: Vec::new(),
        };
        for directive in spec.split(',').map(str::trim).filter(|d| !d.is_empty()) {
            match directive.split_once('=') {
                Some((target, level)) => {
                    if let (false, Ok(level)) = (target.trim().is_empty(), level.trim().parse()) {
                        filter.targets.push((target.trim().to_string(), level));
                    }
                }
                None => {
                    if let Ok(level) = directive.parse() {
                        filter.default = level;
                    }
                }
            }
        }
        filter
    }

    /// Whether a line at `level` from `target` is kept.
    pub fn enabled(&self, target: &str, level: Level) -> bool {
        let wanted = self
            .targets
            .iter()
            .filter(|(prefix, _)| {
                target == prefix
                    || target
                        .strip_prefix(prefix.as_str())
                        .is_some_and(|rest| rest.starts_with("::"))
            })
            .max_by_key(|(prefix, _)| prefix.len())
            .map_or(self.default, |(_, level)| *level);
        level <= wanted
    }

    /// The most detailed level any directive keeps.
    pub fn max(&self) -> LevelFilter {
        self.targets
            .iter()
            .map(|(_, level)| *level)
            .chain([self.default])
            .max()
            .unwrap_or(LevelFilter::Info)
    }
}

/// Where the log lives on `os` (`std::env::consts::OS`): macOS's
/// `~/Library/Logs`, Linux's state folder, beside the executable on
/// Windows, unless `RECO_DESKTOP_LOG_FILE` names one.
pub fn default_path(
    os: &str,
    env: impl Fn(&str) -> Option<String>,
    exe: Option<PathBuf>,
) -> Option<PathBuf> {
    if let Some(named) = env(LOG_FILE_VAR).filter(|p| !p.is_empty()) {
        return Some(PathBuf::from(named));
    }
    const NAME: &str = "reco-desktop.log";
    match os {
        "macos" => env("HOME").map(|home| PathBuf::from(home).join("Library/Logs").join(NAME)),
        "windows" => exe.and_then(|exe| exe.parent().map(|dir| dir.join(NAME))),
        _ => env("XDG_STATE_HOME")
            .filter(|p| !p.is_empty())
            .map(PathBuf::from)
            .or_else(|| env("HOME").map(|home| PathBuf::from(home).join(".local/state")))
            .map(|state| state.join("reco").join(NAME)),
    }
}

/// This app's log file.
pub fn path() -> Option<PathBuf> {
    default_path(
        std::env::consts::OS,
        |key| std::env::var(key).ok(),
        std::env::current_exe().ok(),
    )
}

/// `time` as `2026-10-06T13:02:03.045Z`.
pub fn utc(time: SystemTime) -> String {
    let since = time.duration_since(UNIX_EPOCH).unwrap_or_default();
    let secs = since.as_secs();
    let (days, of_day) = ((secs / 86_400) as i64, secs % 86_400);
    // Days to a civil date (Howard Hinnant's `civil_from_days`).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}.{:03}Z",
        of_day / 3_600,
        of_day / 60 % 60,
        of_day % 60,
        since.subsec_millis()
    )
}

/// What the writer thread is asked.
enum Writing {
    Line(String),
    Flushed(Sender<()>),
}

/// The open log: lines are timestamped here and written on their own
/// thread, so nothing waits for the disk.
#[derive(Clone)]
pub struct LogFile {
    path: PathBuf,
    tx: Sender<Writing>,
}

impl LogFile {
    /// Open (or make) the file and its folder, starting afresh when it is
    /// over 2 MB.
    pub fn open(path: &Path) -> io::Result<Self> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        if path.metadata().is_ok_and(|m| m.len() > CUT_AT) {
            std::fs::remove_file(path)?;
        }
        let mut file = OpenOptions::new().create(true).append(true).open(path)?;
        let (tx, rx) = mpsc::channel::<Writing>();
        std::thread::Builder::new()
            .name("reco-log".into())
            .spawn(move || {
                for write in rx {
                    match write {
                        Writing::Line(line) => {
                            let _ = writeln!(file, "{line}");
                        }
                        Writing::Flushed(done) => {
                            let _ = file.flush();
                            let _ = done.send(());
                        }
                    }
                }
            })?;
        Ok(Self {
            path: path.to_path_buf(),
            tx,
        })
    }

    /// Where it is.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Keep one line, with the time.
    pub fn line(&self, text: &str) {
        let _ = self
            .tx
            .send(Writing::Line(format!("{} {text}", utc(SystemTime::now()))));
    }

    /// Wait until every line so far is in the file.
    pub fn flush(&self) {
        let (done, wait) = mpsc::channel();
        if self.tx.send(Writing::Flushed(done)).is_ok() {
            let _ = wait.recv();
        }
    }

    /// The file's last `n` lines, oldest first (call `flush` first for the
    /// newest).
    pub fn tail(&self, n: usize) -> Vec<String> {
        let Ok(file) = File::open(&self.path) else {
            return Vec::new();
        };
        let lines: Vec<String> = BufReader::new(file).lines().map_while(Result::ok).collect();
        lines[lines.len().saturating_sub(n)..].to_vec()
    }
}

/// The `log` crate's lines into the file.
struct FileLogger {
    file: Mutex<LogFile>,
    filter: Filter,
}

impl Log for FileLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        self.filter.enabled(metadata.target(), metadata.level())
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata())
            && let Ok(file) = self.file.lock()
        {
            file.line(&format!(
                "[{}] {}: {}",
                record.level(),
                record.target(),
                record.args()
            ));
        }
    }

    fn flush(&self) {
        if let Ok(file) = self.file.lock() {
            file.flush();
        }
    }
}

/// Send the `log` crate's lines (the engine's) to `file`, filtered by
/// `RUST_LOG` or [`DEFAULT_FILTER`]; `false` if a logger was already set.
pub fn install_logger(file: LogFile) -> bool {
    let spec = std::env::var("RUST_LOG").unwrap_or_else(|_| DEFAULT_FILTER.to_string());
    let filter = Filter::parse(&spec);
    let max = filter.max();
    // One logger for the process's life.
    let logger = Box::leak(Box::new(FileLogger {
        file: Mutex::new(file),
        filter,
    }));
    let installed = log::set_logger(logger).is_ok();
    if installed {
        log::set_max_level(max);
    }
    installed
}

/// Write a panic straight into the file at `path` (the process may not
/// live to flush the writer), then run the hook that was there.
pub fn install_panic_hook(path: PathBuf) {
    let before = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let location = info
            .location()
            .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
            .unwrap_or_else(|| "<unknown>".into());
        let payload = info
            .payload()
            .downcast_ref::<&str>()
            .map(|s| s.to_string())
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "<not text>".into());
        if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(&path) {
            let _ = writeln!(
                file,
                "{} [PANIC] at {location}: {payload}",
                utc(SystemTime::now())
            );
        }
        before(info);
    }));
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, UNIX_EPOCH};

    use log::{Level, LevelFilter};

    use super::*;

    #[test]
    fn the_filter_reads_like_rust_log() {
        let filter = Filter::parse(DEFAULT_FILTER);
        assert!(filter.enabled("reco_core::render", Level::Info));
        assert!(!filter.enabled("reco_core::render", Level::Debug));
        assert!(
            !filter.enabled("ort::logging", Level::Info),
            "ort is held to warnings"
        );
        assert!(filter.enabled("ort", Level::Warn));
        assert_eq!(filter.max(), LevelFilter::Info);

        let filter = Filter::parse("warn, reco_io=trace");
        assert!(filter.enabled("reco_io::ffmpeg", Level::Trace));
        assert!(!filter.enabled("reco_core", Level::Info));
        assert_eq!(filter.max(), LevelFilter::Trace);

        let filter = Filter::parse("nonsense=, =debug, loud");
        assert!(
            filter.enabled("anything", Level::Info),
            "what doesn't read is left out"
        );
        assert!(!filter.enabled("anything", Level::Debug));
    }

    #[test]
    fn the_file_lives_where_each_system_keeps_logs() {
        let env = |pairs: &'static [(&'static str, &'static str)]| {
            move |key: &str| {
                pairs
                    .iter()
                    .find(|(k, _)| *k == key)
                    .map(|(_, v)| v.to_string())
            }
        };
        assert_eq!(
            default_path("macos", env(&[("HOME", "/Users/ann")]), None),
            Some(PathBuf::from("/Users/ann/Library/Logs/reco-desktop.log"))
        );
        assert_eq!(
            default_path("linux", env(&[("HOME", "/home/ann")]), None),
            Some(PathBuf::from(
                "/home/ann/.local/state/reco/reco-desktop.log"
            ))
        );
        assert_eq!(
            default_path(
                "linux",
                env(&[("HOME", "/home/ann"), ("XDG_STATE_HOME", "/s")]),
                None
            ),
            Some(PathBuf::from("/s/reco/reco-desktop.log"))
        );
        assert_eq!(
            default_path(
                "windows",
                env(&[]),
                Some(PathBuf::from("C:/Reco/reco-desktop.exe"))
            ),
            Some(PathBuf::from("C:/Reco/reco-desktop.log"))
        );
        assert_eq!(
            default_path(
                "macos",
                env(&[("HOME", "/Users/ann"), (LOG_FILE_VAR, "/tmp/check.log")]),
                None
            ),
            Some(PathBuf::from("/tmp/check.log")),
            "the checks name their own"
        );
    }

    #[test]
    fn lines_carry_the_time_in_utc() {
        assert_eq!(utc(UNIX_EPOCH), "1970-01-01T00:00:00.000Z");
        let t = UNIX_EPOCH + Duration::from_millis(1_791_291_723_045);
        assert_eq!(utc(t), "2026-10-06T13:02:03.045Z");
        let leap = UNIX_EPOCH + Duration::from_secs(951_868_799);
        assert_eq!(utc(leap), "2000-02-29T23:59:59.000Z");
    }

    #[test]
    fn the_file_keeps_lines_across_runs_and_starts_afresh_when_large() {
        let dir = std::env::temp_dir().join(format!("reco-log-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("nested/reco-desktop.log");
        let log = LogFile::open(&path).expect("opens, making its folder");
        log.line("[I] first run");
        log.flush();
        drop(log);
        let log = LogFile::open(&path).expect("opens again");
        log.line("[I] second run");
        log.flush();
        let tail = log.tail(2);
        assert_eq!(tail.len(), 2);
        assert!(tail[0].ends_with("[I] first run"), "{tail:?}");
        assert!(tail[1].ends_with("[I] second run"), "{tail:?}");
        assert_eq!(log.tail(1).len(), 1);
        drop(log);

        std::fs::write(&path, vec![b'x'; CUT_AT as usize + 1]).unwrap();
        let log = LogFile::open(&path).expect("opens a large file");
        log.line("[I] fresh");
        log.flush();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(
            text.len() < 200 && text.trim_end().ends_with("[I] fresh"),
            "{text:?}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
