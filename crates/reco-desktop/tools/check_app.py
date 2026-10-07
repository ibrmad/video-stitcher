#!/usr/bin/env python3
"""App check: startup and performance (DESIGN.md Rule 8), sheets and
shortcuts (Rule 9), quitting with unsaved edits and the log file.

Run after `cargo build --profile desktop -p reco-desktop`. Opens the fast
fixture pair (RECO_FIXTURE_LEFT/RIGHT/CAL, else the alfheim set) and the
5.3K match pair, each with a copy of its calibration, through
--left/--right/--calibration and drives the app through Makepad's --remote
control. Each launch gets its own settings folder (drive.launch_env).
Screenshots go to target/desktop-checks/app/. `check_app.py NAME...` runs the
named checks (all by default); exits non-zero if any check failed.
"""
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
import time

import drive

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "..", ".."))
BIN = os.path.join(ROOT, "target", "desktop", "reco-desktop")
OUT = os.path.join(ROOT, "target", "desktop-checks", "app")
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
# The pairs' frame rates (alfheim 30, the match 29.97).
SOURCE_FPS = {"fast": 30.0, "real": 29.97}
# A 60 Hz display's frame (ms): a picture rendered within it can follow
# every frame of a pan.
FRAME_60HZ_MS = 1000 / 60

FAILURES = []


def expect(ok, message):
    print(f"{'ok' if ok else 'FAIL'}: {message}")
    if not ok:
        FAILURES.append(message)


def calibration_copy(files):
    """The pair with its calibration copied to a temp folder: nothing is
    ever written beside the fixtures."""
    folder = tempfile.mkdtemp(prefix="reco-app-")
    cal = os.path.join(folder, os.path.basename(files[2]))
    shutil.copyfile(files[2], cal)
    return (files[0], files[1], cal)


def launch(files, extra=(), copied=False, env=None):
    """The app on a pair, with a copy of its calibration unless `copied`."""
    left, right, cal = files if copied else calibration_copy(files)
    return drive.App.launch(BIN, ["--window-size", "1280x980", "--left", left, "--right", right,
                                  "--calibration", cal, *extra], env=env)


def wait_for(probe, secs):
    deadline = time.monotonic() + secs
    while time.monotonic() < deadline:
        value = probe()
        if value:
            return value
        time.sleep(0.2)
    return None


def text_of(app, widget_id):
    for item in app.snap(widget_id):
        if item.get("i") == widget_id:
            return item.get("t", "")
    return None


def number(text):
    """The first number in a label ('29.9 fps', '4.1 ms'), or None."""
    found = re.search(r"\d+(?:\.\d+)?", text or "")
    return float(found.group()) if found else None


def save_shot(app, name):
    """Save a screenshot without decoding it (decoding takes seconds)."""
    shutil.copyfile(app.get("/g", scale=1.0)["png"], os.path.join(OUT, f"{name}.png"))


def open_stats(app):
    """Open the Adjust panel's Stats section by its chevron."""
    r = app.rect("stats_section")
    if r:
        app.get("/click", x=r[0] + 18, y=r[1] + 12, wait=1)
        time.sleep(0.5)


def stats(app):
    """The Stats section's figures: fps, frame, slowest, decode, render."""
    return {key: number(text_of(app, f"stats_{key}"))
            for key in ("fps", "frame", "slowest", "decode", "render")}


def cpu_percent(app, secs):
    c0 = app.cpu_seconds()
    time.sleep(secs)
    return (app.cpu_seconds() - c0) / secs * 100


def first_frame_ms(app):
    for line in app.log_lines():
        found = re.search(r"first frame: (\d+) ms after launch", line)
        if found:
            return int(found.group(1))
    return None


def draw_summaries(app):
    """Each `--perf-log` summary's (frames, avg ms, max ms)."""
    out = []
    for line in app.log_lines():
        found = re.search(r"ui draw: (\d+) frames, avg ([\d.]+) ms, max ([\d.]+) ms", line)
        if found:
            out.append((int(found.group(1)), float(found.group(2)), float(found.group(3))))
    return out


def pan(app, secs):
    """Drag across the picture and back for `secs`, a move every frame or
    so, and release."""
    x, y, w, h = app.rect("preview")
    cx, cy = x + w / 2, y + h / 2
    app.get("/m", k="down", x=cx, y=cy)
    start = time.monotonic()
    step = 0
    while time.monotonic() - start < secs:
        step += 1
        phase = step % 40
        offset = 6 * (phase if phase < 20 else 40 - phase) - 60
        app.get("/m", k="move", x=cx + offset, y=cy)
    app.get("/m", k="up", x=cx, y=cy, wait=1)
    return step / secs


def check_start():
    """Rule 8: the first window frame within 1 s of the launch. The first
    launch of a new build also pays macOS's one-time look at a new binary,
    so it is printed, and the three launches after it are judged."""
    times = []
    for _ in range(4):
        app = drive.App.launch(BIN, ["--window-size", "1280x820"])
        try:
            times.append(wait_for(lambda: first_frame_ms(app), 5))
        finally:
            app.quit()
    print(f"start: the first launch's first frame came {times[0]} ms after it")
    warm = times[1:]
    expect(all(t is not None and t < 1000 for t in warm),
           f"start: the first window frame within 1 s of the launch ({warm} ms)")


def check_perf():
    """Rule 8 on both pairs: the zero-copy preview, the source rate while
    playing, a fresh picture while panning, UI draws under 4 ms, about 0%
    CPU paused."""
    for name, files in (("fast", FAST), ("real", REAL)):
        if not all(os.path.exists(p) for p in files):
            print(f"skip: perf {name}: fixtures not found")
            continue
        with launch(files, extra=("--perf-log",)) as app:
            if wait_for(lambda: app.rect("preview"), 90) is None:
                expect(False, f"perf {name}: the preview opens")
                continue
            opened = [line for line in app.log_lines() if "preview:" in line and "input" in line]
            expect(any("zero-copy" in line for line in opened),
                   f"perf {name}: the preview is zero-copy, no CPU copies ({opened[-1:]})")
            open_stats(app)

            app.key("space")
            time.sleep(1.5)
            playing_cpu = cpu_percent(app, 3)
            play = stats(app)
            save_shot(app, f"playing-{name}")
            app.key("space")
            source = SOURCE_FPS[name]
            print(f"perf {name}: playing {play}, {playing_cpu:.0f}% CPU")
            expect((play["fps"] or 0) >= 0.95 * source,
                   f"perf {name}: playing keeps the source rate ({play['fps']} fps, source {source})")

            time.sleep(1.5)
            idle = cpu_percent(app, 3)
            expect(idle < 2, f"perf {name}: about 0% CPU while paused ({idle:.1f}%)")

            moves = pan(app, 3)
            panned = stats(app)
            print(f"perf {name}: panning ({moves:.0f} moves a second) {panned}")
            expect((panned["fps"] or 0) >= 48,
                   f"perf {name}: panning redraws the picture at least 48 times a second "
                   f"({panned['fps']}, for {moves:.0f} moves a second)")
            expect((panned["slowest"] or 99) < FRAME_60HZ_MS,
                   f"perf {name}: the slowest picture of a pan fits a 60 Hz frame ({panned['slowest']} ms)")

            draws = draw_summaries(app)
            print(f"perf {name}: ui draw summaries {draws}")
            expect(len(draws) >= 3, f"perf {name}: draw times are logged ({len(draws)} summaries)")
            expect(all(avg < 4.0 for _, avg, _ in draws),
                   f"perf {name}: UI draws take under 4 ms a frame on average ({[d[1] for d in draws]})")
            expect(app.errors() == [], f"perf {name}: no errors in the app log {app.errors()[:3]}")


# The app menu's rows (1 = the first).
SHORTCUTS, REPORT_BUG = 1, 3


def click(app, widget_id):
    """Click a widget if it is on screen; whether it was."""
    r = app.rect(widget_id)
    if r:
        app.get("/click", x=r[0] + r[2] / 2, y=r[1] + r[3] / 2, wait=1)
    return r is not None


def menu(app, row):
    """Pick the app menu's row `row` with the keyboard."""
    click(app, "app_menu")
    wait_for(lambda: app.rect("app_menu_list"), 5)
    time.sleep(0.2)
    for _ in range(row):
        app.key("down")
    app.key("return")


def setup_shown(app):
    return app.rect("setup_header") is not None


def check_sheets():
    """DESIGN.md Rule 9: no shortcut fires behind a sheet, and Escape closes
    every sheet. A closed sheet's widgets stay in the snapshot, so ⌘1 tells:
    once a sheet closes, it hides Setup again."""
    with launch(FAST) as app:
        if wait_for(lambda: app.rect("preview"), 30) is None:
            expect(False, "sheets: the preview opens")
            return
        wait_for(lambda: app.enabled("export_button"), 10)
        time.sleep(0.5)
        openers = (
            ("export", lambda: click(app, "export_button")),
            ("keyboard shortcuts", lambda: menu(app, SHORTCUTS)),
            ("bug report", lambda: menu(app, REPORT_BUG)),
            ("lens picker", lambda: click(app, "lens_browse")),
            ("Preferences", lambda: app.key("Comma", cmd=1)),
        )
        for name, open_it in openers:
            open_it()
            time.sleep(0.8)
            before = text_of(app, "time_current")
            app.key("Key1", cmd=1)
            app.key("space")
            time.sleep(0.8)
            expect(setup_shown(app), f"sheets: ⌘1 does nothing behind the {name} sheet")
            expect(text_of(app, "time_current") == before,
                   f"sheets: Space doesn't play behind the {name} sheet ({before} -> {text_of(app, 'time_current')})")
            if name != "Preferences":
                app.key("Comma", cmd=1)
                time.sleep(0.6)
                expect(app.rect("prefs_save") is None, f"sheets: ⌘, doesn't open Preferences over the {name} sheet")
            save_shot(app, f"sheet-{name.split()[0].lower()}")
            app.key("escape")
            time.sleep(0.5)
            app.key("Key1", cmd=1)
            expect(bool(wait_for(lambda: not setup_shown(app), 3)),
                   f"sheets: Escape closes the {name} sheet (⌘1 hides Setup again)")
            app.key("Key1", cmd=1)
            wait_for(lambda: setup_shown(app), 3)
            time.sleep(0.4)
        expect(app.errors() == [], f"sheets: no errors in the app log {app.errors()[:3]}")


def blend_saved(path):
    with open(path) as f:
        return json.load(f).get("blend_width")


def edit_blend(app, value):
    """Type a seam blend; whether the Adjust panel then says Unsaved."""
    click(app, "seam_value")
    time.sleep(0.3)
    app.get("/k", t=value)
    app.key("return")
    return bool(wait_for(lambda: app.rect("calibration_unsaved"), 3))


def quits(app, secs):
    try:
        app.proc.wait(timeout=secs)
        return True
    except subprocess.TimeoutExpired:
        return False


def asks(app):
    """Quit as ⌘Q does; whether the app stays, asking about unsaved edits
    (False when it quit)."""
    app.get("/quit")
    deadline = time.monotonic() + 3
    while time.monotonic() < deadline:
        if app.proc.poll() is not None:
            return False
        try:
            if app.rect("unsaved_save"):
                return True
        except OSError:
            return False
        time.sleep(0.2)
    return False


def check_unsaved():
    """Quitting with unsaved calibration edits asks first: Cancel stays, Save saves and quits, Don't Save quits and
    leaves the file; with nothing unsaved it quits at once."""
    with launch(FAST) as app:
        wait_for(lambda: app.rect("preview"), 30)
        app.get("/quit")
        expect(quits(app, 5), "unsaved: with nothing unsaved, quitting quits at once")

    files = calibration_copy(FAST)
    before = blend_saved(files[2])
    with launch(files, copied=True) as app:
        wait_for(lambda: app.rect("preview"), 30)
        expect(edit_blend(app, "0.08"), "unsaved: a typed seam blend is unsaved")
        asked = asks(app)
        expect(asked, "unsaved: quitting asks first")
        if not asked:
            return
        save_shot(app, "unsaved")
        click(app, "unsaved_cancel")
        time.sleep(0.8)
        expect(app.proc.poll() is None and app.rect("calibration_unsaved") is not None,
               "unsaved: Cancel keeps the app and the edit")
        expect(asks(app), "unsaved: quitting asks again")
        click(app, "unsaved_save")
        expect(quits(app, 10), "unsaved: Save quits once it has saved")
    saved = blend_saved(files[2])
    expect(saved is not None and abs(saved - 0.08) < 1e-6, f"unsaved: Save wrote the edit ({before} -> {saved})")

    files = calibration_copy(FAST)
    with launch(files, copied=True) as app:
        wait_for(lambda: app.rect("preview"), 30)
        edit_blend(app, "0.12")
        expect(asks(app), "unsaved: quitting asks (Don't Save)")
        click(app, "unsaved_discard")
        expect(quits(app, 10), "unsaved: Don't Save quits")
    expect(blend_saved(files[2]) == before, f"unsaved: and leaves the file as it was ({blend_saved(files[2])})")


def check_logfile():
    """The log file: the engine's lines and the app's,
    kept across runs, RUST_LOG filtering them."""
    path = os.path.join(tempfile.mkdtemp(prefix="reco-app-log-"), "reco-desktop.log")
    with launch(FAST, env={"RECO_DESKTOP_LOG_FILE": path}) as app:
        wait_for(lambda: app.rect("preview"), 30)
    first = open(path).read() if os.path.exists(path) else ""
    expect("Pipeline initialized" in first, "logfile: the engine's lines are in it (reco-core's pipeline)")
    expect("preview: 1280x960 input" in first, "logfile: the app's own lines are in it")
    expect(bool(re.match(r"\d{4}-\d\d-\d\dT\d\d:\d\d:\d\d\.\d{3}Z ", first)), "logfile: each line starts with its time")
    with launch(FAST, env={"RECO_DESKTOP_LOG_FILE": path, "RUST_LOG": "warn"}) as app:
        wait_for(lambda: app.rect("preview"), 30)
    both = open(path).read()
    expect(both.startswith(first) and len(both) > len(first), "logfile: a second run adds to it")
    second = both[len(first):]
    expect("Pipeline initialized" not in second and "preview: 1280x960 input" in second,
           "logfile: RUST_LOG=warn leaves the engine's info lines out; the app's own stay")


CHECKS = {
    "start": check_start,
    "perf": check_perf,
    "sheets": check_sheets,
    "unsaved": check_unsaved,
    "logfile": check_logfile,
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
        print(f"\nApp check FAILED: {len(FAILURES)} failure(s):")
        for message in FAILURES:
            print(f"  - {message}")
        sys.exit(1)
    print(f"App check passed. Screenshots: {OUT}")


if __name__ == "__main__":
    main()
