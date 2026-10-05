#!/usr/bin/env python3
"""Module 7 check: preferences and help (PARITY.md, Module 7).

Run after `cargo build --profile desktop -p reco-desktop`. Each launch gets
its own settings folder and no network (drive.launch_env): requests are log
lines, and RECO_DESKTOP_FAKE_RELEASE stands in for GitHub's latest release.
Dialogs are answered through RECO_DESKTOP_DIALOG_ANSWERS. Screenshots go to
target/desktop-checks/m7/. `check_m7.py NAME...` runs the named checks (all
by default); exits non-zero if any failed.
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
OUT = os.path.join(ROOT, "target", "desktop-checks", "m7")
HOME = os.path.expanduser("~")
FAST = (
    os.environ.get("RECO_FIXTURE_LEFT", f"{HOME}/dev/pitchcam-data/alfheim/cam0.mp4"),
    os.environ.get("RECO_FIXTURE_RIGHT", f"{HOME}/dev/pitchcam-data/alfheim/cam1.mp4"),
    os.environ.get("RECO_FIXTURE_CAL", f"{HOME}/dev/pitchcam-data/alfheim/reco/match.json"),
)

# The app menu's rows, as the keyboard reaches them.
SHORTCUTS, PREFERENCES, REPORT_BUG = 1, 2, 3

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


def item(app, widget_id):
    for found in app.snap(widget_id):
        if found.get("i") == widget_id:
            return found
    return None


def text_of(app, widget_id):
    """A label's, button's or dropdown's text, or a text field's value."""
    found = item(app, widget_id)
    return None if found is None else found.get("t", found.get("val", ""))


def checked(app, widget_id):
    found = item(app, widget_id)
    return None if found is None else bool(found.get("c"))


def save_shot(app, name):
    shutil.copyfile(app.get("/g", scale=1.0)["png"], os.path.join(OUT, f"{name}.png"))


def click(app, widget_id):
    """Click a widget if it is on screen; whether it was."""
    r = app.rect(widget_id)
    if r:
        app.get("/click", x=r[0] + r[2] / 2, y=r[1] + r[3] / 2, wait=1)
    return r is not None


def type_into(app, widget_id, text):
    """Replace a text field's text with `text`."""
    click(app, widget_id)
    app.key("KeyA", cmd=1)
    if text:
        app.get("/k", t=text)
    else:
        app.key("backspace")


def logged(app, needle):
    return any(needle in line for line in app.log_lines())


def pick_row(app, dropdown_id, index):
    """Choose row `index` of a dropdown with the keyboard (menu rows are
    not in the snapshot; the menu opens on the current row)."""
    click(app, dropdown_id)
    time.sleep(0.3)
    for _ in range(4):
        app.key("up")
    for _ in range(index):
        app.key("down")
    app.key("return")
    time.sleep(0.5)


def drag(app, slider_id, by):
    """Drag a slider by `by` of its width (relative to the knob)."""
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


def pixel(app, x, y):
    """The window's colour at (x, y) points, as (r, g, b)."""
    png = app.grab(os.path.join(OUT, "probe.png"), scale=0.5)
    scale = png.width / app.get("/s")["w"][0]["sz"][0]
    return png.pixel(int(x * scale), int(y * scale))[:3]


def mark_gap(app, widget_id):
    """Points between a checkbox's box and its text."""
    png = app.grab(os.path.join(OUT, "probe.png"))
    scale = png.width / app.get("/s")["w"][0]["sz"][0]
    return drive.gap_after_mark(png, app.rect(widget_id), scale)


def face(app, widget_id):
    """The colour inside a control, right of its text."""
    r = app.rect(widget_id)
    return pixel(app, r[0] + r[2] - 40, r[1] + r[3] / 2) if r else None


def shows(app, widget_id, colour):
    """Whether a sheet is on screen, by one of its controls' face. A closed
    sheet's widgets stay in the snapshot where they were drawn, so look at
    the screen."""
    now = face(app, widget_id)
    return now is not None and all(abs(a - b) <= 6 for a, b in zip(now, colour))


def menu(app, row):
    """Pick the app menu's row `row` (1 = the first) with the keyboard."""
    click(app, "app_menu")
    wait_for(lambda: app.rect("menus"), 5)
    time.sleep(0.2)
    for _ in range(row):
        app.key("down")
    app.key("return")
    time.sleep(0.6)


def answers_file(answers):
    fd, path = tempfile.mkstemp(prefix="reco-m7-answers-", suffix=".json")
    with os.fdopen(fd, "w") as f:
        json.dump(answers, f)
    return path


def launch(config, files=None, answers=None, env=None):
    args = ["--window-size", "1280x820"]
    if files:
        args += ["--left", files[0], "--right", files[1]]
        if len(files) > 2:
            args += ["--calibration", files[2]]
    env = dict(env or {}, RECO_CONFIG_DIR=config)
    if answers is not None:
        env["RECO_DESKTOP_DIALOG_ANSWERS"] = answers_file(answers)
    return drive.App.launch(BIN, args, env=env)


def saved(config):
    path = os.path.join(config, "desktop.json")
    return json.load(open(path)) if os.path.exists(path) else {}


def open_prefs(app):
    """Open Preferences from the app menu; its codec field's face colour
    once it shows (for `shows`), or None."""
    wait_for(lambda: app.rect("app_menu"), 15)
    wait_for(lambda: logged(app, "export codecs:"), 10)
    menu(app, PREFERENCES)
    if not wait_for(lambda: app.rect("prefs_save"), 5):
        return None
    time.sleep(0.3)
    return face(app, "prefs_export_codec")


def check_prefs():
    """Preferences opens from the app menu on the saved values; Cancel and
    Escape keep nothing; a missing folder or a model that isn't .onnx is
    refused with the reason; Save keeps and applies every choice, and turns
    usage data on (an app_open event, no network); a new launch shows them."""
    config = tempfile.mkdtemp(prefix="reco-m7-config-")
    folder = tempfile.mkdtemp(prefix="reco-m7-recordings-")
    model = os.path.join(tempfile.mkdtemp(prefix="reco-m7-model-"), "yolo.onnx")
    with open(model, "wb") as f:
        f.write(b"onnx")
    with launch(config, answers={"recording_folder": [folder], "model": [model]}) as app:
        colour = open_prefs(app)
        expect(colour is not None, "prefs: the app menu opens Preferences")
        expect(text_of(app, "prefs_export_codec") == "H.264" and text_of(app, "prefs_export_quality") == "Balanced",
               "prefs: export defaults show (H.264, Balanced)")
        expect(text_of(app, "prefs_folder") == "" and text_of(app, "prefs_model") == "",
               "prefs: no recording folder or model yet")
        expect(text_of(app, "prefs_blend_value") == "0.05", f"prefs: the seam blend shows 0.05 ({text_of(app, 'prefs_blend_value')})")
        expect(checked(app, "prefs_telemetry") is False, "prefs: usage data is off until turned on")
        gap = mark_gap(app, "prefs_telemetry")
        expect(gap >= 6, f"prefs: the checkbox's text clears its box ({gap} pt)")
        save_shot(app, "prefs")

        pick_row(app, "prefs_export_quality", 2)
        expect(text_of(app, "prefs_export_quality") == "High", "prefs: a change shows in the sheet")
        click(app, "prefs_cancel")
        expect(bool(wait_for(lambda: not shows(app, "prefs_export_codec", colour), 3)), "prefs: Cancel closes it")
        expect(saved(config).get("export_quality") in (None, "balanced"), "prefs: Cancel keeps nothing")
        open_prefs(app)
        expect(text_of(app, "prefs_export_quality") == "Balanced",
               f"prefs: reopened, the change is gone ({text_of(app, 'prefs_export_quality')})")
        pick_row(app, "prefs_record_quality", 2)
        app.key("Escape")
        expect(bool(wait_for(lambda: not shows(app, "prefs_export_codec", colour), 3)), "prefs: Escape closes it")
        open_prefs(app)
        expect(text_of(app, "prefs_record_quality") == "Balanced",
               f"prefs: Escape keeps nothing ({text_of(app, 'prefs_record_quality')})")

        type_into(app, "prefs_folder", "/no/such/folder")
        click(app, "prefs_save")
        expect(wait_for(lambda: text_of(app, "prefs_error_text") == "That recording folder doesn't exist.", 3) is not None,
               f"prefs: a missing folder is refused ({text_of(app, 'prefs_error_text')})")
        expect(shows(app, "prefs_export_codec", colour), "prefs: the sheet stays open")
        click(app, "prefs_folder_browse")
        expect(wait_for(lambda: text_of(app, "prefs_folder") == folder, 3) is not None,
               f"prefs: Choose… sets the folder ({text_of(app, 'prefs_folder')})")
        type_into(app, "prefs_model", model + ".txt")
        click(app, "prefs_save")
        expect(wait_for(lambda: text_of(app, "prefs_error_text") == "The AI model must be an .onnx file.", 3) is not None,
               f"prefs: a model that isn't .onnx is refused ({text_of(app, 'prefs_error_text')})")
        click(app, "prefs_model_browse")
        expect(wait_for(lambda: text_of(app, "prefs_model") == model, 3) is not None,
               f"prefs: Choose… sets the model ({text_of(app, 'prefs_model')})")
        save_shot(app, "prefs-refused")

        pick_row(app, "prefs_export_codec", 1)
        pick_row(app, "prefs_export_quality", 2)
        pick_row(app, "prefs_record_codec", 2)
        pick_row(app, "prefs_record_quality", 0)
        drag(app, "prefs_blend", 0.5)
        blend = text_of(app, "prefs_blend_value")
        expect(blend not in (None, "0.05"), f"prefs: dragging the seam blend shows its value ({blend})")
        click(app, "prefs_telemetry")
        expect(checked(app, "prefs_telemetry") is True, "prefs: usage data ticks on")
        codecs = (text_of(app, "prefs_export_codec"), text_of(app, "prefs_record_codec"))
        click(app, "prefs_save")
        expect(bool(wait_for(lambda: not shows(app, "prefs_export_codec", colour), 3)), "prefs: Save closes it")
        kept = saved(config)
        expect((kept.get("export_codec"), kept.get("export_quality")) == (codecs[0].lower(), "high"),
               f"prefs: export defaults kept ({kept.get('export_codec')}, {kept.get('export_quality')})")
        expect((kept.get("recording_codec"), kept.get("recording_quality")) == (codecs[1].lower(), "fast"),
               f"prefs: recording codec and quality kept ({kept.get('recording_codec')}, {kept.get('recording_quality')})")
        expect(kept.get("recording_folder") == folder and kept.get("ai_model_path") == model,
               "prefs: the folder and the model are kept")
        expect(blend is not None and abs(kept.get("default_blend", -1) - float(blend)) < 0.006,
               f"prefs: the seam blend is kept ({kept.get('default_blend')})")
        expect(kept.get("telemetry_enabled") is True and len(kept.get("telemetry_client_id") or "") == 36,
               "prefs: usage data is on, under a random id")
        expect(wait_for(lambda: logged(app, "network: would send app_open"), 3) is not None,
               "prefs: turning it on sends app_open (no network in checks)")
        expect(text_of(app, "record_quality") == "Fast", f"prefs: the view bar shows the recording quality ({text_of(app, 'record_quality')})")
        expect(not app.errors(), f"prefs: no errors in the app log {app.errors()[:3]}")
    with launch(config) as app:
        expect(wait_for(lambda: logged(app, "network: would send app_open"), 10) is not None,
               "prefs: with usage data on, a launch sends app_open")
        open_prefs(app)
        expect(text_of(app, "prefs_folder") == folder and text_of(app, "prefs_model") == model,
               "prefs: a new launch shows the folder and the model")
        expect(text_of(app, "prefs_export_quality") == "High" and text_of(app, "prefs_record_quality") == "Fast",
               "prefs: and the qualities")
        expect(checked(app, "prefs_telemetry") is True, "prefs: and usage data on")
        save_shot(app, "prefs-saved")


def check_blend():
    """A new calibration starts with Preferences' seam blend."""
    config = tempfile.mkdtemp(prefix="reco-m7-config-")
    with open(os.path.join(config, "desktop.json"), "w") as f:
        json.dump({"default_blend": 0.12}, f)
    folder = tempfile.mkdtemp(prefix="reco-m7-blend-")
    files = []
    for source, name in zip(FAST[:2], ("cam0.mp4", "cam1.mp4")):
        os.symlink(source, os.path.join(folder, name))
        files.append(os.path.join(folder, name))
    # The command line takes a calibration too: add the videos as a person
    # does, through the (answered) dialogs.
    with launch(config, answers={"left": [files[0]], "right": [files[1]]}) as app:
        wait_for(lambda: app.rect("add_left"), 15)
        app.click_id("add_left")
        app.click_id("add_right")
        wait_for(lambda: app.enabled("auto_calibrate"), 15)
        app.click_id("auto_calibrate")
        expect(wait_for(lambda: logged(app, "blend 0.12"), 10) is not None,
               "blend: Auto-calibrate starts with the saved seam blend")
        click(app, "cancel_calibration")
        wait_for(lambda: text_of(app, "calibration_status") == "Not calibrated", 30)
    expect(not os.path.exists(os.path.join(folder, "cam0_calibration.json")), "blend: nothing saved beside the videos")


def texts(app, widget_id):
    """Every drawn widget's text with this id, top to bottom (template rows
    share ids)."""
    found = [i for i in app.snap(widget_id) if i.get("i") == widget_id]
    return [i.get("t", "") for i in sorted(found, key=lambda i: i["r"][1])]


def check_shortcuts():
    """Keyboard shortcuts lists the keys the app answers, Website and Forum
    open their pages (logged in checks), and Close or Escape closes it."""
    config = tempfile.mkdtemp(prefix="reco-m7-config-")
    with launch(config) as app:
        wait_for(lambda: app.rect("app_menu"), 15)
        menu(app, SHORTCUTS)
        expect(bool(wait_for(lambda: app.rect("shortcuts_close"), 5)), "shortcuts: the app menu opens Keyboard shortcuts")
        time.sleep(0.3)
        colour = face(app, "shortcuts_website")
        keys, does = texts(app, "keys"), texts(app, "does")
        expect(list(zip(keys, does))[:2] == [("Space", "Play or pause"), ("[  /  ]", "Back or forward 5 seconds")],
               f"shortcuts: the keys and what they do ({list(zip(keys, does))[:2]})")
        expect(len(keys) == 11 and "⌘," in keys and "Scroll" in keys,
               f"shortcuts: every key, the pointer and the menu keys ({len(keys)} rows)")
        save_shot(app, "shortcuts")
        click(app, "shortcuts_website")
        expect(wait_for(lambda: logged(app, "browser: would open https://github.com/reco-project/video-stitcher"), 3) is not None,
               "shortcuts: Website opens the project's page")
        click(app, "shortcuts_forum")
        expect(wait_for(lambda: logged(app, "browser: would open https://forum.reco-project.org/"), 3) is not None,
               "shortcuts: Forum opens the forum")
        click(app, "shortcuts_close")
        expect(bool(wait_for(lambda: not shows(app, "shortcuts_website", colour), 3)), "shortcuts: Close closes it")
        menu(app, SHORTCUTS)
        time.sleep(0.3)
        expect(shows(app, "shortcuts_website", colour), "shortcuts: it opens again")
        app.key("Escape")
        expect(bool(wait_for(lambda: not shows(app, "shortcuts_website", colour), 3)), "shortcuts: Escape closes it")
        expect(not app.errors(), f"shortcuts: no errors in the app log {app.errors()[:3]}")


CHECKS = {"prefs": check_prefs, "blend": check_blend, "shortcuts": check_shortcuts}


def main():
    os.makedirs(OUT, exist_ok=True)
    names = sys.argv[1:] or list(CHECKS)
    for name in names:
        CHECKS[name]()
    if FAILURES:
        print(f"\nModule 7 check FAILED: {len(FAILURES)} failure(s):")
        for message in FAILURES:
            print(f"  - {message}")
        sys.exit(1)
    print(f"Module 7 check passed. Screenshots: {OUT}")


if __name__ == "__main__":
    main()
