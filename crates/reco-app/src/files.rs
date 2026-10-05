//! Small file helpers shared by the jobs that write the user's files.

use std::path::Path;

/// Write `json` to `path` through a temporary file, so a half-written
/// calibration never replaces a good one.
pub fn save_atomically(path: &Path, json: &str) -> std::io::Result<()> {
    let temp = path.with_extension("json.tmp");
    std::fs::write(&temp, json)?;
    std::fs::rename(&temp, path).inspect_err(|_| {
        let _ = std::fs::remove_file(&temp);
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saving_replaces_the_file_whole() {
        let dir = std::env::temp_dir().join(format!("reco-app-save-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("c_calibration.json");
        save_atomically(&path, "{\"a\":1}").unwrap();
        save_atomically(&path, "{\"a\":2}").unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "{\"a\":2}");
        let leftovers = std::fs::read_dir(&dir).unwrap().count();
        assert_eq!(leftovers, 1, "no temporary file is left behind");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
