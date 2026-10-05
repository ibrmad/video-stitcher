#!/usr/bin/env python3
"""Module 2 check: the time panel and status (PARITY.md, Module 2).

Run after `cargo build --profile desktop -p reco-desktop`. Opens the fast
fixture pair (RECO_FIXTURE_LEFT/RIGHT/CAL, else the alfheim set) through
--left/--right/--calibration and drives the app through Makepad's --remote
control. Each launch gets its own settings folder (drive.launch_env).
Screenshots go to target/desktop-checks/m2/. `check_m2.py NAME...` runs the
named checks (all by default); exits non-zero if any check failed.
"""
import json
import os
import shutil
import subprocess
import sys
import tempfile
import time

import drive

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "..", ".."))
BIN = os.path.join(ROOT, "target", "desktop", "reco-desktop")
OUT = os.path.join(ROOT, "target", "desktop-checks", "m2")
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


def launch(files=FAST, extra=(), config_dir=None):
    left, right, cal = files
    env = {"RECO_CONFIG_DIR": config_dir} if config_dir else None
    return drive.App.launch(BIN, ["--window-size", "1280x820", "--left", left, "--right", right,
                                  "--calibration", cal, *extra], env=env)


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


def seconds(clock):
    """'1:05' or '1:02:03' to seconds; None for anything else."""
    try:
        return drive.parse_cputime(clock)
    except (AttributeError, ValueError):
        return None


def pick_aspect_4x3(app):
    """Choose 4:3 from the aspect dropdown with the keyboard."""
    app.click_id("aspect")
    app.key("down")
    app.key("down")
    app.key("return")
    time.sleep(0.5)


def check_persist():
    """The preview aspect survives a restart; a malformed file still opens."""
    config = tempfile.mkdtemp(prefix="reco-desktop-config-")
    with launch(config_dir=config) as app:
        expect(ready(app) is not None, "persist: the preview appears")
        pick_aspect_4x3(app)
    path = os.path.join(config, "desktop.json")
    saved = json.load(open(path)) if os.path.exists(path) else {}
    expect(saved.get("preview_aspect") == "4:3", f"persist: the aspect is saved ({saved.get('preview_aspect')})")
    with launch(config_dir=config) as app:
        ready(app)
        time.sleep(1.0)
        x, y, w, h = app.rect("preview")
        expect(abs(w / h - 4 / 3) < 0.03, f"persist: the aspect comes back after a restart ({w}x{h})")
    with open(path, "w") as f:
        f.write("{")
    with launch(config_dir=config) as app:
        expect(ready(app) is not None, "persist: a malformed settings file still opens")
        expect(app.errors() == [], "persist: no errors in the app log")


def frame_pixels(app, name):
    """Pixels over the preview's frame, every 6 points: exact, so one video
    frame can be told from the next."""
    png = app.grab(os.path.join(OUT, f"{name}.png"))
    scale = png.width / app.get("/s")["w"][0]["sz"][0]
    x, y, w, h = app.rect("preview")
    return [png.pixel(int((x + i) * scale), int((y + j) * scale))[:3]
            for j in range(2, int(h) - 2, 6) for i in range(2, int(w) - 2, 6)]


def widget_pixels(app, widget_id, name):
    """Every pixel of one widget, for telling an icon from another."""
    png = app.grab(os.path.join(OUT, f"{name}.png"))
    scale = png.width / app.get("/s")["w"][0]["sz"][0]
    x, y, w, h = app.rect(widget_id)
    return [png.pixel(int((x + i) * scale), int((y + j) * scale))[:3]
            for j in range(int(h)) for i in range(int(w))]


def check_transport():
    """Step, play and pause; the play button's icon; the status line; the end."""
    with launch() as app:
        ready(app)
        time.sleep(1.0)
        expect(text_of(app, "status_text") == "Ready", f"transport: Ready after opening ({text_of(app, 'status_text')})")
        paused_icon = widget_pixels(app, "play_pause", "icon-paused")
        a = frame_pixels(app, "step-a")
        app.click_id("step_forward")
        time.sleep(0.6)
        b = frame_pixels(app, "step-b")
        app.click_id("step_back")
        time.sleep(1.0)
        c = frame_pixels(app, "step-c")
        expect(a != b, "transport: step forward shows the next frame")
        expect(a == c, "transport: step back shows the frame before again")
        app.key("space")
        time.sleep(2.5)
        status = text_of(app, "status_text") or ""
        expect(status.endswith("fps"), f"transport: the status line shows the frame rate while playing ({status})")
        expect(widget_pixels(app, "play_pause", "icon-playing") != paused_icon,
               "transport: the play button shows pause while playing")
        app.key("space")
        time.sleep(0.5)
        expect(text_of(app, "status_text") == "Paused", f"transport: Paused ({text_of(app, 'status_text')})")
        for _ in range(12):
            app.key("]")
        time.sleep(1.5)
        app.key("space")
        finished = wait_for(lambda: text_of(app, "status_text") == "Finished", 10)
        expect(bool(finished), "transport: playback finishes at the end")
        app.key("space")
        time.sleep(1.5)
        t = seconds(text_of(app, "time_current"))
        expect(t is not None and t < 5, f"transport: Space after the end plays from the start ({t})")
        expect(app.errors() == [], "transport: no errors in the app log")


CHECKS = {
    "persist": check_persist,
    "transport": check_transport,
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
        print(f"\nModule 2 check FAILED: {len(FAILURES)} failure(s):")
        for message in FAILURES:
            print(f"  - {message}")
        sys.exit(1)
    print(f"Module 2 check passed. Screenshots: {OUT}")


if __name__ == "__main__":
    main()
