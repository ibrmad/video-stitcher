//! "Show in folder": show a file in the system's file manager (Finder and
//! Explorer select it; elsewhere its folder opens), and open a page or a
//! link in the browser. Each command is spawned and not waited for (a small
//! thread reaps it).

use std::path::Path;
use std::process::Command;

/// The program and arguments that show `path`.
pub fn reveal_command(path: &Path) -> (String, Vec<String>) {
    if cfg!(target_os = "macos") {
        ("open".into(), vec!["-R".into(), path.display().to_string()])
    } else if cfg!(target_os = "windows") {
        (
            "explorer".into(),
            vec![format!("/select,{}", path.display())],
        )
    } else {
        let folder = path.parent().unwrap_or(path);
        ("xdg-open".into(), vec![folder.display().to_string()])
    }
}

/// The program and arguments that open `target` (a page or a link) in the
/// default browser.
pub fn browser_command(target: &str) -> (String, Vec<String>) {
    if cfg!(target_os = "macos") {
        ("open".into(), vec![target.into()])
    } else if cfg!(target_os = "windows") {
        (
            "cmd".into(),
            vec!["/c".into(), "start".into(), String::new(), target.into()],
        )
    } else {
        ("xdg-open".into(), vec![target.into()])
    }
}

/// Open `target` (a page or a link) in the default browser; returns once
/// the command started.
pub fn open_in_browser(target: &str) -> std::io::Result<()> {
    let (program, args) = browser_command(target);
    let mut child = Command::new(program).args(args).spawn()?;
    std::thread::spawn(move || child.wait());
    Ok(())
}

/// Show `path` in the file manager; returns once the command started.
pub fn reveal(path: &Path) -> std::io::Result<()> {
    let (program, args) = reveal_command(path);
    let mut child = Command::new(program).args(args).spawn()?;
    std::thread::spawn(move || child.wait());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_browser_opens_a_page_or_a_link() {
        let link = "https://forum.reco-project.org/";
        let (program, args) = browser_command(link);
        let expected: (&str, Vec<&str>) = if cfg!(target_os = "macos") {
            ("open", vec![link])
        } else if cfg!(target_os = "windows") {
            // `start` takes a quoted first word for a window title.
            ("cmd", vec!["/c", "start", "", link])
        } else {
            ("xdg-open", vec![link])
        };
        assert_eq!(
            (
                program.as_str(),
                args.iter().map(String::as_str).collect::<Vec<_>>()
            ),
            expected
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn finder_selects_the_file() {
        assert_eq!(
            reveal_command(Path::new("/tmp/a b.mp4")),
            (
                "open".to_string(),
                vec!["-R".to_string(), "/tmp/a b.mp4".to_string()]
            )
        );
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    #[test]
    fn the_folder_opens_elsewhere() {
        assert_eq!(
            reveal_command(Path::new("/tmp/x/a.mp4")),
            ("xdg-open".to_string(), vec!["/tmp/x".to_string()])
        );
    }
}
