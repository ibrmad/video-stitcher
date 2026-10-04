//! Command-line options. Makepad reads its own flags (`--remote`,
//! `--remote=PORT`, ...) from the same list, so unknown flags are ignored.

/// Options this app reads from the command line.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Args {
    /// Initial window size in points, from `--window-size WxH`.
    pub window_size: Option<(f64, f64)>,
    /// Show the shell as if files were loaded, with sample content, from
    /// `--look-preview`. For design review until loading exists (Module 1).
    pub look_preview: bool,
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
        while let Some(arg) = iter.next() {
            let arg = arg.as_ref();
            if arg == "--look-preview" {
                out.look_preview = true;
            } else if let Some(value) = arg.strip_prefix("--window-size=") {
                out.window_size = Some(parse_size(value)?);
            } else if arg == "--window-size" {
                let value = iter
                    .next()
                    .ok_or("--window-size needs a value like 1280x820")?;
                out.window_size = Some(parse_size(value.as_ref())?);
            }
        }
        Ok(out)
    }
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
        assert!(a.look_preview);
        assert_eq!(a.window_size, None);
    }

    #[test]
    fn rejects_bad_sizes() {
        assert!(Args::parse(["--window-size", "wide"]).is_err());
        assert!(Args::parse(["--window-size", "100x100"]).is_err());
        assert!(Args::parse(["--window-size"]).is_err());
    }
}
