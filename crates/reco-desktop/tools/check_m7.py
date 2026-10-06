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
    wait_for(lambda: app.rect("app_menu_list"), 5)
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


def launch(config, files=None, answers=None, env=None, extra=(), size="1280x820"):
    args = (["--window-size", size] if size else []) + list(extra)
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
    return face(app, "prefs_record_codec")


def check_prefs():
    """Preferences holds only app-wide settings: the recording codec and
    folder, and usage data (the export defaults, recording quality, seam
    blend and AI model live where they are used). It opens from the app
    menu on the saved values; Cancel and Escape keep nothing; a missing
    folder is refused with the reason; Save keeps and applies every choice,
    and turns usage data on (an app_open event, no network); a new launch
    shows them."""
    config = tempfile.mkdtemp(prefix="reco-m7-config-")
    folder = tempfile.mkdtemp(prefix="reco-m7-recordings-")
    with launch(config, answers={"recording_folder": [folder]}) as app:
        colour = open_prefs(app)
        expect(colour is not None, "prefs: the app menu opens Preferences")
        elsewhere = [wid for wid in ("prefs_export_codec", "prefs_export_quality", "prefs_record_quality",
                                     "prefs_blend", "prefs_model") if app.rect(wid)]
        expect(not elsewhere, f"prefs: only app-wide settings, the rest live where they are used ({elsewhere})")
        expect(text_of(app, "prefs_record_codec") == "H.264",
               f"prefs: the recording codec shows H.264 ({text_of(app, 'prefs_record_codec')})")
        expect(text_of(app, "prefs_folder") == "", "prefs: no recording folder yet")
        expect(checked(app, "prefs_telemetry") is False, "prefs: usage data is off until turned on")
        gap = mark_gap(app, "prefs_telemetry")
        expect(gap >= 6, f"prefs: the checkbox's text clears its box ({gap} pt)")
        save_shot(app, "prefs")

        pick_row(app, "prefs_record_codec", 1)
        expect(text_of(app, "prefs_record_codec") != "H.264", "prefs: a change shows in the sheet")
        click(app, "prefs_cancel")
        expect(bool(wait_for(lambda: not shows(app, "prefs_record_codec", colour), 3)), "prefs: Cancel closes it")
        expect(saved(config).get("recording_codec") in (None, "h264"), "prefs: Cancel keeps nothing")
        open_prefs(app)
        expect(text_of(app, "prefs_record_codec") == "H.264",
               f"prefs: reopened, the change is gone ({text_of(app, 'prefs_record_codec')})")
        pick_row(app, "prefs_record_codec", 1)
        app.key("Escape")
        expect(bool(wait_for(lambda: not shows(app, "prefs_record_codec", colour), 3)), "prefs: Escape closes it")
        open_prefs(app)
        expect(text_of(app, "prefs_record_codec") == "H.264",
               f"prefs: Escape keeps nothing ({text_of(app, 'prefs_record_codec')})")

        type_into(app, "prefs_folder", "/no/such/folder")
        click(app, "prefs_save")
        expect(wait_for(lambda: text_of(app, "prefs_error_text") == "That recording folder doesn't exist.", 3) is not None,
               f"prefs: a missing folder is refused ({text_of(app, 'prefs_error_text')})")
        expect(shows(app, "prefs_record_codec", colour), "prefs: the sheet stays open")
        click(app, "prefs_folder_browse")
        expect(wait_for(lambda: text_of(app, "prefs_folder") == folder, 3) is not None,
               f"prefs: Choose… sets the folder ({text_of(app, 'prefs_folder')})")
        save_shot(app, "prefs-refused")

        pick_row(app, "prefs_record_codec", 1)
        click(app, "prefs_telemetry")
        expect(checked(app, "prefs_telemetry") is True, "prefs: usage data ticks on")
        codec = text_of(app, "prefs_record_codec")
        click(app, "prefs_save")
        expect(bool(wait_for(lambda: not shows(app, "prefs_record_codec", colour), 3)), "prefs: Save closes it")
        kept = saved(config)
        expect(kept.get("recording_codec") == (codec or "").lower(),
               f"prefs: the recording codec is kept ({kept.get('recording_codec')})")
        expect(kept.get("recording_folder") == folder, "prefs: the folder is kept")
        expect(kept.get("telemetry_enabled") is True and len(kept.get("telemetry_client_id") or "") == 36,
               "prefs: usage data is on, under a random id")
        expect(wait_for(lambda: logged(app, "network: would send app_open"), 3) is not None,
               "prefs: turning it on sends app_open (no network in checks)")
        expect(not app.errors(), f"prefs: no errors in the app log {app.errors()[:3]}")
    with launch(config) as app:
        expect(wait_for(lambda: logged(app, "network: would send app_open"), 10) is not None,
               "prefs: with usage data on, a launch sends app_open")
        open_prefs(app)
        expect(text_of(app, "prefs_folder") == folder and text_of(app, "prefs_record_codec") == codec,
               "prefs: a new launch shows the codec and the folder")
        expect(checked(app, "prefs_telemetry") is True, "prefs: and usage data on")
        save_shot(app, "prefs-saved")


def check_blend():
    """A first calibration starts at seam blend 0.05: a default saved by an
    older Preferences (it had one) no longer applies."""
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
        expect(wait_for(lambda: logged(app, "blend 0.05"), 10) is not None,
               "blend: Auto-calibrate starts at 0.05, whatever an old saved default said")
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
        expect(len(keys) == 12 and "⌘S" in keys and "⌘," in keys and "Scroll" in keys,
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


def kept(folder, name):
    """What the app kept under a switch's folder for `name`, oldest first."""
    files = sorted((f for f in os.listdir(folder) if f.startswith(f"{name}-")),
                   key=lambda f: int(f.rsplit("-", 1)[1].split(".")[0]))
    return [open(os.path.join(folder, f)).read() for f in files]


def check_bug():
    """Report a bug: Send waits for words and usage data (a hint offers
    Preferences); Copy report copies the report, with or without the
    details; with usage data on, Send sends it (no network in checks) with
    the version, the GPU, the files' names and the log, the home folder as
    ~, and says so."""
    config = tempfile.mkdtemp(prefix="reco-m7-config-")
    clipboard = tempfile.mkdtemp(prefix="reco-m7-clipboard-")
    with launch(config, env={"RECO_DESKTOP_NO_CLIPBOARD": clipboard}) as app:
        wait_for(lambda: app.rect("app_menu"), 15)
        menu(app, REPORT_BUG)
        expect(bool(wait_for(lambda: app.rect("bug_copy"), 5)), "bug: the app menu opens Report a bug")
        time.sleep(0.3)
        colour = face(app, "bug_copy")
        expect(app.enabled("bug_send") is False, "bug: Send waits")
        expect(app.rect("bug_prefs") is not None, "bug: with usage data off, a hint offers Preferences")
        type_into(app, "bug_message", "It froze")
        time.sleep(0.3)
        expect(app.enabled("bug_send") is False, "bug: words alone don't enable Send while usage data is off")
        save_shot(app, "bug")
        click(app, "bug_copy")
        expect(wait_for(lambda: logged(app, "clipboard: would copy bug_report"), 3) is not None,
               "bug: Copy report copies it (no clipboard in checks)")
        copied = kept(clipboard, "bug_report")
        report = copied[0] if copied else ""
        expect(report.startswith("## User description\nIt froze\n\n## Contact\n(not provided)\n"),
               f"bug: the report starts with the words and the contact ({report[:60]!r})")
        expect("## Environment\n- Reco 0.5.4" in report and "\n## Log (last " in report,
               "bug: with the version and the log")
        expect(HOME + "/" not in report, "bug: the home folder reads as ~")
        click(app, "bug_details")
        click(app, "bug_copy")
        wait_for(lambda: len(kept(clipboard, "bug_report")) == 2, 3)
        plain = (kept(clipboard, "bug_report") + [""])[1]
        expect(plain == "## User description\nIt froze\n\n## Contact\n(not provided)\n",
               f"bug: without details, only the words and the contact ({plain!r})")
        click(app, "bug_prefs")
        expect(bool(wait_for(lambda: not shows(app, "bug_copy", colour), 3)), "bug: Preferences… closes the sheet")
        expect(bool(wait_for(lambda: app.rect("prefs_save"), 3)), "bug: and opens Preferences")
        expect(not app.errors(), f"bug: no errors in the app log {app.errors()[:3]}")

    config = tempfile.mkdtemp(prefix="reco-m7-config-")
    with open(os.path.join(config, "desktop.json"), "w") as f:
        json.dump({"telemetry_enabled": True}, f)
    sent = tempfile.mkdtemp(prefix="reco-m7-sent-")
    with launch(config, FAST, env={"RECO_DESKTOP_NO_NETWORK": sent}) as app:
        expect(wait_for(lambda: app.rect("preview"), 30) is not None, "bug: the preview opens")
        wait_for(lambda: logged(app, "network: would send context"), 10)
        menu(app, REPORT_BUG)
        wait_for(lambda: app.rect("bug_copy"), 5)
        time.sleep(0.3)
        expect(app.enabled("bug_send") is False, "bug: Send waits for words")
        type_into(app, "bug_message", "Export froze at 40%")
        type_into(app, "bug_contact", "ann on the forum")
        expect(wait_for(lambda: app.enabled("bug_send"), 3) is True, "bug: with words and usage data on, Send is ready")
        click(app, "bug_send")
        expect(wait_for(lambda: logged(app, "network: would send bug_report"), 3) is not None,
               "bug: Send sends it (no network in checks)")
        expect(bool(wait_for(lambda: title_rect(app, "Report sent"), 3)), "bug: a notice says it was sent")
        batches = [json.loads(b) for b in kept(sent, "bug_report")]
        report = batches[0]["events"][0]["props"]["report"] if batches else ""
        expect(batches and batches[0]["events"][0]["name"] == "bug_report", "bug: as a bug_report event")
        expect("## Contact\nann on the forum\n" in report and "Export froze at 40%" in report,
               "bug: with the words and the contact")
        expect("## Files\n- Left: cam0.mp4\n- Right: cam1.mp4\n- Calibration: match.json\n" in report,
               "bug: with the files' names only")
        expect("- GPU: " in report and "- GPU: not started" not in report, "bug: with the GPU")
        expect(HOME + "/" not in report, "bug: and no home folder")
        expect([json.loads(b)["events"][0]["name"] for b in kept(sent, "app_open")] == ["app_open"]
               and len(kept(sent, "context")) == 1, "bug: usage data also sent app_open and the context once")
        expect(not app.errors(), f"bug: no errors in the app log {app.errors()[:3]}")


def title_rect(app, text):
    """The rect of a toast title showing `text`, or None."""
    for found in app.snap(text):
        if found.get("t") == text and found.get("i") == "title":
            return found["r"]
    return None


def events(folder, name):
    """The props of each `name` event kept in `folder`."""
    return [json.loads(b)["events"][0]["props"] for b in kept(folder, name)]


def check_usage():
    """Usage data goes out only when opted in: with it on, an opened match
    sends its source info, an export its outcome, and a failed calibration
    its error (no network in checks); with it off, nothing at all."""
    config = tempfile.mkdtemp(prefix="reco-m7-config-")
    with open(os.path.join(config, "desktop.json"), "w") as f:
        json.dump({"telemetry_enabled": True}, f)
    sent = tempfile.mkdtemp(prefix="reco-m7-sent-")
    out = os.path.join(tempfile.mkdtemp(prefix="reco-m7-export-"), "short")
    with launch(config, FAST, answers={"export": [out]}, env={"RECO_DESKTOP_NO_NETWORK": sent},
                extra=["--export-range", "0-1"]) as app:
        expect(wait_for(lambda: app.rect("preview"), 30) is not None, "usage: the preview opens")
        source = wait_for(lambda: events(sent, "source_info"), 10) or [{}]
        expect((source[0].get("width"), source[0].get("height"), source[0].get("decoder")) == (1280, 960, "zero-copy")
               and abs(source[0].get("fps", 0) - 30) < 1 and isinstance(source[0].get("sync_offset"), int),
               f"usage: an opened match sends its source info ({source[0]})")
        wait_for(lambda: app.enabled("export_button"), 10)
        click(app, "export_button")
        wait_for(lambda: app.rect("sheet_export"), 5)
        time.sleep(0.5)
        click(app, "export_browse")
        wait_for(lambda: text_of(app, "export_output") == out + ".mp4", 5)
        click(app, "sheet_export")
        expect(wait_for(lambda: logged(app, "export: done"), 90) is not None, "usage: a one-second export finishes")
        done = wait_for(lambda: events(sent, "export_complete"), 5) or [{}]
        expect(25 <= done[0].get("frames", 0) <= 35 and done[0].get("codec") == "h264"
               and abs(done[0].get("duration_sec", 0) - done[0].get("frames", 0) / 30) < 0.1,
               f"usage: and its outcome ({done[0]})")
        expect(len(events(sent, "source_info")) == 1, "usage: source info once per open")
        expect(not app.errors(), f"usage: no errors in the app log {app.errors()[:3]}")

    quiet = tempfile.mkdtemp(prefix="reco-m7-quiet-")
    with launch(tempfile.mkdtemp(prefix="reco-m7-config-"), FAST, env={"RECO_DESKTOP_NO_NETWORK": quiet}) as app:
        expect(wait_for(lambda: app.rect("preview"), 30) is not None, "usage: off, the preview opens")
        time.sleep(2)
        expect(os.listdir(quiet) == [] and not logged(app, "network: would send"),
               f"usage: with it off, nothing is sent ({os.listdir(quiet)})")

    folder = tempfile.mkdtemp(prefix="reco-m7-calibrate-")
    files = []
    for source_file, name in zip(FAST[:2], ("cam0.mp4", "cam1.mp4")):
        os.symlink(source_file, os.path.join(folder, name))
        files.append(os.path.join(folder, name))
    sent = tempfile.mkdtemp(prefix="reco-m7-sent-")
    with launch(config, answers={"left": [files[0]], "right": [files[1]]}, env={"RECO_DESKTOP_NO_NETWORK": sent}) as app:
        wait_for(lambda: app.rect("add_left"), 15)
        app.click_id("add_left")
        app.click_id("add_right")
        wait_for(lambda: app.enabled("auto_calibrate"), 15)
        app.click_id("auto_calibrate")
        failed = wait_for(lambda: events(sent, "calibration_error"), 180) or [{}]
        expect("no usable frame pairs" in failed[0].get("error_message", ""),
               f"usage: a failed calibration sends its error ({failed[0]})")


def check_update():
    """A newer release (GitHub's answer faked in checks) shows a notice with
    Download, which opens the release page only when clicked; the same
    version, or a tag that isn't a plain name, shows nothing; offline,
    nothing is asked."""
    config = tempfile.mkdtemp(prefix="reco-m7-config-")
    with launch(config, env={"RECO_DESKTOP_FAKE_RELEASE": "v9.9.9"}) as app:
        expect(bool(wait_for(lambda: title_rect(app, "Update available: v9.9.9"), 10)),
               "update: a newer release shows a notice")
        expect(text_of(app, "action") == "Download", f"update: with Download ({text_of(app, 'action')})")
        expect(not logged(app, "browser: would open"), "update: the browser waits for a click")
        save_shot(app, "update")
        click(app, "action")
        expect(wait_for(lambda: logged(app, "browser: would open https://github.com/reco-project/video-stitcher/releases/tag/v9.9.9"), 3)
               is not None, "update: Download opens the release page")
        expect(wait_for(lambda: title_rect(app, "Update available: v9.9.9") is None, 3) is True,
               "update: and closes the notice")
        expect(not app.errors(), f"update: no errors in the app log {app.errors()[:3]}")
    for tag, says in (("v0.5.4", "update check: up to date"), ("v1 & calc", "update check: no release in the answer")):
        with launch(config, env={"RECO_DESKTOP_FAKE_RELEASE": tag}) as app:
            expect(wait_for(lambda: logged(app, says), 10) is not None, f"update: {tag!r} → {says!r}")
            time.sleep(0.5)
            expect(not title_rect(app, f"Update available: {tag}"), f"update: no notice for {tag!r}")
    with launch(config) as app:
        expect(wait_for(lambda: logged(app, "update check: would ask GitHub"), 10) is not None,
               "update: offline, GitHub isn't asked")


def drag_edge(app, x, y, by):
    """Drag a panel edge at (x, y) by `by` points across."""
    app.get("/m", k="down", x=x, y=y, wait=1)
    for step in range(1, 7):
        app.get("/m", k="move", x=x + by * step / 6, y=y, wait=1)
    app.get("/m", k="up", x=x + by, y=y, wait=1)


def window_size(app):
    return tuple(app.get("/s")["w"][0]["sz"][:2])


def check_persist():
    """The window's size and the side panels' widths come back after a
    restart (saved a quiet second after a change); --window-size still
    wins."""
    config = tempfile.mkdtemp(prefix="reco-m7-config-")
    with launch(config, FAST, size="1440x900") as app:
        expect(wait_for(lambda: app.rect("preview"), 30) is not None, "persist: the preview opens")
        # A stitched match opens the Adjust panel by itself.
        wait_for(lambda: app.rect("inspector"), 5)
        setup, adjust = app.rect("media_panel"), app.rect("inspector")
        expect(setup is not None and adjust is not None, "persist: both side panels show")
        mid = setup[1] + setup[3] / 2
        drag_edge(app, setup[0] + setup[2] + 3, mid, 60)
        drag_edge(app, adjust[0] - 3, mid, -40)
        time.sleep(0.5)
        widths = (app.rect("media_panel")[2], app.rect("inspector")[2])
        expect(abs(widths[0] - setup[2] - 60) <= 2 and abs(widths[1] - adjust[2] - 40) <= 2,
               f"persist: dragging the edges widens both panels ({setup[2]}→{widths[0]}, {adjust[2]}→{widths[1]})")
        expect(wait_for(lambda: logged(app, "layout: saved"), 3) is not None, "persist: saved a quiet second later")
        save_shot(app, "persist-before")
    kept_layout = saved(config)
    expect(kept_layout.get("window_size") == [1440.0, 900.0] and not kept_layout.get("window_maximized"),
           f"persist: the window is remembered ({kept_layout.get('window_size')})")
    with launch(config, FAST, size=None) as app:
        expect(wait_for(lambda: app.rect("preview"), 30) is not None, "persist: it opens again")
        time.sleep(0.5)
        expect(window_size(app) == (1440, 900), f"persist: at its size ({window_size(app)})")
        wait_for(lambda: app.rect("inspector"), 5)
        again = (app.rect("media_panel")[2], (app.rect("inspector") or [0, 0, 0])[2])
        expect(all(abs(a - b) <= 1 for a, b in zip(again, widths)),
               f"persist: with the panels' widths ({widths} → {again})")
        save_shot(app, "persist-after")
    with launch(config) as app:
        time.sleep(1.0)
        expect(window_size(app) == (1280, 820), f"persist: --window-size wins ({window_size(app)})")
    # Full screen at start is not run here: even a hidden window would take
    # over the display. remember_window's test and a one-off probe cover it.


CHECKS = {"prefs": check_prefs, "blend": check_blend, "shortcuts": check_shortcuts, "bug": check_bug,
          "usage": check_usage, "update": check_update, "persist": check_persist}


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
