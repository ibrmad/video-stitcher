//! The commit the app is built from, for the app menu and bug reports
//! (`RECO_GIT_HASH`; missing outside a git checkout).

use std::path::Path;

fn main() {
    let git = Path::new("../../.git");
    // Build again when the commit changes: HEAD moves to another branch, or
    // the branch it names moves on.
    println!("cargo:rerun-if-changed=../../.git/HEAD");
    if let Ok(head) = std::fs::read_to_string(git.join("HEAD")) {
        if let Some(branch) = head.trim().strip_prefix("ref: ") {
            // A path that doesn't exist would run this script on every build.
            if git.join(branch).is_file() {
                println!("cargo:rerun-if-changed=../../.git/{branch}");
            }
        }
    }
    if git.join("packed-refs").is_file() {
        println!("cargo:rerun-if-changed=../../.git/packed-refs");
    }
    let hash = std::process::Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .filter(|out| out.status.success())
        .map(|out| String::from_utf8_lossy(&out.stdout).trim().to_string());
    if let Some(hash) = hash.filter(|h| !h.is_empty()) {
        println!("cargo:rustc-env=RECO_GIT_HASH={hash}");
    }
}
