#!/usr/bin/env python3
"""Module 1 check: the live preview (PARITY.md, Module 1).

Run after `cargo build --profile desktop -p reco-desktop`. Opens the fast
fixture pair (RECO_FIXTURE_LEFT/RIGHT/CAL, else the alfheim set) through
--left/--right/--calibration, zero-copy and then with --preview-readback,
and drives it through Makepad's --remote control. With the 5.3K match pair
present it also reports the real-footage frame rate. Screenshots go to
target/desktop-checks/m1/. Exits non-zero if any check failed.
"""
import os
import sys
import time

import drive

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "..", ".."))
BIN = os.path.join(ROOT, "target", "desktop", "reco-desktop")
OUT = os.path.join(ROOT, "target", "desktop-checks", "m1")
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
REAL_FPS = 30000 / 1001

FAILURES = []


def expect(ok, message):
    print(f"{'ok' if ok else 'FAIL'}: {message}")
    if not ok:
        FAILURES.append(message)


def launch(files, extra=()):
    left, right, cal = files
    return drive.App.launch(BIN, ["--window-size", "1280x820", "--left", left, "--right", right,
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


def seconds(clock):
    """'1:05' or '1:02:03' to seconds; None for anything else."""
    try:
        return drive.parse_cputime(clock)
    except (AttributeError, ValueError):
        return None


def samples(app, name):
    """Luminance on a 24 x 14 grid over the preview's drawn frame."""
    png = app.grab(os.path.join(OUT, f"{name}.png"))
    scale = png.width / app.get("/s")["w"][0]["sz"][0]
    x, y, w, h = app.rect("preview")
    out = []
    for j in range(14):
        for i in range(24):
            r, g, b = png.pixel(int((x + w * (i + 0.5) / 24) * scale), int((y + h * (j + 0.5) / 14) * scale))[:3]
            out.append(0.2126 * r + 0.7152 * g + 0.0722 * b)
    return out


def spread(values):
    mean = sum(values) / len(values)
    return (sum((v - mean) ** 2 for v in values) / len(values)) ** 0.5


def changed(a, b, tol=6):
    """Share of grid samples that differ by more than tol."""
    return sum(1 for p, q in zip(a, b) if abs(p - q) > tol) / len(a)


def drag(app, x, y, dx):
    app.get("/m", k="down", x=x, y=y)
    for step in range(1, 7):
        app.get("/m", k="move", x=x + dx * step / 6, y=y)
    app.get("/m", k="up", x=x + dx, y=y, wait=1)


def check_mode(mode, extra):
    name = f"{mode}"
    with launch(FAST, extra) as app:
        appeared = wait_for(lambda: app.rect("preview"), 30)
        expect(appeared is not None, f"{name}: the preview appears")
        if appeared is None:
            return
        expect(app.rect("empty_state") is None, f"{name}: the empty state is gone")
        log = "\n".join(app.log_lines())
        expect(f"{mode} on" in log, f"{name}: renders {mode}")

        first = samples(app, f"ready-{mode}")
        expect(spread(first) > 10, f"{name}: a real picture (spread {spread(first):.1f})")

        # Space plays; the clock and the picture move.
        t0 = text_of(app, "time_current")
        app.key("space")
        time.sleep(1.5)
        moving = samples(app, f"playing-{mode}")
        t1 = text_of(app, "time_current")
        expect(t1 != t0, f"{name}: Space plays ({t0} -> {t1})")
        expect(changed(first, moving) > 0.05, f"{name}: the picture moves while playing")

        # Space pauses: two grabs half a second apart are identical, and
        # the process sits at about 0% CPU (DESIGN.md Rule 8).
        app.key("space")
        time.sleep(0.6)
        still_a = samples(app, f"paused-{mode}")
        time.sleep(0.5)
        still_b = samples(app, f"paused-b-{mode}")
        expect(changed(still_a, still_b) == 0, f"{name}: paused frames stay still")
        c0 = app.cpu_seconds()
        time.sleep(3)
        idle = (app.cpu_seconds() - c0) / 3 * 100
        expect(idle < 5, f"{name}: about 0% CPU while paused ({idle:.1f}%)")

        # Drag pans; R resets.
        x, y, w, h = app.rect("preview")
        mid = (x + w / 2, y + h / 2)
        drag(app, mid[0], mid[1], 120)
        time.sleep(0.8)
        panned = samples(app, f"panned-{mode}")
        expect(changed(still_b, panned) > 0.2, f"{name}: dragging pans")
        app.key("r")
        time.sleep(1.2)
        reset = samples(app, f"reset-{mode}")
        expect(changed(still_b, reset) < changed(still_b, panned), f"{name}: R resets the view")

        # The wheel zooms (scrolling up/away zooms in).
        app.scroll(mid[0], mid[1], -120)
        time.sleep(0.8)
        zoomed = samples(app, f"zoomed-{mode}")
        expect(changed(reset, zoomed) > 0.2, f"{name}: the wheel zooms")
        app.key("r")
        time.sleep(1.0)

        # Arrow keys pan and = zooms, as the drag and the wheel do; F toggles
        # fullscreen (macOS: the window takes the screen and gives it back).
        keyed_from = samples(app, f"keys-from-{mode}")
        for _ in range(4):
            app.key("left")
        time.sleep(0.8)
        arrowed = samples(app, f"arrows-{mode}")
        expect(changed(keyed_from, arrowed) > 0.2, f"{name}: arrow keys pan")
        app.key("r")
        time.sleep(1.0)
        for _ in range(4):
            app.key("=")
        time.sleep(0.8)
        plussed = samples(app, f"equals-{mode}")
        expect(changed(keyed_from, plussed) > 0.2, f"{name}: = zooms in")
        app.key("r")
        time.sleep(1.0)
        size = app.get("/s")["w"][0]["sz"]
        app.key("f")
        time.sleep(1.5)
        full = app.get("/s")["w"][0]["sz"]
        app.key("f")
        time.sleep(1.5)
        back = app.get("/s")["w"][0]["sz"]
        expect(full != size and back == size, f"{name}: F toggles fullscreen ({size} -> {full} -> {back})")

        # ] seeks 5 s.
        before = seconds(text_of(app, "time_current"))
        app.key("]")
        time.sleep(1.0)
        after = seconds(text_of(app, "time_current"))
        expect(before is not None and after is not None and 4 <= after - before <= 6,
               f"{name}: ] seeks 5 s ({before} -> {after})")

        check_focus(app, name)

        # A pick with the mouse gives the keyboard back to the preview. The
        # list opens below the dropdown; its first row (Auto) is the menu's
        # 4 pt padding and half a 28 pt row down.
        app.click_id("aspect")
        time.sleep(0.3)
        ax, ay, aw, ah = app.rect("aspect")
        app.get("/click", x=ax + aw / 2, y=ay + ah + 4 + 14, wait=1)
        time.sleep(0.5)
        before = seconds(text_of(app, "time_current"))
        app.key("]")
        time.sleep(1.0)
        after = seconds(text_of(app, "time_current"))
        expect(before is not None and after is not None and after - before >= 4,
               f"{name}: keys reach the preview after a mouse pick of the aspect ({before} -> {after})")

        # Preview aspect 4:3 from the dropdown: a click opens it and gives it
        # the keyboard, and arrow keys step through the choices (the open
        # list's rows are not in the snapshot).
        app.click_id("aspect")
        app.key("down")
        app.key("down")
        app.key("return")
        time.sleep(1.0)
        samples(app, f"aspect-4x3-{mode}")
        x, y, w, h = app.rect("preview")
        expect(abs(w / h - 4 / 3) < 0.03, f"{name}: the preview letterboxes to 4:3 ({w}x{h})")

        expect(app.errors() == [], f"{name}: no errors in the app log")


def check_focus(app, name):
    """Mouse clicks leave the shortcuts with the preview (a click does not
    take the keyboard, as on macOS); a control focused with Tab keeps Space."""
    # ] still seeks after two clicks on Play.
    before = seconds(text_of(app, "time_current"))
    app.click_id("play_pause")
    app.click_id("play_pause")
    app.key("]")
    time.sleep(1.0)
    after = seconds(text_of(app, "time_current"))
    expect(before is not None and after is not None and after - before >= 4,
           f"{name}: keys still reach the preview after clicking Play ({before} -> {after})")

    # Space after clicking a panel toggle plays, and does not click it again.
    app.click_id("toggle_media")
    folded = wait_for(lambda: app.rect("setup_header") is None, 2)
    t0 = text_of(app, "time_current")
    app.key("space")
    time.sleep(1.5)
    t1 = text_of(app, "time_current")
    app.key("space")
    expect(folded and app.rect("setup_header") is None, f"{name}: Space does not click the panel toggle again")
    expect(t1 != t0, f"{name}: Space plays after clicking a panel toggle ({t0} -> {t1})")
    app.click_id("toggle_media")
    time.sleep(0.5)

    # A control focused from the keyboard keeps Space for itself.
    app.key("tab")
    held = text_of(app, "time_current")
    app.key("space")
    time.sleep(1.0)
    expect(text_of(app, "time_current") == held, f"{name}: Space on a Tab-focused control leaves the preview alone")
    app.key("escape")
    x, y, w, h = app.rect("preview")
    app.get("/click", x=x + w / 2, y=y + h / 2, wait=1)


def check_bad_video():
    left, _, cal = FAST
    junk = os.path.join(OUT, "not-a-video.mp4")
    with open(junk, "wb") as f:
        f.write(b"\x5a" * 4096)
    with launch((left, junk, cal)) as app:
        title = wait_for(lambda: text_of(app, "next_title") == "Couldn't open the videos", 20)
        app.grab(os.path.join(OUT, "bad-video.png"))
        expect(bool(title), "bad video: the viewer says it couldn't open the videos")
        body = text_of(app, "next_body") or ""
        expect("decoded" in body, f"bad video: it says no frame could be decoded ({body})")
        expect(app.rect("preview") is None, "bad video: no preview is drawn")
        expect(app.enabled("play_pause") is False, "bad video: Play stays disabled")


def check_bad_file():
    _, right, cal = FAST
    with launch(("/nonexistent/left.mp4", right, cal)) as app:
        title = wait_for(lambda: text_of(app, "next_title") == "Couldn't open the videos", 15)
        app.grab(os.path.join(OUT, "bad-file.png"))
        expect(bool(title), "bad file: the viewer says it couldn't open the videos")
        expect(app.rect("preview") is None, "bad file: no preview is drawn")


def check_real():
    if not all(os.path.exists(p) for p in REAL):
        print("skip: the 5.3K match pair is not on this machine")
        return
    with launch(REAL) as app:
        if wait_for(lambda: app.rect("preview"), 60) is None:
            expect(False, "real: the 5.3K preview appears")
            return
        app.key("space")
        time.sleep(4)
        samples(app, "real-5k")
        status = text_of(app, "status_text") or ""
        rate = float(status.split()[0]) if status.endswith("fps") else 0.0
        app.key("space")
        print(f"real: 5.3K playback at {rate:.1f} fps (source {REAL_FPS:.2f})")
        if rate < 0.8 * REAL_FPS:
            print("WARN: 5.3K playback below 80% of the source rate; record it in FRICTION.md")
        expect(app.errors() == [], "real: no errors in the app log")


def main():
    if not all(os.path.exists(p) for p in FAST):
        print("skip: fixtures not found (set RECO_FIXTURE_LEFT/RIGHT/CAL)")
        return
    os.makedirs(OUT, exist_ok=True)
    check_mode("zero-copy", ())
    check_mode("readback", ("--preview-readback",))
    check_bad_file()
    check_bad_video()
    check_real()
    if FAILURES:
        print(f"\nModule 1 check FAILED: {len(FAILURES)} failure(s):")
        for message in FAILURES:
            print(f"  - {message}")
        sys.exit(1)
    print(f"Module 1 check passed. Screenshots: {OUT}")


if __name__ == "__main__":
    main()
