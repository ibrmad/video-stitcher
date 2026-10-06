#!/usr/bin/env python3
"""Module 8 check: the parity sweep's performance pass (PARITY.md, Module 8;
DESIGN.md Rule 8).

Run after `cargo build --profile desktop -p reco-desktop`. Opens the fast
fixture pair (RECO_FIXTURE_LEFT/RIGHT/CAL, else the alfheim set) and the
5.3K match pair, each with a copy of its calibration, through
--left/--right/--calibration and drives the app through Makepad's --remote
control. Each launch gets its own settings folder (drive.launch_env).
Screenshots go to target/desktop-checks/m8/. `check_m8.py NAME...` runs the
named checks (all by default); exits non-zero if any check failed.
"""
import os
import re
import shutil
import sys
import tempfile
import time

import drive

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "..", ".."))
BIN = os.path.join(ROOT, "target", "desktop", "reco-desktop")
OUT = os.path.join(ROOT, "target", "desktop-checks", "m8")
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
    folder = tempfile.mkdtemp(prefix="reco-m8-")
    cal = os.path.join(folder, os.path.basename(files[2]))
    shutil.copyfile(files[2], cal)
    return (files[0], files[1], cal)


def launch(files, extra=()):
    left, right, cal = calibration_copy(files)
    return drive.App.launch(BIN, ["--window-size", "1280x980", "--left", left, "--right", right,
                                  "--calibration", cal, *extra])


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


CHECKS = {
    "start": check_start,
    "perf": check_perf,
    "sheets": check_sheets,
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
        print(f"\nModule 8 check FAILED: {len(FAILURES)} failure(s):")
        for message in FAILURES:
            print(f"  - {message}")
        sys.exit(1)
    print(f"Module 8 check passed. Screenshots: {OUT}")


if __name__ == "__main__":
    main()
