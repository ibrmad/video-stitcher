//! The field outline (ROI): where the pitch is in each camera's picture, for
//! AI tracking. The outline is drawn in the browser editor the Slint app
//! shipped (`resources/roi_editor.html`): this writes the editor with the
//! current frames and the calibration, and reads back the JSON the editor
//! copies.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver};

use base64::Engine;
use reco_core::calibration::FieldRoi;
use reco_io::ffmpeg::calibration_io::extract_frames;

/// The editor page, with placeholders for the frames and the calibration.
const TEMPLATE: &str = include_str!("../../../resources/roi_editor.html");

/// What the editor is written from.
#[derive(Clone, Debug)]
pub struct EditorRequest {
    /// The left camera's first file.
    pub left: PathBuf,
    /// The right camera's first file.
    pub right: PathBuf,
    /// The frame shown in both.
    pub frame: u64,
    /// The calibration file.
    pub calibration: PathBuf,
}

/// Where the editor is written: `$XDG_CACHE_HOME/reco/roi`, else
/// `~/.cache/reco/roi` (a sandboxed browser can't read `/tmp`).
pub fn editor_folder() -> PathBuf {
    std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".cache")))
        .unwrap_or_else(std::env::temp_dir)
        .join("reco")
        .join("roi")
}

/// An outline from the editor's JSON: at least three points for a camera,
/// every point inside the picture (0 to 1).
pub fn parse_outline(text: &str) -> Result<FieldRoi, String> {
    let roi: FieldRoi =
        serde_json::from_str(text.trim()).map_err(|e| format!("it isn't an outline ({e})"))?;
    if roi.left.len() < 3 && roi.right.len() < 3 {
        return Err("an outline needs at least three points".into());
    }
    let inside = |p: &[f64; 2]| (0.0..=1.0).contains(&p[0]) && (0.0..=1.0).contains(&p[1]);
    if !roi.left.iter().chain(&roi.right).all(inside) {
        return Err("a point lies outside the picture".into());
    }
    Ok(roi)
}

/// Write the editor into `folder` with both cameras' frame and the
/// calibration; the page's path. Blocking (it decodes two frames): run it
/// off the UI thread.
pub fn write_editor(request: &EditorRequest, folder: &Path) -> Result<PathBuf, String> {
    std::fs::create_dir_all(folder)
        .map_err(|e| format!("couldn't make {}: {e}", folder.display()))?;
    let base64 = base64::engine::general_purpose::STANDARD;
    let image = |video: &Path| -> Result<String, String> {
        let png = frame_png(video, request.frame)?;
        Ok(format!("'data:image/png;base64,{}'", base64.encode(png)))
    };
    let calibration = std::fs::read_to_string(&request.calibration)
        .map_err(|e| format!("couldn't read {}: {e}", request.calibration.display()))?;
    // The path sits in a JavaScript string: escape what would end it.
    let path = request
        .calibration
        .display()
        .to_string()
        .replace('\\', "\\\\")
        .replace('\'', "\\'");
    let html = TEMPLATE
        .replace("'{{LEFT_IMAGE_DATA}}'", &image(&request.left)?)
        .replace("'{{RIGHT_IMAGE_DATA}}'", &image(&request.right)?)
        .replace("{{CAL_JSON}}", &calibration)
        .replace("'{{CAL_PATH}}'", &format!("'{path}'"));
    let page = folder.join("roi_editor.html");
    std::fs::write(&page, html).map_err(|e| format!("couldn't write {}: {e}", page.display()))?;
    Ok(page)
}

/// Frame `frame` of `video` as a PNG.
fn frame_png(video: &Path, frame: u64) -> Result<Vec<u8>, String> {
    use image::ImageEncoder;
    let frames = extract_frames(video, &[frame])
        .map_err(|e| format!("couldn't read {}: {e}", video.display()))?;
    let yuv = frames
        .first()
        .ok_or_else(|| format!("{} has no frame {frame}", video.display()))?;
    let (width, height) = (yuv.width as usize, yuv.height as usize);
    let mut rgb = vec![0u8; width * height * 3];
    for row in 0..height {
        for col in 0..width {
            let luma = f32::from(yuv.y[row * width + col]);
            let chroma = (row / 2) * (width / 2) + col / 2;
            let (u, v) = (
                f32::from(yuv.u[chroma]) - 128.0,
                f32::from(yuv.v[chroma]) - 128.0,
            );
            let at = (row * width + col) * 3;
            rgb[at] = (luma + 1.402 * v).clamp(0.0, 255.0) as u8;
            rgb[at + 1] = (luma - 0.344 * u - 0.714 * v).clamp(0.0, 255.0) as u8;
            rgb[at + 2] = (luma + 1.772 * u).clamp(0.0, 255.0) as u8;
        }
    }
    let mut png = Vec::new();
    image::codecs::png::PngEncoder::new(&mut png)
        .write_image(&rgb, yuv.width, yuv.height, image::ExtendedColorType::Rgb8)
        .map_err(|e| format!("couldn't encode the frame: {e}"))?;
    Ok(png)
}

/// The editor being written on its own thread.
pub struct EditorJob {
    result: Receiver<Result<PathBuf, String>>,
}

impl EditorJob {
    /// Write the editor for `request` into `folder`; `waker` runs when it
    /// is done.
    pub fn start(
        request: EditorRequest,
        folder: PathBuf,
        waker: Arc<dyn Fn() + Send + Sync>,
    ) -> Self {
        let (tx, result) = mpsc::channel();
        let spawned = std::thread::Builder::new()
            .name("reco-roi-editor".into())
            .spawn({
                let tx = tx.clone();
                let waker = Arc::clone(&waker);
                move || {
                    if tx.send(write_editor(&request, &folder)).is_ok() {
                        waker();
                    }
                }
            });
        if let Err(e) = spawned {
            let _ = tx.send(Err(format!("couldn't start: {e}")));
            waker();
        }
        Self { result }
    }

    /// The page, or why it couldn't be written, once done (never blocks).
    pub fn try_result(&self) -> Option<Result<PathBuf, String>> {
        self.result.try_recv().ok()
    }
}

/// Open `page` in the default browser; returns once the command started.
pub fn open_in_browser(page: &Path) -> std::io::Result<()> {
    let (program, args) = if cfg!(target_os = "macos") {
        ("open", vec![page.display().to_string()])
    } else if cfg!(target_os = "windows") {
        (
            "cmd",
            vec!["/c".into(), "start".into(), page.display().to_string()],
        )
    } else {
        ("xdg-open", vec![page.display().to_string()])
    };
    let mut child = Command::new(program).args(args).spawn()?;
    std::thread::spawn(move || child.wait());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::preview::fixtures;

    #[test]
    fn an_outline_needs_three_points_inside_the_picture() {
        let good = r#"{"left": [[0.1, 0.2], [0.9, 0.2], [0.5, 0.9]], "right": []}"#;
        assert_eq!(parse_outline(good).unwrap().left.len(), 3);
        assert!(parse_outline("{").is_err(), "not JSON");
        assert!(
            parse_outline(r#"{"left": [[0.1, 0.2]], "right": []}"#).is_err(),
            "too few points"
        );
        let outside = r#"{"left": [[0.1, 0.2], [1.4, 0.2], [0.5, 0.9]]}"#;
        assert!(parse_outline(outside).is_err(), "a point past the edge");
    }

    #[test]
    fn the_editor_folder_follows_the_cache_home() {
        let folder = editor_folder();
        assert!(folder.ends_with("reco/roi"), "{folder:?}");
    }

    #[test]
    fn the_editor_carries_both_frames_and_the_calibration() {
        let Some((left, right, calibration)) = fixtures::fast_set() else {
            return;
        };
        let folder = std::env::temp_dir().join(format!("reco-app-roi-{}", std::process::id()));
        let page = write_editor(
            &EditorRequest {
                left,
                right,
                frame: 30,
                calibration,
            },
            &folder,
        )
        .expect("the editor is written");
        let html = std::fs::read_to_string(&page).unwrap();
        assert_eq!(
            html.matches("data:image/png;base64,").count(),
            2,
            "both frames"
        );
        assert!(!html.contains("{{CAL_JSON}}") && html.contains("left_uniforms"));
        let _ = std::fs::remove_dir_all(&folder);
    }

    #[test]
    fn the_editor_job_reports_its_page() {
        let Some((left, right, calibration)) = fixtures::fast_set() else {
            return;
        };
        let folder = std::env::temp_dir().join(format!("reco-app-roi-job-{}", std::process::id()));
        let job = EditorJob::start(
            EditorRequest {
                left,
                right,
                frame: 0,
                calibration,
            },
            folder.clone(),
            Arc::new(|| {}),
        );
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
        let mut result = None;
        while result.is_none() && std::time::Instant::now() < deadline {
            result = job.try_result();
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        assert!(
            matches!(&result, Some(Ok(page)) if page.exists()),
            "{result:?}"
        );
        let _ = std::fs::remove_dir_all(&folder);
    }
}
