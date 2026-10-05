//! The app menu's help: the project's pages, and the update check (the
//! latest release on GitHub against this build). Pure; the app fetches.

/// The project's home on GitHub.
pub const WEBSITE: &str = "https://github.com/reco-project/video-stitcher";
/// The project's forum.
pub const FORUM: &str = "https://forum.reco-project.org/";
/// GitHub's answer for the latest release.
pub const LATEST_RELEASE: &str =
    "https://api.github.com/repos/reco-project/video-stitcher/releases/latest";

/// The page of the release `tag`.
pub fn release_page(tag: &str) -> String {
    format!("{WEBSITE}/releases/tag/{tag}")
}

/// The tag in a GitHub release's JSON, when it is a plain version name
/// (letters, digits, dots, dashes and underscores: it ends up in a URL
/// the system opens).
pub fn release_tag(body: &str) -> Option<String> {
    let release: serde_json::Value = serde_json::from_str(body).ok()?;
    let tag = release["tag_name"].as_str()?;
    let plain = !tag.is_empty()
        && tag
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'));
    plain.then(|| tag.to_string())
}

/// Major, minor and patch of "v0.5.4", "0.6" or "0.5.4-rc1" (missing parts
/// are 0; what follows the patch is ignored).
fn version(name: &str) -> Option<[u64; 3]> {
    let numbers = name.trim_start_matches('v');
    let numbers = numbers.split(['-', '+']).next()?;
    let mut out = [0; 3];
    for (slot, part) in numbers.split('.').enumerate() {
        *out.get_mut(slot)? = part.parse().ok()?;
    }
    Some(out)
}

/// Whether the release `tag` ("v0.6.0") is newer than `current` ("0.5.4"):
/// major, minor and patch compared as numbers; anything else after them
/// (a "-rc1") counts for nothing, and a name that isn't a version is never
/// newer.
pub fn is_newer(tag: &str, current: &str) -> bool {
    match (version(tag), version(current)) {
        (Some(latest), Some(current)) => latest > current,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_compare_by_number() {
        assert!(is_newer("v0.6.0", "0.5.4"));
        assert!(is_newer("v0.5.10", "0.5.4"), "10 is more than 4");
        assert!(is_newer("1.0", "0.5.4"));
        assert!(is_newer("v0.5.5-rc1", "0.5.4"));
        assert!(!is_newer("v0.5.4", "0.5.4"));
        assert!(!is_newer("v0.5.3", "0.5.4"));
        assert!(!is_newer("v0.5.4-rc1", "0.5.4"));
        assert!(!is_newer("nightly", "0.5.4"));
        assert!(!is_newer("v0.6.0", "dev"));
    }

    #[test]
    fn the_tag_comes_from_the_release() {
        assert_eq!(
            release_tag(r#"{"tag_name":"v0.6.0","name":"Reco 0.6"}"#).as_deref(),
            Some("v0.6.0")
        );
        assert_eq!(
            release_tag(r#"{"message":"API rate limit exceeded"}"#),
            None
        );
        assert_eq!(release_tag("<html>"), None);
        assert_eq!(
            release_tag(r#"{"tag_name":"v1 & calc"}"#),
            None,
            "only a plain name reaches a URL"
        );
    }

    #[test]
    fn the_release_page_names_the_tag() {
        assert_eq!(
            release_page("v0.6.0"),
            "https://github.com/reco-project/video-stitcher/releases/tag/v0.6.0"
        );
    }
}
