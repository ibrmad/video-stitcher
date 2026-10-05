//! Command-line options. Makepad reads its own flags (`--remote`,
//! `--remote=PORT`, ...) from the same list, so unknown flags are ignored.

use std::path::PathBuf;

use reco_io::stitch_job::InputPath;

/// Options this app reads from the command line.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Args {
    /// Initial window size in points, from `--window-size WxH`.
    pub window_size: Option<(f64, f64)>,
    /// Show the shell in one of its states with sample content, from
    /// `--look-preview[=STATE]` (default `ready`). For design review until the
    /// engine drives these states (Modules 1-6).
    pub look_preview: Option<LookPreview>,
    /// Camera files to open into the live preview.
    pub files: Option<FileArgs>,
    /// Read frames back instead of sharing textures (`--preview-readback`).
    pub preview_readback: bool,
}

/// Camera files and calibration to open at startup (the Slint app's
/// `RECO_AUTOLOAD`): `--left a.mp4[;b.mp4] --right … --calibration c.json`.
#[derive(Clone, Debug, PartialEq)]
pub struct FileArgs {
    /// The left camera's files, in order.
    pub left: Vec<PathBuf>,
    /// The right camera's files, in order.
    pub right: Vec<PathBuf>,
    /// The calibration JSON.
    pub calibration: PathBuf,
}

impl FileArgs {
    /// The left camera as one input (chained when it has several files).
    pub fn left_input(&self) -> InputPath {
        input(&self.left)
    }

    /// The right camera as one input.
    pub fn right_input(&self) -> InputPath {
        input(&self.right)
    }
}

fn input(paths: &[PathBuf]) -> InputPath {
    match paths {
        [one] => InputPath::Single(one.clone()),
        many => InputPath::Chained(many.to_vec()),
    }
}

/// `a.mp4;b.mp4` → two paths (empty pieces dropped).
fn split_paths(value: &str) -> Vec<PathBuf> {
    value
        .split(';')
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .map(PathBuf::from)
        .collect()
}

/// The app states `--look-preview` can show.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LookPreview {
    /// One camera has videos; the next step is the other camera.
    OneCamera,
    /// Both cameras have videos; the next step is calibration.
    Cameras,
    /// Calibration is running.
    Calibrating,
    /// Calibration ran and could not line the cameras up.
    CalibrationFailed,
    /// Calibrated and previewing: the default.
    Ready,
    /// An export is running over the preview.
    Exporting,
}

impl LookPreview {
    /// The state named on the command line (`one-camera`, `cameras`,
    /// `calibrating`, `calibration-failed`, `ready`, `exporting`).
    fn parse(name: &str) -> Result<Self, String> {
        match name {
            "one-camera" => Ok(Self::OneCamera),
            "cameras" => Ok(Self::Cameras),
            "calibrating" => Ok(Self::Calibrating),
            "calibration-failed" => Ok(Self::CalibrationFailed),
            "ready" => Ok(Self::Ready),
            "exporting" => Ok(Self::Exporting),
            other => Err(format!(
                "unknown --look-preview state `{other}` \
                 (one-camera, cameras, calibrating, calibration-failed, ready, exporting)"
            )),
        }
    }
}

impl Args {
    /// Parse options from an argument list without the program name.
    pub fn parse<I, S>(args: I) -> Result<Self, String>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut out = Args::default();
        let mut iter = args.into_iter();
        let (mut left, mut right, mut calibration) = (None, None, None);
        while let Some(arg) = iter.next() {
            let arg = arg.as_ref();
            if arg == "--look-preview" {
                out.look_preview = Some(LookPreview::Ready);
            } else if let Some(state) = arg.strip_prefix("--look-preview=") {
                out.look_preview = Some(LookPreview::parse(state)?);
            } else if let Some(value) = arg.strip_prefix("--window-size=") {
                out.window_size = Some(parse_size(value)?);
            } else if arg == "--window-size" {
                let value = iter
                    .next()
                    .ok_or("--window-size needs a value like 1280x820")?;
                out.window_size = Some(parse_size(value.as_ref())?);
            } else if arg == "--preview-readback" {
                out.preview_readback = true;
            } else if let Some((flag, value)) =
                flag_value(arg, &mut iter, &["--left", "--right", "--calibration"])?
            {
                match flag {
                    "--left" => left = Some(split_paths(&value)),
                    "--right" => right = Some(split_paths(&value)),
                    _ => calibration = Some(PathBuf::from(value)),
                }
            }
        }
        out.files = match (left, right, calibration) {
            (None, None, None) => None,
            (Some(left), Some(right), Some(calibration))
                if !left.is_empty() && !right.is_empty() =>
            {
                Some(FileArgs {
                    left,
                    right,
                    calibration,
                })
            }
            (l, r, c) => {
                let missing = [
                    ("--left", l.is_none()),
                    ("--right", r.is_none()),
                    ("--calibration", c.is_none()),
                ]
                .into_iter()
                .find(|(_, m)| *m)
                .map_or("--left", |(f, _)| f);
                return Err(format!(
                    "{missing} is required with the other camera file flags"
                ));
            }
        };
        Ok(out)
    }
}

/// If `arg` is one of `flags`, its value: from `--flag=value` or the next
/// argument.
fn flag_value<'a, I, S>(
    arg: &str,
    iter: &mut I,
    flags: &[&'a str],
) -> Result<Option<(&'a str, String)>, String>
where
    I: Iterator<Item = S>,
    S: AsRef<str>,
{
    for flag in flags {
        if let Some(value) = arg
            .strip_prefix(flag)
            .and_then(|rest| rest.strip_prefix('='))
        {
            return Ok(Some((flag, value.to_string())));
        }
        if arg == *flag {
            let value = iter.next().ok_or_else(|| format!("{flag} needs a value"))?;
            return Ok(Some((flag, value.as_ref().to_string())));
        }
    }
    Ok(None)
}

/// Parse `WxH` (for example `1280x820`) into points.
fn parse_size(value: &str) -> Result<(f64, f64), String> {
    let (w, h) = value
        .split_once(['x', 'X'])
        .ok_or_else(|| format!("window size `{value}` is not WxH"))?;
    let w: f64 = w
        .trim()
        .parse()
        .map_err(|_| format!("bad window width `{w}`"))?;
    let h: f64 = h
        .trim()
        .parse()
        .map_err(|_| format!("bad window height `{h}`"))?;
    if !(320.0..=8192.0).contains(&w) || !(240.0..=8192.0).contains(&h) {
        return Err(format!("window size {w}x{h} is outside 320x240..8192x8192"));
    }
    Ok((w, h))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_args_give_defaults() {
        assert_eq!(Args::parse(Vec::<String>::new()).unwrap(), Args::default());
    }

    #[test]
    fn parses_window_size_in_both_forms() {
        let a = Args::parse(["--window-size", "720x600"]).unwrap();
        assert_eq!(a.window_size, Some((720.0, 600.0)));
        let b = Args::parse(["--window-size=1920X1200"]).unwrap();
        assert_eq!(b.window_size, Some((1920.0, 1200.0)));
    }

    #[test]
    fn parses_look_preview_and_ignores_makepad_flags() {
        let a = Args::parse(["--remote", "--look-preview", "--remote=5000"]).unwrap();
        assert_eq!(a.look_preview, Some(LookPreview::Ready));
        assert_eq!(a.window_size, None);
    }

    #[test]
    fn parses_every_look_preview_state() {
        for (name, state) in [
            ("one-camera", LookPreview::OneCamera),
            ("cameras", LookPreview::Cameras),
            ("calibrating", LookPreview::Calibrating),
            ("calibration-failed", LookPreview::CalibrationFailed),
            ("ready", LookPreview::Ready),
            ("exporting", LookPreview::Exporting),
        ] {
            let a = Args::parse([format!("--look-preview={name}")]).unwrap();
            assert_eq!(a.look_preview, Some(state), "{name}");
        }
    }

    #[test]
    fn rejects_unknown_look_preview_state() {
        assert!(Args::parse(["--look-preview=party"]).is_err());
    }

    #[test]
    fn parses_the_three_files() {
        let a = Args::parse(
            ["--left=a.mp4", "--right", "b.mp4", "--calibration=c.json"].map(String::from),
        )
        .unwrap();
        let files = a.files.expect("files");
        assert_eq!(files.left, vec![PathBuf::from("a.mp4")]);
        assert_eq!(files.right, vec![PathBuf::from("b.mp4")]);
        assert_eq!(files.calibration, PathBuf::from("c.json"));
        assert!(matches!(files.left_input(), InputPath::Single(_)));
    }

    #[test]
    fn chains_files_split_by_semicolons() {
        let a = Args::parse(
            [
                "--left=a1.mp4;a2.mp4",
                "--right=b.mp4",
                "--calibration=c.json",
            ]
            .map(String::from),
        )
        .unwrap();
        let files = a.files.unwrap();
        assert_eq!(files.left.len(), 2);
        assert!(matches!(files.left_input(), InputPath::Chained(v) if v.len() == 2));
    }

    #[test]
    fn files_must_come_as_a_set() {
        let err =
            Args::parse(["--left=a.mp4", "--calibration=c.json"].map(String::from)).unwrap_err();
        assert!(err.contains("--right"), "{err}");
    }

    #[test]
    fn readback_flag() {
        assert!(
            Args::parse(["--preview-readback".to_string()])
                .unwrap()
                .preview_readback
        );
        assert!(!Args::parse(Vec::<String>::new()).unwrap().preview_readback);
    }

    #[test]
    fn rejects_bad_sizes() {
        assert!(Args::parse(["--window-size", "wide"]).is_err());
        assert!(Args::parse(["--window-size", "100x100"]).is_err());
        assert!(Args::parse(["--window-size"]).is_err());
    }
}
