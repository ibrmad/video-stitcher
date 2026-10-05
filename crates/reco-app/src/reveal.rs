//! "Show in folder": show a file in the system's file manager. Finder and
//! Explorer select it; elsewhere its folder opens. The command is spawned
//! and not waited for (a small thread reaps it).

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
