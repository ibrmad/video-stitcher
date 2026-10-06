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

# A toast card's fill (theme.reco_band).
CARD = "#212121"

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
    # The ruler band, then the lanes (theme.reco_ruler_height, reco_lane_row).
    ruler, lane_row = 18, 18
    row = ruler / 2 if lane is None else ruler + lane_row * lane + lane_row / 2
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
        # While playing, the clock shows the scrub, not the frames playing on.
        app.key("space")
        time.sleep(1.0)
        (x2, _), (x3, _) = ruler_point(app, 45, 60.0), ruler_point(app, 50, 60.0)
        app.get("/m", k="down", x=x2, y=y0)
        app.get("/m", k="move", x=x3, y=y0, wait=1)
        time.sleep(0.6)
        held = seconds(text_of(app, "time_current"))
        app.get("/m", k="up", x=x3, y=y0, wait=1)
        app.key("space")
        expect(held is not None and abs(held - 50) <= 1, f"ruler: scrubbing while playing holds the clock ({held})")
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


def drawn(app, cards):
    """Whether each card is on screen where the snapshot puts it: its own
    fill inside its bottom right corner. The snapshot gives where a card is
    laid out, which a card kept in its own draw list may not be drawn at."""
    png = app.grab(os.path.join(OUT, "toasts-drawn.png"), scale=0.5)
    scale = png.width / app.get("/s")["w"][0]["sz"][0]
    for r in cards:
        if not r:
            return False
        x, y = r[0] + r[2] - 10, r[1] + r[3] - 6
        if not drive.close_to(png.pixel(int(x * scale), int(y * scale)), CARD, tol=4):
            return False
    return True


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
    check_toasts_follow()


def follows(app, gap, state):
    """The toasts still up keep `gap` to the viewer's right edge, and are
    drawn there."""
    cx_, cy_, cw_, ch_ = app.rect("canvas")
    now = [r for r in (app.rect(f"toast_{i}") for i in range(4)) if r]
    kept = bool(now) and all(abs(cx_ + cw_ - (r[0] + r[2]) - gap) <= 1 for r in now) and drawn(app, now)
    expect(kept, f"toasts: with Adjust {state}, they keep to the viewer's right edge and are drawn there "
                 f"({[r[0] + r[2] for r in now]} vs {cx_ + cw_ - gap})")


def check_toasts_follow():
    """Toasts keep to the viewer's edge, on screen and not only in the
    layout, whatever moves it: a panel's slide, a dragged edge, a narrow
    window (the owner saw one left under the Adjust panel after a
    calibration opened it)."""
    with launch(extra=("--toast-demo",)) as app:
        ready(app)
        x, y, w, h = app.rect("canvas")
        first = app.rect("toast_0")
        gap = x + w - (first[0] + first[2])
        expect(drawn(app, [first]), "toasts: drawn where they are laid out to start")
        app.key("Key2", cmd=1)
        time.sleep(0.6)
        follows(app, gap, "closed")
        app.key("Key2", cmd=1)
        time.sleep(0.6)
        follows(app, gap, "open")
        ix, iy, iw, ih = app.rect("inspector")
        bar_x, bar_y = ix - 3, iy + ih / 2
        app.get("/m", k="down", x=bar_x, y=bar_y)
        app.get("/m", k="move", x=bar_x - 50, y=bar_y)
        app.get("/m", k="move", x=bar_x - 100, y=bar_y)
        app.get("/m", k="up", x=bar_x - 100, y=bar_y, wait=1)
        time.sleep(0.3)
        follows(app, gap, "dragged wider")
        for size, state in (((900, 820), "folded by a narrow window"), ((1280, 820), "back")):
            app.get("/w", k="resize", width=size[0], height=size[1], wait=1)
            time.sleep(0.6)
            follows(app, gap, state)


def ffprobe(path):
    """(width, height, frames) of a video, or None without ffprobe."""
    tool = shutil.which("ffprobe")
    if tool is None:
        return None
    out = subprocess.run([tool, "-v", "error", "-select_streams", "v:0", "-count_frames",
                          "-show_entries", "stream=width,height,nb_read_frames", "-of", "json", path],
                         capture_output=True, text=True)
    stream = (json.loads(out.stdout or "{}").get("streams") or [{}])[0]
    return stream.get("width"), stream.get("height"), int(stream.get("nb_read_frames") or 0)


def recordings(folder):
    return sorted(f for f in os.listdir(folder) if f.startswith("reco_recording_") and f.endswith(".mp4"))


def saved(config):
    path = os.path.join(config, "desktop.json")
    return json.load(open(path)) if os.path.exists(path) else {}


def menu_rows(app):
    """The open menu's rows, {text: rect} (its template rows' labels)."""
    return {i.get("t"): i["r"] for i in app.snap("label") if i.get("i") == "label"}


def record_menu(app):
    """Open the record menu by its ▾; its rows ({} when it didn't open)."""
    app.click_id("record_menu_button")
    if not wait_for(lambda: app.rect("record_menu_list"), 5):
        return {}
    time.sleep(0.3)
    return menu_rows(app)


def chosen(app):
    """The open menu's row marked ✓, or None (none, or more than one)."""
    marks = [i["r"] for i in app.snap("mark") if i.get("i") == "mark" and i.get("t") == "✓"]
    if len(marks) != 1:
        return None
    middle = marks[0][1] + marks[0][3] / 2
    for text, r in menu_rows(app).items():
        if abs(r[1] + r[3] / 2 - middle) < 4:
            return text
    return None


def pick(app, rows, text):
    """Click the open menu's row `text`; whether it was there."""
    r = rows.get(text)
    if r:
        app.get("/click", x=r[0] + 20, y=r[1] + r[3] / 2, wait=1)
    return r is not None


def recording(app):
    """Whether the Record button shows a recording's time."""
    return (text_of(app, "record_button") or "Record") != "Record"


def check_record():
    """Record: one button starts and stops (the time on it while recording),
    and the ▾ beside it opens a menu with the quality (✓ on the current one,
    remembered) and "Codec and folder…" (Preferences), off while recording;
    the view bar stays still when recording starts; a 1920x1080 file with
    one frame per frame played; notices naming the file; Show in folder;
    quitting while recording still leaves a playable file. Never clicks Show
    in folder (it opens Finder)."""
    config = tempfile.mkdtemp(prefix="reco-desktop-config-")
    folder = tempfile.mkdtemp(prefix="reco-desktop-recordings-")
    with open(os.path.join(config, "desktop.json"), "w") as f:
        json.dump({"recording_folder": folder}, f)
    with launch(config_dir=config) as app:
        ready(app)
        time.sleep(1.0)
        expect(text_of(app, "record_button") == "Record",
               f"record: the button says Record ({text_of(app, 'record_button')})")
        rows = record_menu(app)
        expect({"Fast", "Balanced", "High", "Codec and folder…"} <= set(rows),
               f"record: the menu has the qualities and Codec and folder… ({sorted(r for r in rows if r)})")
        expect(chosen(app) == "Balanced", f"record: ✓ on the current quality ({chosen(app)})")
        save_shot(app, "record-menu")
        pick(app, rows, "High")
        expect(bool(wait_for(lambda: app.rect("record_menu_list") is None, 3)), "record: picking a quality closes the menu")
        expect(saved(config).get("recording_quality") == "high",
               f"record: the quality is remembered ({saved(config).get('recording_quality')})")
        rows = record_menu(app)
        expect(chosen(app) == "High", f"record: the ✓ moves to it ({chosen(app)})")
        pick(app, rows, "Codec and folder…")
        expect(bool(wait_for(lambda: app.rect("prefs_record_codec"), 5)), "record: Codec and folder… opens Preferences")
        app.key("Escape")
        time.sleep(0.5)

        bar_before = [app.rect(wid) for wid in ("aspect", "record_button", "record_menu_button")]
        app.click_id("record_button")
        expect(bool(wait_for(lambda: title_rect(app, "Recording started"), 10)), "record: a toast says recording started")
        body = next((i.get("t", "") for i in app.snap("reco_recording_") if i.get("i") == "body"), "")
        expect(body.startswith("reco_recording_") and "/" not in body,
               f"record: the toast names the file, not its path ({body!r})")
        expect(bool(wait_for(lambda: recording(app), 3)),
               f"record: while recording the button shows the time ({text_of(app, 'record_button')})")
        expect(app.enabled("record_menu_button") is False, "record: the menu is off while recording")
        bar_during = [app.rect(wid) for wid in ("aspect", "record_button", "record_menu_button")]
        expect(bar_during == bar_before,
               f"record: nothing in the view bar moves when recording starts ({bar_before} -> {bar_during})")
        save_shot(app, "recording")
        app.key("space")
        time.sleep(3.0)
        status = text_of(app, "status_text") or ""
        expect(status.startswith("Recording ·"), f"record: the status line says recording ({status})")
        shown = text_of(app, "record_button") or ""
        expect(shown not in ("0:00", "Record") and seconds(shown) is not None,
               f"record: the button's time counts ({shown})")
        app.key("space")
        time.sleep(0.5)
        app.click_id("record_button")
        expect(bool(wait_for(lambda: title_rect(app, "Recording saved"), 15)), "record: a toast says the recording was saved")
        expect(app.rect("show_in_folder") is not None, "record: Show in folder appears")
        expect(text_of(app, "record_button") == "Record" and app.enabled("record_menu_button") is True,
               "record: the button says Record again and the menu is back")
        body = next((i.get("t", "") for i in app.snap("frames ·") if i.get("i") == "body"), "")
        files = recordings(folder)
        expect(len(files) == 1, f"record: one file in the recording folder ({files})")
        probe = ffprobe(os.path.join(folder, files[0])) if files else None
        if files and probe is None:
            print("skip: ffprobe is not installed; the file's size and frames are not checked")
        elif probe:
            w, h, frames = probe
            expect((w, h) == (1920, 1080), f"record: 1920x1080 for Auto ({w}x{h})")
            expect(80 <= frames <= 100, f"record: about 3 s at 30 fps, one frame per frame played ({frames})")
            told = body.split()[0] if body else ""
            expect(told == str(frames), f"record: the toast counts the frames written ({body!r} vs {frames})")
        # Three short recordings: six notices in a few seconds, all still due.
        # Each starts in a new second: recordings are named by the second,
        # and notices with the same words merge into one.
        for _ in range(3):
            time.sleep(1.0 - time.time() % 1.0 + 0.05)
            app.click_id("record_button")
            wait_for(lambda: recording(app), 10)
            app.click_id("record_button")
            wait_for(lambda: not recording(app), 10)
        slots = [app.rect(f"toast_{i}") is not None for i in range(4)]
        expect(all(slots), f"record: more notices than fit show four ({slots})")
        expect(app.errors() == [], "record: no errors in the app log")
    with launch(config_dir=config) as app:
        ready(app)
        before = set(recordings(folder))
        app.click_id("record_button")
        wait_for(lambda: title_rect(app, "Recording started"), 10)
        app.key("space")
        time.sleep(1.5)
    new = sorted(set(recordings(folder)) - before)
    expect(len(new) == 1, f"record: quitting while recording leaves a file ({new})")
    if new and shutil.which("ffprobe"):
        expect(ffprobe(os.path.join(folder, new[0]))[2] > 0, "record: and it plays")


def check_perf():
    """Rule 8: drawing a frame takes under 4 ms on average while playing."""
    for name, files in (("fast", FAST), ("real", REAL)):
        if not all(os.path.exists(p) for p in files):
            print(f"skip: perf {name}: fixtures not found")
            continue
        with launch(files, extra=("--perf-log",)) as app:
            ready(app)
            app.key("space")
            time.sleep(6.5)
            app.key("space")
            lines = [line for line in app.log_lines() if "ui draw:" in line]
        averages = [float(line.split("avg ")[1].split(" ms")[0]) for line in lines]
        print(f"perf {name}: {lines[-1].split('- ')[-1] if lines else 'no summary'}")
        expect(len(averages) >= 2, f"perf {name}: draw times are logged ({len(averages)} summaries)")
        expect(all(a < 4.0 for a in averages), f"perf {name}: under 4 ms a frame on average ({averages})")


CHECKS = {
    "persist": check_persist,
    "transport": check_transport,
    "ruler": check_ruler,
    "toasts": check_toasts,
    "record": check_record,
    "perf": check_perf,
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
