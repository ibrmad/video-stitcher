#!/usr/bin/env python3
"""Module 4 check: stitching and calibration (PARITY.md, Module 4).

Run after `cargo build --profile desktop -p reco-desktop`. Opens the fast
fixture pair with a temporary copy of its calibration (so saving never
touches the fixture), tunes the stitch in the Adjust panel, saves, and
reopens. Each launch gets its own settings folder (drive.launch_env).
Screenshots go to target/desktop-checks/m4/. `check_m4.py NAME...` runs the
named checks (all by default); exits non-zero if any check failed.
"""
import json
import math
import os
import shutil
import sys
import tempfile
import time

import drive

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "..", ".."))
BIN = os.path.join(ROOT, "target", "desktop", "reco-desktop")
OUT = os.path.join(ROOT, "target", "desktop-checks", "m4")
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


def ready(app):
    """Wait for the stitched preview; its rect, or None."""
    return wait_for(lambda: app.rect("preview"), 30)


def text_of(app, widget_id):
    for item in app.snap(widget_id):
        if item.get("i") == widget_id:
            return item.get("t", "")
    return None


def save_shot(app, name):
    """Save a screenshot without decoding it (decoding takes seconds)."""
    shutil.copyfile(app.get("/g", scale=1.0)["png"], os.path.join(OUT, f"{name}.png"))


def title_rect(app, text):
    """The rect of a toast title showing `text`, or None."""
    for item in app.snap(text):
        if item.get("t") == text and item.get("i") == "title":
            return item["r"]
    return None


def click(app, widget_id):
    """Click a widget if it is on screen; whether it was."""
    r = app.rect(widget_id)
    if r:
        app.get("/click", x=r[0] + r[2] / 2, y=r[1] + r[3] / 2, wait=1)
    return r is not None


def launch(files=None, extra=(), config_dir=None, answers_file=None, more_env=None):
    args = ["--window-size", "1280x820"]
    if files:
        left, right, cal = files
        args += ["--left", left, "--right", right, "--calibration", cal]
    env = {}
    if config_dir:
        env["RECO_CONFIG_DIR"] = config_dir
    if answers_file:
        env["RECO_DESKTOP_DIALOG_ANSWERS"] = answers_file
    env.update(more_env or {})
    return drive.App.launch(BIN, [*args, *extra], env=env or None)


def calibration_copy():
    """The fast pair with a calibration copied into a temporary folder."""
    folder = tempfile.mkdtemp(prefix="reco-m4-")
    cal = os.path.join(folder, "match.json")
    shutil.copyfile(FAST[2], cal)
    return (FAST[0], FAST[1], cal)


def frame_pixels(app, name):
    png = app.grab(os.path.join(OUT, f"{name}.png"), scale=0.5)
    scale = png.width / app.get("/s")["w"][0]["sz"][0]
    x, y, w, h = app.rect("preview")
    return [png.pixel(int((x + i) * scale), int((y + j) * scale))[:3]
            for j in range(4, int(h) - 4, 16) for i in range(4, int(w) - 4, 16)]


def slide(app, slider_id, fraction):
    """Drag a slider's knob to `fraction` of its track; whether it was there."""
    r = app.rect(slider_id)
    if r is None:
        return False
    x, y, w, h = r
    app.get("/m", k="down", x=x + w / 2, y=y + h / 2)
    for step in range(1, 5):
        app.get("/m", k="move", x=x + w / 2 + (w * fraction - w / 2) * step / 4, y=y + h / 2, wait=1)
    app.get("/m", k="up", x=x + w * fraction, y=y + h / 2, wait=1)
    return True


def open_advanced(app, fold_id):
    """Open an Advanced tier by its chevron."""
    r = app.rect(fold_id)
    if r:
        app.get("/click", x=r[0] + 18, y=r[1] + 12, wait=1)
        time.sleep(0.5)


def check_tune():
    """The Adjust panel shows the calibration's values, tunes the picture
    live, and its title row offers Save while something is unsaved (⌘S
    too) and writes them; a reopen shows them again."""
    files = calibration_copy()
    loaded = json.load(open(files[2]))
    with launch(files) as app:
        expect(ready(app) is not None, "tune: the preview opens")
        time.sleep(1.0)
        blend = text_of(app, "seam_value")
        expect(blend == f"{loaded.get('blend_width', 0.05):.2f}",
               f"tune: the seam blend shows the calibration's value ({blend})")
        expect(app.rect("save_calibration") is None, "tune: nothing to save yet")
        # Straight ahead the render cancels the tilt: turn the view first.
        for _ in range(8):
            app.key("ArrowRight")
        time.sleep(1.0)
        before = frame_pixels(app, "tune-before")
        open_advanced(app, "stitch_advanced")
        slide(app, "rig_tilt", 0.8)
        tilt = text_of(app, "tilt_value") or ""
        expect(tilt.endswith("°") and tilt != "0.0°", f"tune: the tilt follows the slider ({tilt})")
        time.sleep(0.6)
        after = frame_pixels(app, "tune-after")
        expect(before != after, "tune: tilting changes the picture")
        saving = wait_for(lambda: app.rect("save_calibration"), 5)
        expect(bool(saving), "tune: Save appears once something changed")
        panel = app.rect("inspector")
        expect(bool(saving) and panel is not None and panel[0] <= saving[0] and saving[1] < panel[1] + 28,
               f"tune: in the Adjust panel's title row, beside the tuning ({saving} in {panel})")
        expect(app.rect("calibration_unsaved") is not None, "tune: the title row says Unsaved")
        slide(app, "intersect", 0.3)
        expect(text_of(app, "intersect_value") not in (None, "", f"{loaded['params']['intersect']:.3f}"),
               f"tune: the overlap follows its slider ({text_of(app, 'intersect_value')})")
        click(app, "reset_layout")
        time.sleep(0.5)
        expect(text_of(app, "intersect_value") == f"{loaded['params']['intersect']:.3f}",
               f"tune: Reset layout restores the file's overlap ({text_of(app, 'intersect_value')})")
        app.key("KeyS", cmd=1)
        saved = wait_for(lambda: title_rect(app, "Calibration saved"), 10)
        expect(bool(saved), "tune: ⌘S saves it, and a toast says so")
        expect(bool(wait_for(lambda: app.rect("save_calibration") is None, 5)), "tune: Save goes once saved")
        written = json.load(open(files[2]))
        expect(abs(math.degrees(written["rig_tilt"]) - float(tilt.rstrip("°"))) < 0.06,
               f"tune: the file has the new tilt ({math.degrees(written['rig_tilt']):.2f} vs {tilt})")
        expect(app.errors() == [], "tune: no errors in the app log")
    with launch(files) as app:
        ready(app)
        time.sleep(1.0)
        open_advanced(app, "stitch_advanced")
        expect(text_of(app, "tilt_value") == tilt, f"tune: the tilt comes back after a reopen ({text_of(app, 'tilt_value')})")


def check_sync():
    """The sync offset in frames: Apply moves the right camera and the
    length follows; a value as long as the videos is refused."""
    files = calibration_copy()
    with launch(files) as app:
        expect(ready(app) is not None, "sync: the preview opens")
        wait_for(lambda: text_of(app, "time_total") == "1:00", 15)
        open_advanced(app, "stitch_advanced")
        click(app, "sync_input")
        app.key("KeyA", cmd=1)
        app.get("/k", t="30")
        click(app, "sync_apply")
        shorter = wait_for(lambda: text_of(app, "time_total") == "0:59", 20)
        expect(bool(shorter), f"sync: 30 frames apart, the pair plays a second less ({text_of(app, 'time_total')})")
        expect(bool(wait_for(lambda: app.rect("save_calibration"), 5)), "sync: Save appears")
        click(app, "sync_input")
        app.key("KeyA", cmd=1)
        app.get("/k", t="999999")
        click(app, "sync_apply")
        refused = wait_for(lambda: title_rect(app, "Couldn't change the sync offset"), 10)
        expect(bool(refused), "sync: an offset as long as the videos is refused")
        click(app, "sync_input")
        app.key("KeyA", cmd=1)
        app.get("/k", t="soon")
        click(app, "sync_apply")
        expect(bool(wait_for(lambda: title_rect(app, "Sync offset"), 5)), "sync: text that is not a number says so")
        save_shot(app, "sync")


OUTLINE = '{"left": [[0.1, 0.4], [0.9, 0.4], [0.95, 0.95], [0.05, 0.95]], "right": [[0.1, 0.4], [0.9, 0.4], [0.5, 0.95]]}'


def check_roi():
    """The field outline: the browser editor is written (never opened in
    checks), a pasted outline is used and saved, bad text says why, and
    Remove outline clears it."""
    files = calibration_copy()
    cache = tempfile.mkdtemp(prefix="reco-m4-cache-")
    env = {"XDG_CACHE_HOME": cache, "RECO_DESKTOP_NO_BROWSER": "1"}
    with launch(files, more_env=env) as app:
        expect(ready(app) is not None, "roi: the preview opens")
        open_advanced(app, "field_outline")
        expect(text_of(app, "roi_status") == "None", f"roi: no outline yet ({text_of(app, 'roi_status')})")
        click(app, "roi_edit")
        opened = wait_for(lambda: title_rect(app, "Outline editor ready"), 20)
        expect(bool(opened), "roi: Edit in browser writes the editor")
        page = os.path.join(cache, "reco", "roi", "roi_editor.html")
        html = open(page).read() if os.path.exists(page) else ""
        expect(html.count("data:image/png;base64,") == 2, "roi: the editor carries both cameras' frames")
        click(app, "roi_json")
        app.get("/k", t="not an outline")
        click(app, "roi_use")
        expect(bool(wait_for(lambda: title_rect(app, "That isn't a field outline"), 5)), "roi: bad text says why")
        click(app, "roi_json")
        app.key("KeyA", cmd=1)
        app.get("/k", t=OUTLINE)
        click(app, "roi_use")
        used = wait_for(lambda: text_of(app, "roi_status") == "7 points", 5)
        expect(bool(used), f"roi: a pasted outline is used ({text_of(app, 'roi_status')})")
        expect(bool(wait_for(lambda: app.rect("save_calibration"), 5)), "roi: Save appears")
        click(app, "save_calibration")
        wait_for(lambda: title_rect(app, "Calibration saved"), 10)
        roi = json.load(open(files[2])).get("field_roi") or {}
        expect(len(roi.get("left", [])) == 4 and len(roi.get("right", [])) == 3,
               f"roi: the outline is saved with the calibration ({roi})")
        click(app, "roi_clear")
        cleared = wait_for(lambda: text_of(app, "roi_status") == "None", 5)
        expect(bool(cleared), f"roi: Remove outline clears it ({text_of(app, 'roi_status')})")
        save_shot(app, "roi")
        expect(app.errors() == [], "roi: no errors in the app log")


CHECKS = {
    "tune": check_tune,
    "sync": check_sync,
    "roi": check_roi,
}


def main():
    if not all(os.path.exists(p) for p in FAST):
        print("skip: fixtures not found (set RECO_FIXTURE_LEFT/RIGHT/CAL)")
        return
    os.makedirs(OUT, exist_ok=True)
    names = sys.argv[1:] or list(CHECKS)
    for name in names:
        CHECKS[name]()
    if FAILURES:
        print(f"\nModule 4 check FAILED: {len(FAILURES)} failure(s):")
        for message in FAILURES:
            print(f"  - {message}")
        sys.exit(1)
    print(f"Module 4 check passed. Screenshots: {OUT}")


if __name__ == "__main__":
    main()
