//! The window shell's widgets, registered in dependency order. Each file
//! registers its widgets under `mod.widgets.Reco*`.

mod bug_sheet;
mod controls;
mod export_sheet;
pub mod file_list;
mod fold;
mod inspector;
pub mod key_table;
mod lens_picker;
mod media_panel;
pub mod menu_list;
pub mod panorama;
pub mod pick_list;
mod prefs_sheet;
pub mod preview;
mod shell;
mod shortcuts_sheet;
pub mod time_panel;
mod toasts;
mod top_bar;
mod viewer;
pub mod zones;

use makepad_widgets::*;

/// Register every shell widget. Call after `makepad_widgets::widgets_mod`
/// and before the app's own script module.
pub fn script_mod(vm: &mut ScriptVm) {
    fold::script_mod(vm);
    controls::script_mod(vm);
    menu_list::script_mod(vm);
    file_list::script_mod(vm);
    toasts::script_mod(vm);
    panorama::script_mod(vm);
    preview::script_mod(vm);
    top_bar::script_mod(vm);
    media_panel::script_mod(vm);
    viewer::script_mod(vm);
    inspector::script_mod(vm);
    zones::script_mod(vm);
    export_sheet::script_mod(vm);
    pick_list::script_mod(vm);
    lens_picker::script_mod(vm);
    prefs_sheet::script_mod(vm);
    key_table::script_mod(vm);
    shortcuts_sheet::script_mod(vm);
    bug_sheet::script_mod(vm);
    time_panel::script_mod(vm);
    shell::script_mod(vm);
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    /// The start of a crate-relative resource reference. Assembled from two
    /// pieces so this file's own source does not match it.
    const NEEDLE: &str = concat!("crate_resource(\"", "self:");

    /// Every crate-relative resource path in `source`, relative to the
    /// crate root.
    fn self_resources(source: &str) -> Vec<&str> {
        source
            .split(NEEDLE)
            .skip(1)
            .filter_map(|rest| rest.split('"').next())
            .collect()
    }

    /// Every `.rs` file under `dir`, recursively.
    fn rust_files(dir: &Path) -> Vec<PathBuf> {
        let mut files = Vec::new();
        for entry in std::fs::read_dir(dir).expect("readable source dir") {
            let path = entry.expect("dir entry").path();
            if path.is_dir() {
                files.extend(rust_files(&path));
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                files.push(path);
            }
        }
        files
    }

    #[test]
    fn finds_self_resources() {
        let src = format!(r#"a: {NEEDLE}resources/icons/a.svg") b: {NEEDLE}x/b.png")"#);
        assert_eq!(self_resources(&src), ["resources/icons/a.svg", "x/b.png"]);
    }

    /// Makepad skips an SVG it cannot find without logging anything, so a
    /// typo in an icon path would otherwise only show as a missing icon.
    /// Makepad scales an SVG's drawn content, not its viewBox, to the icon
    /// box, so every icon pins its full 16 by 16 box with an invisible rect.
    /// Without it a small drawing is blown up to fill the box: the step
    /// icons once drew larger than Play.
    #[test]
    fn every_icon_pins_its_viewbox() {
        // Makepad scales an icon by its drawn content, not its viewBox, so
        // each icon pins its whole box with an invisible rect. Icons paint in
        // #000 for the widget to tint, never `currentColor`.
        let icons = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("resources")
            .join("icons");
        let mut loose = Vec::new();
        for entry in std::fs::read_dir(&icons).expect("icons folder") {
            let path = entry.expect("readable entry").path();
            if path.extension().is_some_and(|ext| ext == "svg") {
                let text = std::fs::read_to_string(&path).expect("readable icon");
                let size = text
                    .split_once("viewBox=\"0 0 ")
                    .and_then(|(_, rest)| rest.split_once('"'))
                    .and_then(|(size, _)| size.split_once(' '));
                let pinned = size.is_some_and(|(w, h)| {
                    text.contains(&format!(
                        r#"<rect x="0" y="0" width="{w}" height="{h}" fill="none" stroke="none" />"#
                    ))
                });
                if !pinned || text.contains("currentColor") {
                    loose.push(path.file_name().unwrap().to_string_lossy().into_owned());
                }
            }
        }
        loose.sort();
        assert!(
            loose.is_empty(),
            "icons without a pinned viewBox, or painted in currentColor: {loose:?}"
        );
    }

    #[test]
    fn every_self_resource_exists() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let mut checked = 0;
        let mut missing = Vec::new();
        for file in rust_files(&root.join("src")) {
            let text = std::fs::read_to_string(&file).expect("readable source");
            for resource in self_resources(&text) {
                checked += 1;
                if !root.join(resource).is_file() {
                    missing.push(format!("{}: {resource}", file.display()));
                }
            }
        }
        assert!(checked > 0, "no crate-relative resource references found");
        assert!(
            missing.is_empty(),
            "missing resources:\n{}",
            missing.join("\n")
        );
    }

    /// Keys whose numbers are data, not design: slider ranges, the
    /// fractions of an `Align`, and line limits.
    const DATA_KEYS: [&str; 8] = [
        "min",
        "max",
        "default",
        "precision",
        "step",
        "x",
        "y",
        "max_lines",
    ];

    /// `line` with string-literal contents blanked and any `//` comment
    /// removed, so text such as `"16:9"` is not read as code.
    fn code_only(line: &str) -> String {
        let mut out = String::with_capacity(line.len());
        let mut in_string = false;
        let mut escaped = false;
        for c in line.chars() {
            if !in_string {
                in_string = c == '"';
                out.push(c);
            } else if escaped {
                escaped = false;
                out.push(' ');
            } else if c == '\\' {
                escaped = true;
                out.push(' ');
            } else if c == '"' {
                in_string = false;
                out.push(c);
            } else {
                out.push(' ');
            }
        }
        match out.find("//") {
            Some(at) => out[..at].to_string(),
            None => out,
        }
    }

    /// Raw design values in a screen file (DESIGN.md Rule 5): colour
    /// literals, and non-zero numbers after a `key:` that is not a data key.
    fn raw_values(source: &str) -> Vec<String> {
        let mut found = Vec::new();
        for (index, line) in source.lines().enumerate() {
            let code = code_only(line);
            let code = code.as_str();
            for (at, _) in code.match_indices('#') {
                let rest = &code[at + 1..];
                let hex = rest.strip_prefix('x').unwrap_or(rest);
                let digits = hex.chars().take_while(char::is_ascii_hexdigit).count();
                if digits >= 3 {
                    let literal = &rest[..rest.len() - hex.len() + digits];
                    found.push(format!("line {}: colour #{literal}", index + 1));
                }
            }
            for (at, _) in code.match_indices(':') {
                let key: String = code[..at]
                    .chars()
                    .rev()
                    .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                    .collect::<Vec<_>>()
                    .into_iter()
                    .rev()
                    .collect();
                let number: String = code[at + 1..]
                    .trim_start()
                    .chars()
                    .take_while(|c| c.is_ascii_digit() || *c == '.' || *c == '-')
                    .collect();
                let Ok(value) = number.parse::<f64>() else {
                    continue;
                };
                if !key.is_empty() && !DATA_KEYS.contains(&key.as_str()) && value != 0.0 {
                    found.push(format!("line {}: {key}: {number}", index + 1));
                }
            }
        }
        found
    }

    #[test]
    fn raw_values_finds_colours_and_sizes_but_not_data() {
        let src = "a := View{width: 30 padding: theme.reco_space_m spacing: 0}\n\
                   b := Label{color: #x1e1e2e} // width: 99 in a comment\n\
                   c := Slider{min: -30.0 max: 30.0 align: Align{x: 0.5 y: 0.5}}\n\
                   d := DropDown{labels: [\"16:9\" \"4:3\"]}";
        assert_eq!(
            raw_values(src),
            ["line 1: width: 30", "line 2: colour #x1e1e2e"]
        );
    }

    /// Screens take every colour, size, spacing step and type size from the
    /// theme (DESIGN.md Rule 5).
    #[test]
    fn screens_use_theme_values_only() {
        let ui = Path::new(env!("CARGO_MANIFEST_DIR")).join("src").join("ui");
        let mut found = Vec::new();
        for file in rust_files(&ui) {
            if file.ends_with("mod.rs") {
                continue;
            }
            let text = std::fs::read_to_string(&file).expect("readable source");
            for value in raw_values(&text) {
                found.push(format!("{}: {value}", file.display()));
            }
        }
        assert!(
            found.is_empty(),
            "raw design values outside theme.rs:\n{}",
            found.join("\n")
        );
    }
}
