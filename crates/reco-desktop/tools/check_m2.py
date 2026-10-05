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


def save_shot(app, name):
    """Save a screenshot without decoding it (decoding takes seconds)."""
    shutil.copyfile(app.get("/g", scale=1.0)["png"], os.path.join(OUT, f"{name}.png"))


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


def ruler_point(app, secs, length, lane=None):
    """Window point at `secs` on the ruler: in the ruler band, or in lane 0/1."""
    x, y, w, h = app.rect("timeline")
    row = 8 if lane is None else 16 + 16 * lane + 8
    return x + w * secs / length, y + row


def pixel_at(png, app, point):
    scale = png.width / app.get("/s")["w"][0]["sz"][0]
    return png.pixel(int(point[0] * scale), int(point[1] * scale))[:3]


def check_ruler():
    """Scrubbing seeks on release; lanes come from the files; the export range is tinted."""
    with launch() as app:
        ready(app)
        time.sleep(1.0)
        (x0, y0), (x1, _) = ruler_point(app, 6, 60.0), ruler_point(app, 30, 60.0)
        app.get("/m", k="down", x=x0, y=y0)
        for step in range(1, 6):
            app.get("/m", k="move", x=x0 + (x1 - x0) * step / 5, y=y0, wait=1)
        during = seconds(text_of(app, "time_current"))
        app.get("/m", k="up", x=x1, y=y0, wait=1)
        expect(during is not None and abs(during - 30) <= 1, f"ruler: the clock follows a scrub ({during})")
        time.sleep(2.0)
        after = seconds(text_of(app, "time_current"))
        expect(after is not None and abs(after - 30) <= 1, f"ruler: releasing seeks there ({after})")
        expect(app.errors() == [], "ruler: no errors in the app log")

    left, right, cal = FAST
    with launch((f"{left};{left}", f"{right};{right}", cal)) as app:
        ready(app)
        total = wait_for(lambda: text_of(app, "time_total") == "2:00", 20)
        expect(bool(total), f"ruler: the length comes from the files ({text_of(app, 'time_total')})")
        png = app.grab(os.path.join(OUT, "lanes-chained.png"))
        block = pixel_at(png, app, ruler_point(app, 30, 120.0, lane=0))
        gap = min((pixel_at(png, app, ruler_point(app, 60 + d / 20, 120.0, lane=0)) for d in range(-20, 21)), key=sum)
        expect(sum(gap) + 60 < sum(block), f"ruler: a gap between the two files ({gap} vs {block})")

    with launch(extra=("--export-range", "10-40")) as app:
        ready(app)
        time.sleep(1.0)
        png = app.grab(os.path.join(OUT, "export-range.png"))
        tinted = pixel_at(png, app, ruler_point(app, 27.5, 60.0, lane=0))
        plain = pixel_at(png, app, ruler_point(app, 52.5, 60.0, lane=0))
        expect(tinted[1] > plain[1] + 4, f"ruler: the export range is tinted ({tinted} vs {plain})")


def title_rect(app, text):
    """The rect of a toast title showing `text`, or None."""
    for item in app.snap(text):
        if item.get("t") == text and item.get("i") == "title":
            return item["r"]
    return None


def check_toasts():
    """At most four, newest at the bottom, inside the viewer; close and
    expiry; the status line keeps its own text; a failed open raises one."""
    with launch(extra=("--toast-demo",)) as app:
        ready(app)
        shown = [app.rect(f"toast_{i}") for i in range(4)]
        save_shot(app, "toasts")
        expect(all(shown), f"toasts: four show ({sum(1 for r in shown if r)})")
        expect(title_rect(app, "Calibration saved") is None, "toasts: the oldest of five left first")
        newest, older = title_rect(app, "Recording saved"), title_rect(app, "Couldn't open the videos")
        expect(bool(newest and older and newest[1] > older[1]), "toasts: the newest is at the bottom")
        x, y, w, h = app.rect("canvas")
        inside = all(r and r[0] >= x and r[0] + r[2] <= x + w and r[1] + r[3] <= y + h for r in shown)
        expect(inside, "toasts: inside the viewer, clear of the Adjust panel and the time panel")
        expect(text_of(app, "status_text") == "Ready", f"toasts: the status line keeps its own text ({text_of(app, 'status_text')})")
        close = app.rect("close")
        if close:
            app.get("/click", x=close[0] + close[2] / 2, y=close[1] + close[3] / 2, wait=1)
        expect(close is not None, "toasts: a toast has a close button")
        expect(title_rect(app, "Recording started") is None, "toasts: a close button dismisses its toast")
        gone = wait_for(lambda: title_rect(app, "Recording saved") is None, 6)
        expect(bool(gone), "toasts: an info toast leaves after about four seconds")
        expect(title_rect(app, "Low calibration confidence") is not None, "toasts: a warning stays longer")
        expect(app.errors() == [], "toasts: no errors in the app log")
    _, right, cal = FAST
    with launch(("/nonexistent/left.mp4", right, cal)) as app:
        failed = wait_for(lambda: title_rect(app, "Couldn't open the videos"), 15)
        expect(bool(failed), "toasts: a failed open raises an error toast")


CHECKS = {
    "persist": check_persist,
    "transport": check_transport,
    "ruler": check_ruler,
    "toasts": check_toasts,
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
