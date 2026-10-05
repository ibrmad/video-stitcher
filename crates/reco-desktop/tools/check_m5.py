#!/usr/bin/env python3
"""Module 5 check: camera and lens (PARITY.md, Module 5).

Run after `cargo build --profile desktop -p reco-desktop`. Opens the fast
fixture pair with a temporary copy of its calibration (so saving never
touches the fixture). Each launch gets its own settings folder.
Screenshots go to target/desktop-checks/m5/. `check_m5.py NAME...` runs the
named checks (all by default); exits non-zero if any check failed.
"""
import json
import os
import shutil
import sys
import tempfile
import time

import drive

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "..", ".."))
BIN = os.path.join(ROOT, "target", "desktop", "reco-desktop")
OUT = os.path.join(ROOT, "target", "desktop-checks", "m5")
HOME = os.path.expanduser("~")
FAST = (
    os.environ.get("RECO_FIXTURE_LEFT", f"{HOME}/dev/pitchcam-data/alfheim/cam0.mp4"),
    os.environ.get("RECO_FIXTURE_RIGHT", f"{HOME}/dev/pitchcam-data/alfheim/cam1.mp4"),
    os.environ.get("RECO_FIXTURE_CAL", f"{HOME}/dev/pitchcam-data/alfheim/reco/match.json"),
)

FAILURES = []


def expect(ok, message):
    print(f"{'ok' if ok else 'FAIL'}: {message}")
    if not ok:
        FAILURES.append(message)


def wait_for(probe, secs):
    deadline = time.monotonic() + secs
    while time.monotonic() < deadline:
        value = probe()
        if value:
            return value
        time.sleep(0.2)
    return None


def text_of(app, widget_id):
    """A label's or button's text, or a text field's value."""
    for item in app.snap(widget_id):
        if item.get("i") == widget_id:
            return item.get("t", item.get("val", ""))
    return None


def title_rect(app, text):
    for item in app.snap(text):
        if item.get("t") == text and item.get("i") == "title":
            return item["r"]
    return None


def click(app, widget_id):
    r = app.rect(widget_id)
    if r:
        app.get("/click", x=r[0] + r[2] / 2, y=r[1] + r[3] / 2, wait=1)
    return r is not None


def drag(app, slider_id, by):
    """Drag a slider by `by` of its width (Makepad's sliders move relative
    to where the knob was; more than 1 reaches an end); whether it was
    there."""
    r = app.rect(slider_id)
    if r is None:
        return False
    x, y, w, h = r
    start = x + w / 2
    app.get("/m", k="down", x=start, y=y + h / 2, wait=1)
    for step in range(1, 7):
        app.get("/m", k="move", x=start + w * by * step / 6, y=y + h / 2, wait=1)
    app.get("/m", k="up", x=start + w * by, y=y + h / 2, wait=1)
    return True


def open_advanced(app, fold_id):
    """Open an Advanced tier by its chevron."""
    r = app.rect(fold_id)
    if r:
        app.get("/click", x=r[0] + 18, y=r[1] + 12, wait=1)
        time.sleep(0.5)


def frame_pixels(app, name):
    png = app.grab(os.path.join(OUT, f"{name}.png"), scale=0.5)
    scale = png.width / app.get("/s")["w"][0]["sz"][0]
    x, y, w, h = app.rect("preview")
    return [png.pixel(int((x + i) * scale), int((y + j) * scale))[:3]
            for j in range(4, int(h) - 4, 16) for i in range(4, int(w) - 4, 16)]


def calibration_copy():
    folder = tempfile.mkdtemp(prefix="reco-m5-")
    cal = os.path.join(folder, "match.json")
    shutil.copyfile(FAST[2], cal)
    return (FAST[0], FAST[1], cal)


def launch(files):
    left, right, cal = files
    args = ["--window-size", "1280x980", "--left", left, "--right", right, "--calibration", cal]
    return drive.App.launch(BIN, args)


def degrees(text):
    return float((text or "nan°").rstrip("°"))


def check_view():
    """The field of view starts where the picture allows, follows its
    slider and the zoom keys; stay inside can be switched off; Reset view
    goes back."""
    files = calibration_copy()
    with launch(files) as app:
        expect(wait_for(lambda: app.rect("preview"), 30) is not None, "view: the preview opens")
        opening = wait_for(lambda: text_of(app, "fov_value") if text_of(app, "fov_value") != "75°" else None, 10)
        expect(opening is not None and degrees(opening) < 75,
               f"view: the opening field of view is what the picture allows ({opening})")
        before = frame_pixels(app, "view-before")
        # The first drag, with the Advanced tier under it closed (a closed
        # fold's hidden rows used to take it).
        drag(app, "fov_slider", -1.5)
        time.sleep(0.8)
        narrow = text_of(app, "fov_value")
        expect(degrees(narrow) == 20, f"view: the slider narrows the view ({narrow})")
        after = frame_pixels(app, "view-narrow")
        expect(before != after, "view: the picture follows")
        click(app, "preview")
        app.key("-")
        time.sleep(0.8)
        expect(degrees(text_of(app, "fov_value")) > degrees(narrow),
               f"view: the zoom keys move the slider ({narrow} -> {text_of(app, 'fov_value')})")
        drag(app, "fov_slider", 1.5)
        time.sleep(0.8)
        capped = degrees(text_of(app, "fov_value"))
        expect(capped < 150, f"view: staying inside caps the field of view ({capped:.0f}°)")
        open_advanced(app, "view_advanced")
        click(app, "constrained_look")
        time.sleep(0.5)
        drag(app, "fov_slider", 1.5)
        time.sleep(0.8)
        expect(degrees(text_of(app, "fov_value")) == 150,
               f"view: switched off, the whole range is free ({text_of(app, 'fov_value')})")
        click(app, "reset_view")
        time.sleep(0.8)
        expect(degrees(text_of(app, "fov_value")) == 75,
               f"view: Reset view goes back ({text_of(app, 'fov_value')})")
        expect(app.errors() == [], f"view: no errors in the app log {app.errors()[:3]}")


def check_lens():
    """Each camera's lens is named; a fine-tune slider moves the picture and
    marks the calibration unsaved; Reset lens goes back; lens correction is
    saved with the calibration."""
    files = calibration_copy()
    loaded = json.load(open(files[2]))
    with launch(files) as app:
        expect(wait_for(lambda: app.rect("preview"), 30) is not None, "lens: the preview opens")
        named = wait_for(lambda: text_of(app, "left_lens_name") not in (None, "", "Looking up…"), 20)
        expect(bool(named), f"lens: the left lens is named ({text_of(app, 'left_lens_name')})")
        expect(text_of(app, "right_lens_name") not in (None, "", "Looking up…"),
               f"lens: the right lens is named ({text_of(app, 'right_lens_name')})")
        open_advanced(app, "lens_advanced")
        fx = f"{loaded['left_uniforms']['fx']:.0f}"
        expect(text_of(app, "lens_fx_value") == fx, f"lens: fine-tune shows the file's focal length ({text_of(app, 'lens_fx_value')} vs {fx})")
        expect(app.enabled("reset_lens") is False, "lens: nothing to reset yet")
        pick_row(app, "lens_camera", 2)
        before = frame_pixels(app, "lens-before")
        drag(app, "lens_fx", 0.4)
        time.sleep(0.8)
        moved = text_of(app, "lens_fx_value")
        expect(moved not in (None, fx), f"lens: the focal length follows its slider ({moved})")
        after = frame_pixels(app, "lens-after")
        expect(before != after, "lens: the picture follows")
        expect(wait_for(lambda: app.enabled("reset_lens"), 5) is True, "lens: Reset lens can go back")
        expect(bool(wait_for(lambda: app.rect("save_calibration"), 5)), "lens: Save appears")
        click(app, "reset_lens")
        time.sleep(0.8)
        expect(text_of(app, "lens_fx_value") == fx, f"lens: Reset lens restores the file's lens ({text_of(app, 'lens_fx_value')})")
        expect(wait_for(lambda: app.enabled("reset_lens") is False, 5), "lens: and has nothing left to reset")
        click(app, "lens_correction")
        time.sleep(0.8)
        flat = frame_pixels(app, "lens-uncorrected")
        expect(flat != before, "lens: switching correction off changes the picture")
        click(app, "save_calibration")
        expect(bool(wait_for(lambda: title_rect(app, "Calibration saved"), 10)), "lens: saved")
        written = json.load(open(files[2]))
        expect(written.get("lens_correction_amount") == 0.0,
               f"lens: the file keeps correction off ({written.get('lens_correction_amount')})")
        expect(app.errors() == [], f"lens: no errors in the app log {app.errors()[:3]}")


def pick_row(app, dropdown_id, index):
    """Choose row `index` of a dropdown with the keyboard (menu rows are
    not in the snapshot)."""
    click(app, dropdown_id)
    time.sleep(0.3)
    # The menu opens on the current row: go to the top first.
    for _ in range(4):
        app.key("up")
    for _ in range(index):
        app.key("down")
    app.key("return")
    time.sleep(0.8)


def check_preview():
    """Show puts one camera in the preview, flat and inside the frame; the
    side matters; Stitched picture goes back."""
    files = calibration_copy()
    with launch(files) as app:
        expect(wait_for(lambda: app.rect("preview"), 30) is not None, "preview: the preview opens")
        time.sleep(1.0)
        stitched = frame_pixels(app, "preview-stitched")
        pick_row(app, "lens_preview", 1)
        left = frame_pixels(app, "preview-left")
        expect(left != stitched, "preview: Left camera shows one camera")
        x, y, w, h = app.rect("preview")
        png = app.grab(os.path.join(OUT, "preview-left-full.png"), scale=0.5)
        scale = png.width / app.get("/s")["w"][0]["sz"][0]
        edge = [png.pixel(int((x + 3) * scale), int((y + j) * scale))[:3] for j in range(10, int(h) - 10, 20)]
        expect(all(max(p) < 16 for p in edge), "preview: the camera keeps its shape, black at the sides")
        pick_row(app, "lens_preview", 2)
        right = frame_pixels(app, "preview-right")
        expect(right != left, "preview: Right camera shows the other one")
        pick_row(app, "lens_preview", 0)
        back = frame_pixels(app, "preview-back")
        expect(back == stitched, "preview: Stitched picture goes back")
        expect(app.errors() == [], f"preview: no errors in the app log {app.errors()[:3]}")


CHECKS = {"view": check_view, "lens": check_lens, "preview": check_preview}


def main():
    os.makedirs(OUT, exist_ok=True)
    names = sys.argv[1:] or list(CHECKS)
    for name in names:
        CHECKS[name]()
    if FAILURES:
        print(f"\n{len(FAILURES)} check(s) failed")
        sys.exit(1)
    print(f"Module 5 check passed. Screenshots: {OUT}")


if __name__ == "__main__":
    main()
