#!/usr/bin/env python3
"""Files and calibration check.

Run after `cargo build --profile desktop -p reco-desktop`. Starts the app
without files and drives it through Makepad's --remote control. Native file
dialogs are answered from RECO_DESKTOP_DIALOG_ANSWERS (a JSON file the
check writes), through the same path a real pick takes. Videos are linked
into temporary folders, so a saved calibration never lands beside the
fixtures. Each launch gets its own settings folder (drive.launch_env).
Screenshots go to target/desktop-checks/files/. `check_files.py NAME...` runs the
named checks (all by default); exits non-zero if any check failed. The
`calibrate` check also calibrates the real match pair (under a minute in the
desktop build) unless RECO_CHECK_SKIP_REAL is set.
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
OUT = os.path.join(ROOT, "target", "desktop-checks", "files")
HOME = os.path.expanduser("~")
FAST = (
    os.environ.get("RECO_FIXTURE_LEFT", f"{HOME}/dev/pitchcam-data/alfheim/cam0.mp4"),
    os.environ.get("RECO_FIXTURE_RIGHT", f"{HOME}/dev/pitchcam-data/alfheim/cam1.mp4"),
    os.environ.get("RECO_FIXTURE_CAL", f"{HOME}/dev/pitchcam-data/alfheim/reco/match.json"),
)
REAL = (
    f"{HOME}/Downloads/match_recording/left/GX010120.MP4",
    f"{HOME}/Downloads/match_recording/right/GX010092.MP4",
    f"{HOME}/Downloads/match_recording/left/GX010120_calibration.json",
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


def linked(name, files=FAST, names=("cam0.mp4", "cam1.mp4"), calibration=False):
    """A fresh folder with the pair linked in (and its calibration copied
    beside the left video as `{stem}_calibration.json` when asked)."""
    folder = tempfile.mkdtemp(prefix=f"reco-files-{name}-")
    for source, link in zip(files[:2], names):
        os.symlink(source, os.path.join(folder, link))
    if calibration:
        stem = os.path.splitext(names[0])[0]
        shutil.copyfile(files[2], os.path.join(folder, f"{stem}_calibration.json"))
    return folder


def answers(**picks):
    """A dialog answers file: each dialog ("left", "right", "calibration")
    answers with its list of paths."""
    fd, path = tempfile.mkstemp(prefix="reco-files-answers-", suffix=".json")
    with os.fdopen(fd, "w") as f:
        json.dump(picks, f)
    return path


def launch(files=None, extra=(), config_dir=None, answers_file=None, env=None):
    args = ["--window-size", "1280x820"]
    if files:
        left, right, cal = files
        args += ["--left", left, "--right", right, "--calibration", cal]
    env = dict(env or {})
    if config_dir:
        env["RECO_CONFIG_DIR"] = config_dir
    if answers_file:
        env["RECO_DESKTOP_DIALOG_ANSWERS"] = answers_file
    return drive.App.launch(BIN, [*args, *extra], env=env or None)


def check_files():
    """Add each camera through its dialog: counts and lengths, the title,
    the next step; a calibration saved beside the left video loads."""
    folder = linked("files")
    picks = answers(left=[f"{folder}/cam0.mp4"], right=[f"{folder}/cam1.mp4"])
    with launch(answers_file=picks) as app:
        start = wait_for(lambda: text_of(app, "next_title") == "Add your two camera videos", 15)
        expect(bool(start), f"files: an empty start asks for the videos ({text_of(app, 'next_title')})")
        expect(text_of(app, "project_name") == "No videos yet", f"files: no title yet ({text_of(app, 'project_name')})")
        app.click_id("add_left")
        left = wait_for(lambda: text_of(app, "left_files") == "1 file · 1:00", 15)
        expect(bool(left), f"files: the left camera shows its file and length ({text_of(app, 'left_files')})")
        expect(text_of(app, "next_title") == "Now add the right camera",
               f"files: the next step asks for the right camera ({text_of(app, 'next_title')})")
        expect(text_of(app, "project_name") == "cam0.mp4", f"files: the title names the file ({text_of(app, 'project_name')})")
        expect(bool(wait_for(lambda: app.rect("lanes"), 5)), "files: the lanes show once a camera has video")
        app.click_id("next_primary")
        right = wait_for(lambda: text_of(app, "right_files") == "1 file · 1:00", 15)
        expect(bool(right), f"files: the next-step card adds the right camera ({text_of(app, 'right_files')})")
        expect(text_of(app, "next_title") == "Line up the two cameras",
               f"files: then it asks to calibrate ({text_of(app, 'next_title')})")
        expect(text_of(app, "calibration_detail") == "Ready to calibrate",
               f"files: calibration is ready to run ({text_of(app, 'calibration_detail')})")
        expect(app.enabled("auto_calibrate"), "files: Auto-calibrate is enabled")
        expect(app.rect("preview") is None, "files: no preview without a calibration")
        save_shot(app, "files-two-cameras")
        expect(app.errors() == [], "files: no errors in the app log")

    folder = linked("sibling", calibration=True)
    picks = answers(left=[f"{folder}/cam0.mp4"], right=[f"{folder}/cam1.mp4"])
    with launch(answers_file=picks) as app:
        wait_for(lambda: text_of(app, "next_title") == "Add your two camera videos", 15)
        app.click_id("add_left")
        wait_for(lambda: text_of(app, "left_files") == "1 file · 1:00", 15)
        app.click_id("add_right")
        expect(ready(app) is not None, "files: a calibration saved beside the left video loads, and the preview opens")
        expect(text_of(app, "calibration_status") == "Calibrated",
               f"files: the calibration shows as done ({text_of(app, 'calibration_status')})")
        expect(text_of(app, "calibration_detail") == "cam0_calibration.json",
               f"files: and names its file ({text_of(app, 'calibration_detail')})")
        expect(title_rect(app, "Found a saved calibration") is not None, "files: a toast says it was found")
        save_shot(app, "files-ready")
        expect(app.errors() == [], "files: no errors in the app log")


def rows_of(app, prefix):
    """The file list rows whose name starts with `prefix`, top to bottom:
    (name, rect)."""
    rows = [(i.get("t", ""), i["r"]) for i in app.snap(prefix) if i.get("i") == "name"]
    return sorted(rows, key=lambda row: row[1][1])


def names_of(app, prefix):
    return [name for name, _ in rows_of(app, prefix)]


def check_list():
    """A camera's files: recording order, remove, drag to reorder, keys,
    Remove all."""
    folder = tempfile.mkdtemp(prefix="reco-files-list-")
    for name in ("GX030001.MP4", "GX010001.MP4", "GX020001.MP4"):
        os.symlink(FAST[0], os.path.join(folder, name))
    os.symlink(FAST[1], os.path.join(folder, "GX010002.MP4"))
    picks = answers(left=[f"{folder}/{n}" for n in ("GX030001.MP4", "GX010001.MP4", "GX020001.MP4")],
                    right=[f"{folder}/GX010002.MP4"])
    with launch(answers_file=picks, env={"RECO_DESKTOP_LOG_CURSOR": "1"}) as app:
        wait_for(lambda: text_of(app, "next_title") == "Add your two camera videos", 15)
        app.click_id("add_right")
        app.click_id("add_left")
        three = wait_for(lambda: text_of(app, "left_files") == "3 files · 3:00", 15)
        expect(bool(three), f"list: three files and their length ({text_of(app, 'left_files')})")
        expect(click(app, "left_fold"), "list: a camera with files has a chevron for its list")
        shown = wait_for(lambda: len(rows_of(app, "GX0")) >= 3, 5)
        expect(bool(shown), f"list: the chevron shows the camera's files ({len(rows_of(app, 'GX0'))} rows)")
        left = [n for n in names_of(app, "GX0") if n.endswith("0001.MP4")]
        expect(left == ["GX010001.MP4", "GX020001.MP4", "GX030001.MP4"],
               f"list: the left camera's files show in recording order ({left})")
        save_shot(app, "list-open")
        # The rows' grab hand stays: the sliders' arrow is for sliders only.
        lx, ly, lw, lh = app.rect("left_list")
        for step in range(5):
            app.get("/m", k="move", x=lx + lw * (step + 1) / 6, y=ly + 12, wait=1)
        cursors = {line.split("cursor: ")[1].strip() for line in app.log_lines() if "cursor: " in line}
        expect("Grab" in cursors, f"list: the rows show the grab hand ({sorted(cursors)})")
        removes = sorted((i["r"] for i in app.snap("remove") if i.get("i") == "remove"), key=lambda r: r[1])
        if len(removes) >= 2:
            r = removes[1]
            app.get("/click", x=r[0] + r[2] / 2, y=r[1] + r[3] / 2, wait=1)
        gone = wait_for(lambda: text_of(app, "left_files") == "2 files · 2:00", 10)
        expect(bool(gone), f"list: a row's remove button removes its file ({text_of(app, 'left_files')})")
        left = [n for n in names_of(app, "GX0") if n.endswith("0001.MP4")]
        expect(left == ["GX010001.MP4", "GX030001.MP4"], f"list: the others stay in order ({left})")
        rows = rows_of(app, "GX0")
        if len(rows) >= 2:
            x, y, w, h = app.rect("left_list")
            app.get("/m", k="down", x=x + w / 3, y=y + 12)
            for step in range(1, 6):
                app.get("/m", k="move", x=x + w / 3, y=y + 12 + 36 * step / 5, wait=1)
            app.get("/m", k="up", x=x + w / 3, y=y + 48, wait=1)
        moved = wait_for(lambda: [n for n in names_of(app, "GX0") if n.endswith("0001.MP4")]
                         == ["GX030001.MP4", "GX010001.MP4"], 5)
        expect(bool(moved), f"list: dragging a row moves its file ({names_of(app, 'GX0')})")
        app.key("up", alt=1)
        back = wait_for(lambda: [n for n in names_of(app, "GX0") if n.endswith("0001.MP4")]
                        == ["GX010001.MP4", "GX030001.MP4"], 5)
        expect(bool(back), f"list: Alt+Up moves the selected file back ({names_of(app, 'GX0')})")
        app.click_id("clear_left")
        cleared = wait_for(lambda: text_of(app, "next_title") == "Now add the left camera", 10)
        expect(bool(cleared), f"list: Remove all empties the camera ({text_of(app, 'next_title')})")
        expect(app.errors() == [], "list: no errors in the app log")


def check_calibrate():
    """Auto-calibrate: its steps, Cancel, a failure that says what to do,
    Load file and the file's remove button; the real pair calibrates, saves
    beside its left video and opens the preview."""
    folder = linked("calibrate")
    picks = answers(left=[f"{folder}/cam0.mp4"], right=[f"{folder}/cam1.mp4"], calibration=[FAST[2]])
    with launch(answers_file=picks) as app:
        wait_for(lambda: text_of(app, "next_title") == "Add your two camera videos", 15)
        app.click_id("add_left")
        app.click_id("add_right")
        wait_for(lambda: text_of(app, "next_title") == "Line up the two cameras", 15)
        app.click_id("next_primary")
        step = wait_for(lambda: (text_of(app, "calibration_detail") or "").startswith("Step "), 30)
        expect(bool(step), f"calibrate: the steps show ({text_of(app, 'calibration_detail')})")
        expect(text_of(app, "next_title") == "Calibrating…", f"calibrate: the card says so ({text_of(app, 'next_title')})")
        expect((text_of(app, "status_text") or "").startswith("Calibrating · step"),
               f"calibrate: the status line counts the steps ({text_of(app, 'status_text')})")
        expect(not app.enabled("load_calibration"), "calibrate: Load file waits while it runs")
        save_shot(app, "calibrating")
        expect(click(app, "calibrate_cancel"), "calibrate: the card offers Cancel")
        stopped = wait_for(lambda: text_of(app, "calibration_status") == "Not calibrated", 30)
        expect(bool(stopped), f"calibrate: the card's Cancel stops it ({text_of(app, 'calibration_status')})")
        # Advanced opens by its chevron; its options lock while a run goes.
        advanced = app.rect("calibration_advanced")
        if advanced:
            app.get("/click", x=advanced[0] + 18, y=advanced[1] + 12, wait=1)
        wait_for(lambda: app.enabled("cal_frames") is not None, 3)
        # Their values can be typed, as every slider's.
        click(app, "cal_skip_end_value")
        time.sleep(0.3)
        app.get("/k", t="12")
        app.key("return")
        time.sleep(0.4)
        expect(text_of(app, "cal_skip_end_value") == "12 s",
               f"calibrate: a typed time to skip ({text_of(app, 'cal_skip_end_value')})")
        app.click_id("auto_calibrate")
        locked = wait_for(lambda: app.enabled("cal_frames") is False, 5)
        expect(bool(locked), f"calibrate: the Advanced options lock while it runs ({app.enabled('cal_frames')})")
        expect(app.enabled("cal_skip_end_value") is False,
               f"calibrate: their value fields too ({app.enabled('cal_skip_end_value')})")
        expect(click(app, "cancel_calibration"), "calibrate: the Setup panel offers Cancel")
        stopped = wait_for(lambda: text_of(app, "calibration_status") == "Not calibrated", 30)
        expect(bool(stopped), f"calibrate: the Setup panel's Cancel stops it ({text_of(app, 'calibration_status')})")
        expect(app.enabled("cal_frames") is True, f"calibrate: and unlocks them ({app.enabled('cal_frames')})")
        expect(app.enabled("cal_skip_end_value") is True,
               f"calibrate: the value fields too ({app.enabled('cal_skip_end_value')})")
        app.click_id("auto_calibrate")
        failed = wait_for(lambda: text_of(app, "next_title") == "Calibration didn't work", 180)
        expect(bool(failed), f"calibrate: footage with no matches fails plainly ({text_of(app, 'next_title')})")
        expect(text_of(app, "calibration_status") == "Calibration failed",
               f"calibrate: the Setup panel says so ({text_of(app, 'calibration_status')})")
        expect(title_rect(app, "Calibration failed") is not None, "calibrate: an error toast says so")
        expect(not os.path.exists(os.path.join(folder, "cam0_calibration.json")), "calibrate: nothing is saved")
        save_shot(app, "calibration-failed")
        app.click_id("next_secondary")
        expect(ready(app) is not None, "calibrate: Load calibration file opens the preview")
        expect(text_of(app, "calibration_detail") == "match.json",
               f"calibrate: the file is named ({text_of(app, 'calibration_detail')})")
        expect(click(app, "clear_calibration"), "calibrate: the calibration has a remove button")
        closed = wait_for(lambda: app.rect("preview") is None, 10)
        expect(bool(closed), "calibrate: removing it closes the preview")
        expect(text_of(app, "calibration_status") == "Not calibrated",
               f"calibrate: and it is gone ({text_of(app, 'calibration_status')})")
        expect(app.errors() == [], "calibrate: no errors in the app log")

    if os.environ.get("RECO_CHECK_SKIP_REAL"):
        print("skip: calibrate: the real pair (RECO_CHECK_SKIP_REAL is set)")
        return
    if not all(os.path.exists(p) for p in REAL):
        print("skip: calibrate: the real match pair was not found")
        return
    folder = linked("calibrate-real", files=REAL, names=("GX010120.MP4", "GX010092.MP4"))
    picks = answers(left=[f"{folder}/GX010120.MP4"], right=[f"{folder}/GX010092.MP4"])
    with launch(answers_file=picks) as app:
        wait_for(lambda: text_of(app, "next_title") == "Add your two camera videos", 15)
        app.click_id("add_left")
        app.click_id("add_right")
        wait_for(lambda: text_of(app, "next_title") == "Line up the two cameras", 30)
        started = time.monotonic()
        app.click_id("auto_calibrate")
        done = wait_for(lambda: text_of(app, "calibration_status") in ("Calibrated", "Calibration failed"), 600)
        took = time.monotonic() - started
        expect(text_of(app, "calibration_status") == "Calibrated",
               f"calibrate: the real pair calibrates in {took:.0f} s ({text_of(app, 'calibration_detail')})")
        saved = os.path.join(folder, "GX010120_calibration.json")
        expect(os.path.exists(saved), "calibrate: the calibration is saved beside the left video")
        expect(ready(app) is not None, "calibrate: and the preview opens with it")
        expect(title_rect(app, "Calibrated") is not None, "calibrate: a toast says so")
        save_shot(app, "calibrated-real")
        expect(app.errors() == [], "calibrate: no errors in the app log")


def recent_sessions(config):
    path = os.path.join(config, "desktop.json")
    return json.load(open(path)).get("recent", []) if os.path.exists(path) else []


def check_recent():
    """An opened pair is remembered; the next-step card's Recent files
    restores it, cameras and calibration; Clear recent files forgets."""
    config = tempfile.mkdtemp(prefix="reco-desktop-config-")
    folder = linked("recent", calibration=True)
    picks = answers(left=[f"{folder}/cam0.mp4"], right=[f"{folder}/cam1.mp4"])
    with launch(answers_file=picks, config_dir=config) as app:
        wait_for(lambda: text_of(app, "next_title") == "Add your two camera videos", 15)
        expect(app.rect("next_secondary") is None, "recent: no Recent files button before anything was opened")
        app.click_id("add_left")
        app.click_id("add_right")
        expect(ready(app) is not None, "recent: the pair opens")
    sessions = recent_sessions(config)
    expect(len(sessions) == 1 and sessions[0].get("left") == [f"{folder}/cam0.mp4"]
           and sessions[0].get("calibration") == f"{folder}/cam0_calibration.json",
           f"recent: the session is remembered ({sessions})")
    with launch(config_dir=config) as app:
        wait_for(lambda: text_of(app, "next_title") == "Add your two camera videos", 15)
        expect(text_of(app, "next_secondary") == "Recent files…",
               f"recent: the card offers Recent files ({text_of(app, 'next_secondary')})")
        click(app, "next_secondary")
        opened = wait_for(lambda: app.rect("recent_menu_list"), 5)
        expect(bool(opened), "recent: Recent files opens the menu")
        app.key("down")
        app.key("return")
        expect(ready(app) is not None, "recent: picking the session opens it again")
        expect(text_of(app, "left_files") == "1 file · 1:00" and text_of(app, "calibration_status") == "Calibrated",
               f"recent: with both cameras and the calibration ({text_of(app, 'left_files')}, {text_of(app, 'calibration_status')})")
        click(app, "recent_menu")
        wait_for(lambda: app.rect("recent_menu_list"), 5)
        app.key("up")
        app.key("return")
        time.sleep(0.5)
        expect(app.errors() == [], "recent: no errors in the app log")
    expect(recent_sessions(config) == [], f"recent: Clear recent files forgets them ({recent_sessions(config)})")


CHECKS = {
    "files": check_files,
    "list": check_list,
    "calibrate": check_calibrate,
    "recent": check_recent,
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
        print(f"\nFiles check FAILED: {len(FAILURES)} failure(s):")
        for message in FAILURES:
            print(f"  - {message}")
        sys.exit(1)
    print(f"Files check passed. Screenshots: {OUT}")


if __name__ == "__main__":
    main()
