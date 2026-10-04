//! Display helpers for file names.

/// `name` shortened to at most `max_chars` characters by replacing its
/// middle with `…`, so both the start (camera and chapter) and the end
/// (extension) stay visible.
pub fn middle_ellipsis(name: &str, max_chars: usize) -> String {
    let chars: Vec<char> = name.chars().collect();
    if chars.len() <= max_chars {
        return name.to_string();
    }
    if max_chars < 2 {
        return "…".to_string();
    }
    let keep = max_chars - 1;
    let head = keep.div_ceil(2);
    let tail = keep / 2;
    let mut out: String = chars[..head].iter().collect();
    out.push('…');
    out.extend(&chars[chars.len() - tail..]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_names_are_unchanged() {
        assert_eq!(middle_ellipsis("match.json", 16), "match.json");
    }

    #[test]
    fn long_names_keep_both_ends() {
        let out = middle_ellipsis("GX010120_left_camera_chapter.MP4", 16);
        assert_eq!(out, "GX010120…ter.MP4");
        assert_eq!(out.chars().count(), 16);
    }

    #[test]
    fn counts_characters_not_bytes() {
        assert_eq!(middle_ellipsis("åäöåäöåäö", 5), "åä…äö");
    }

    #[test]
    fn tiny_limits_give_just_the_ellipsis() {
        assert_eq!(middle_ellipsis("abcdef", 1), "…");
        assert_eq!(middle_ellipsis("abcdef", 0), "…");
    }
}
