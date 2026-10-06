#!/usr/bin/env python3
"""Module 5 check: camera and lens (PARITY.md, Module 5).

Run after `cargo build --profile desktop -p reco-desktop`. Opens the fast
fixture pair with a temporary copy of its calibration (so saving never
touches the fixture). Each launch gets its own settings folder.
Screenshots go to target/desktop-checks/m5/. `check_m5.py NAME...` runs the
named checks (all by default); exits non-zero if any check failed.
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
OUT = os.path.join(ROOT, "target", "desktop-checks", "m5")
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


def text_of(app, widget_id):
    """A label's or button's text, or a text field's value."""
    for item in app.snap(widget_id):
        if item.get("i") == widget_id:
            return item.get("t", item.get("val", ""))
    return None


def title_rect(app, text):
    for item in app.snap(text):
        if item.get("t") == text and item.get("i") == "title":
            return item["r"]
    return None


def click(app, widget_id):
    r = app.rect(widget_id)
    if r:
        app.get("/click", x=r[0] + r[2] / 2, y=r[1] + r[3] / 2, wait=1)
    return r is not None


def drag(app, slider_id, by):
    """Drag a slider by `by` of its width (Makepad's sliders move relative
    to where the knob was; more than 1 reaches an end); whether it was
    there."""
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


def open_advanced(app, fold_id):
    """Open an Advanced tier by its chevron."""
    r = app.rect(fold_id)
    if r:
        app.get("/click", x=r[0] + 18, y=r[1] + 12, wait=1)
        time.sleep(0.5)


def frame_pixels(app, name):
    png = app.grab(os.path.join(OUT, f"{name}.png"), scale=0.5)
    scale = png.width / app.get("/s")["w"][0]["sz"][0]
    x, y, w, h = app.rect("preview")
    return [png.pixel(int((x + i) * scale), int((y + j) * scale))[:3]
            for j in range(4, int(h) - 4, 16) for i in range(4, int(w) - 4, 16)]


def calibration_copy():
    folder = tempfile.mkdtemp(prefix="reco-m5-")
    cal = os.path.join(folder, "match.json")
    shutil.copyfile(FAST[2], cal)
    return (FAST[0], FAST[1], cal)


def launch(files, answers=None, env=None):
    left, right, cal = files
    args = ["--window-size", "1280x980", "--left", left, "--right", right, "--calibration", cal]
    env = dict(env) if env else None
    if answers is not None:
        fd, path = tempfile.mkstemp(prefix="reco-m5-answers-", suffix=".json")
        with os.fdopen(fd, "w") as f:
            json.dump(answers, f)
        env = {**(env or {}), "RECO_DESKTOP_DIALOG_ANSWERS": path}
    return drive.App.launch(BIN, args, env=env)


def type_into(app, widget_id, text):
    """Replace a text field's text with `text`."""
    click(app, widget_id)
    app.key("KeyA", cmd=1)
    app.get("/k", t=text)


def texts(app, widget_id):
    """Every drawn widget's text with this id (template rows share ids)."""
    return [i.get("t", "") for i in app.snap(widget_id) if i.get("i") == widget_id]


def degrees(text):
    return float((text or "nan°").rstrip("°"))


def check_view():
    """The field of view starts where the picture allows, follows its
    slider and the zoom keys; stay inside can be switched off; Reset view
    goes back."""
    files = calibration_copy()
    with launch(files) as app:
        expect(wait_for(lambda: app.rect("preview"), 30) is not None, "view: the preview opens")
        opening = wait_for(lambda: text_of(app, "fov_value") if text_of(app, "fov_value") != "75°" else None, 10)
        expect(opening is not None and degrees(opening) < 75,
               f"view: the opening field of view is what the picture allows ({opening})")
        before = frame_pixels(app, "view-before")
        # The first drag, with the Advanced tier under it closed (a closed
        # fold's hidden rows used to take it).
        drag(app, "fov_slider", -1.5)
        time.sleep(0.8)
        narrow = text_of(app, "fov_value")
        expect(degrees(narrow) == 20, f"view: the slider narrows the view ({narrow})")
        after = frame_pixels(app, "view-narrow")
        expect(before != after, "view: the picture follows")
        click(app, "preview")
        app.key("-")
        time.sleep(0.8)
        expect(degrees(text_of(app, "fov_value")) > degrees(narrow),
               f"view: the zoom keys move the slider ({narrow} -> {text_of(app, 'fov_value')})")
        drag(app, "fov_slider", 1.5)
        time.sleep(0.8)
        capped = degrees(text_of(app, "fov_value"))
        expect(capped < 150, f"view: staying inside caps the field of view ({capped:.0f}°)")
        open_advanced(app, "view_advanced")
        click(app, "constrained_look")
        time.sleep(0.5)
        drag(app, "fov_slider", 1.5)
        time.sleep(0.8)
        expect(degrees(text_of(app, "fov_value")) == 150,
               f"view: switched off, the whole range is free ({text_of(app, 'fov_value')})")
        click(app, "reset_view")
        time.sleep(0.8)
        expect(degrees(text_of(app, "fov_value")) == 75,
               f"view: Reset view goes back ({text_of(app, 'fov_value')})")
        expect(app.errors() == [], f"view: no errors in the app log {app.errors()[:3]}")


def check_lens():
    """Each camera's lens is named; a fine-tune slider moves the picture and
    marks the calibration unsaved; Reset lens goes back; lens correction is
    saved with the calibration."""
    files = calibration_copy()
    loaded = json.load(open(files[2]))
    with launch(files) as app:
        expect(wait_for(lambda: app.rect("preview"), 30) is not None, "lens: the preview opens")
        named = wait_for(lambda: text_of(app, "left_lens_name") not in (None, "", "Looking up…"), 20)
        expect(bool(named), f"lens: the left lens is named ({text_of(app, 'left_lens_name')})")
        expect(text_of(app, "right_lens_name") not in (None, "", "Looking up…"),
               f"lens: the right lens is named ({text_of(app, 'right_lens_name')})")
        open_advanced(app, "lens_advanced")
        fx = f"{loaded['left_uniforms']['fx']:.0f}"
        expect(text_of(app, "lens_fx_value") == fx, f"lens: fine-tune shows the file's focal length ({text_of(app, 'lens_fx_value')} vs {fx})")
        expect(app.enabled("reset_lens") is False, "lens: nothing to reset yet")
        pick_row(app, "lens_camera", 2)
        before = frame_pixels(app, "lens-before")
        drag(app, "lens_fx", 0.4)
        time.sleep(0.8)
        moved = text_of(app, "lens_fx_value")
        expect(moved not in (None, fx), f"lens: the focal length follows its slider ({moved})")
        after = frame_pixels(app, "lens-after")
        expect(before != after, "lens: the picture follows")
        expect(wait_for(lambda: app.enabled("reset_lens"), 5) is True, "lens: Reset lens can go back")
        expect(bool(wait_for(lambda: app.rect("save_calibration"), 5)), "lens: Save appears")
        # A typed focal length, and one past the range's end, which stops there.
        typed = str(int(fx) + 20)
        click(app, "lens_fx_value")
        time.sleep(0.3)
        app.get("/k", t=typed)
        app.key("return")
        time.sleep(0.5)
        expect(text_of(app, "lens_fx_value") == typed, f"lens: a typed focal length ({text_of(app, 'lens_fx_value')} vs {typed})")
        click(app, "lens_fx_value")
        time.sleep(0.3)
        app.get("/k", t="99999")
        app.key("return")
        time.sleep(0.5)
        end = text_of(app, "lens_fx_value") or ""
        expect(end.isdigit() and int(typed) < int(end) < 99999,
               f"lens: past the range, its end ({end})")
        click(app, "reset_lens")
        time.sleep(0.8)
        expect(text_of(app, "lens_fx_value") == fx, f"lens: Reset lens restores the file's lens ({text_of(app, 'lens_fx_value')})")
        expect(wait_for(lambda: app.enabled("reset_lens") is False, 5), "lens: and has nothing left to reset")
        click(app, "lens_correction")
        time.sleep(0.8)
        flat = frame_pixels(app, "lens-uncorrected")
        expect(flat != before, "lens: switching correction off changes the picture")
        click(app, "save_calibration")
        expect(bool(wait_for(lambda: title_rect(app, "Calibration saved"), 10)), "lens: saved")
        written = json.load(open(files[2]))
        expect(written.get("lens_correction_amount") == 0.0,
               f"lens: the file keeps correction off ({written.get('lens_correction_amount')})")
        expect(app.errors() == [], f"lens: no errors in the app log {app.errors()[:3]}")


def pick_row(app, dropdown_id, index):
    """Choose row `index` of a dropdown with the keyboard (menu rows are
    not in the snapshot)."""
    click(app, dropdown_id)
    time.sleep(0.3)
    # The menu opens on the current row: go to the top first.
    for _ in range(4):
        app.key("up")
    for _ in range(index):
        app.key("down")
    app.key("return")
    time.sleep(0.8)


def check_preview():
    """Show puts one camera in the preview, flat and inside the frame; the
    side matters; Stitched picture goes back."""
    files = calibration_copy()
    with launch(files) as app:
        expect(wait_for(lambda: app.rect("preview"), 30) is not None, "preview: the preview opens")
        time.sleep(1.0)
        stitched = frame_pixels(app, "preview-stitched")
        pick_row(app, "lens_preview", 1)
        left = frame_pixels(app, "preview-left")
        expect(left != stitched, "preview: Left camera shows one camera")
        x, y, w, h = app.rect("preview")
        png = app.grab(os.path.join(OUT, "preview-left-full.png"), scale=0.5)
        scale = png.width / app.get("/s")["w"][0]["sz"][0]
        edge = [png.pixel(int((x + 3) * scale), int((y + j) * scale))[:3] for j in range(10, int(h) - 10, 20)]
        expect(all(max(p) < 16 for p in edge), "preview: the camera keeps its shape, black at the sides")
        pick_row(app, "lens_preview", 2)
        right = frame_pixels(app, "preview-right")
        expect(right != left, "preview: Right camera shows the other one")
        pick_row(app, "lens_preview", 0)
        back = frame_pixels(app, "preview-back")
        expect(back == stitched, "preview: Stitched picture goes back")
        expect(app.errors() == [], f"preview: no errors in the app log {app.errors()[:3]}")


def check_picker():
    """The lens picker searches Reco's profiles and applies one to both
    cameras, naming it; a profile file loads for one camera through the
    dialog."""
    files = calibration_copy()
    lens_file = os.path.join(os.path.dirname(files[2]), "custom_lens.json")
    with open(lens_file, "w") as f:
        json.dump({"width": 1280, "height": 960, "fx": 700.0, "fy": 700.0, "cx": 640.0,
                   "cy": 480.0, "d": [0.05, 0.01, 0.0, 0.0]}, f)
    with launch(files, answers={"lens": [lens_file]}) as app:
        expect(wait_for(lambda: app.rect("preview"), 30) is not None, "picker: the preview opens")
        wait_for(lambda: text_of(app, "left_lens_name") not in (None, "", "Looking up…"), 20)
        before = frame_pixels(app, "picker-before")
        click(app, "lens_browse")
        expect(bool(wait_for(lambda: app.rect("picker_search"), 5)), "picker: the lens profiles button opens it")
        expect(text_of(app, "picker_hint") == "Type to search over 4,200 camera profiles.",
               f"picker: it asks for a search ({text_of(app, 'picker_hint')})")
        type_into(app, "picker_search", "hero9")
        found = wait_for(lambda: [t for t in texts(app, "line") if "HERO9" in t.upper()], 10)
        expect(bool(found), f"picker: a search lists matching profiles ({(found or [''])[0]})")
        expect((text_of(app, "picker_hint") or "").endswith("profiles"),
               f"picker: and says how many ({text_of(app, 'picker_hint')})")
        save_shot(app, "picker")
        r = app.rect("picker_results")
        app.get("/click", x=r[0] + 40, y=r[1] + 12, wait=1)
        expect(bool(wait_for(lambda: title_rect(app, "Lens profile applied"), 5)), "picker: picking applies it")
        left, right = text_of(app, "left_lens_name"), text_of(app, "right_lens_name")
        expect((left or "").endswith("(picked)") and (right or "").endswith("(picked)"),
               f"picker: both cameras are named by it ({left} | {right})")
        open_advanced(app, "lens_advanced")
        expect(wait_for(lambda: app.enabled("reset_lens"), 5) is True, "picker: Reset lens can go back")
        time.sleep(0.6)
        expect(frame_pixels(app, "picker-after") != before, "picker: the picture follows")
        click(app, "lens_browse")
        time.sleep(0.5)
        pick_row(app, "picker_cameras", 1)
        click(app, "picker_file")
        left = wait_for(lambda: text_of(app, "left_lens_name") if (text_of(app, "left_lens_name") or "").endswith("(file)") else None, 5)
        expect(left == "custom_lens (file)", f"picker: a file loads for the left camera ({text_of(app, 'left_lens_name')})")
        expect((text_of(app, "right_lens_name") or "").endswith("(picked)"),
               f"picker: the right camera keeps its lens ({text_of(app, 'right_lens_name')})")
        expect(app.errors() == [], f"picker: no errors in the app log {app.errors()[:3]}")


def logged(app, needle):
    return any(needle in line for line in app.log_lines())


def visible_rows(app):
    """The lens picker's drawn result rows, top to bottom: (text, rect)."""
    rows = [(i.get("t", ""), i["r"]) for i in app.snap("line") if i.get("i") == "line"]
    return sorted(rows, key=lambda row: row[1][1])


def check_picker_scroll():
    """A list longer than the picker scrolls, and every row can be picked:
    scrolled to the end, the row under the pointer is the one applied; a
    new search starts at the top."""
    files = calibration_copy()
    with launch(files) as app:
        expect(wait_for(lambda: app.rect("preview"), 30) is not None, "scroll: the preview opens")
        wait_for(lambda: text_of(app, "left_lens_name") not in (None, "", "Looking up…"), 20)
        click(app, "lens_browse")
        wait_for(lambda: app.rect("picker_search"), 5)
        type_into(app, "picker_search", "hero9 wide 5")
        expect(wait_for(lambda: text_of(app, "picker_hint") == "12 profiles", 10) is not None,
               f"scroll: the search finds more profiles than the list shows ({text_of(app, 'picker_hint')})")
        time.sleep(0.4)
        top = visible_rows(app)
        r = app.rect("picker_results")
        for _ in range(4):
            app.scroll(r[0] + 100, r[1] + 100, 60)
            time.sleep(0.2)
        time.sleep(0.4)
        rows = visible_rows(app)
        expect(rows and top and rows[0][0] != top[0][0], "scroll: the wheel scrolls the list")
        # One of the last two rows, unlike the row two above it (a pick that
        # missed by the scroll would land there).
        texts = [t for t, _ in rows]
        at = next((i for i in (len(texts) - 1, len(texts) - 2) if i >= 2 and texts[i] != texts[i - 2]), None)
        expect(at is not None, f"scroll: the last rows differ from the ones above ({texts[-4:]})")
        if at is None:
            return
        text, rect = rows[at]
        x = r[0] + 60
        y = rect[1] + rect[3] / 2
        app.get("/m", k="move", x=x, y=y, wait=1)
        time.sleep(0.3)
        png = app.grab(os.path.join(OUT, "probe.png"))
        scale = png.width / app.get("/s")["w"][0]["sz"][0]
        under = png.pixel(int((r[0] + r[2] - 20) * scale), int(y * scale))[:3]
        above = png.pixel(int((r[0] + r[2] - 20) * scale), int((y - 48) * scale))[:3]
        expect(sum(under) > sum(above), f"scroll: the row under the pointer is highlighted ({under} vs {above} two rows up)")
        save_shot(app, "picker-scrolled")
        app.get("/click", x=x, y=y, wait=1)
        name = text.rsplit(" · ", 1)[0]
        expect(wait_for(lambda: logged(app, f"lens: {name} for"), 5) is not None,
               f"scroll: picking a row near the end applies that row ({name})")
        last = texts[-1].rsplit(" · ", 1)[0]
        click(app, "lens_browse")
        time.sleep(0.6)
        # The keys: a press released off the list gives it the keyboard
        # without picking; ↓ to the last row scrolls it into view.
        app.get("/m", k="down", x=x, y=r[1] + 12, wait=1)
        hint = app.rect("picker_hint")
        app.get("/m", k="move", x=x, y=hint[1] + 4, wait=1)
        app.get("/m", k="up", x=x, y=hint[1] + 4, wait=1)
        for _ in range(12):
            app.key("down")
        time.sleep(0.5)
        shown = visible_rows(app)
        expect([t for t, _ in shown] == texts, "scroll: ↓ to the last row scrolls the list to its end")
        png = app.grab(os.path.join(OUT, "probe.png"))
        lit = lambda row: sum(png.pixel(int((r[0] + r[2] - 20) * scale), int((row[1][1] + row[1][3] / 2) * scale))[:3])
        expect(lit(shown[-1]) > lit(shown[-3]), "scroll: and highlights the last row")
        picks = sum("lens: " in line for line in app.log_lines())
        app.key("return")
        expect(wait_for(lambda: sum("lens: " in line for line in app.log_lines()) > picks, 5) is not None
               and logged(app, f"lens: {last} for"), f"scroll: Return picks it ({last})")
        click(app, "lens_browse")
        time.sleep(0.4)
        type_into(app, "picker_search", "hero9 wide")
        wait_for(lambda: (text_of(app, "picker_hint") or "").endswith("profiles"), 10)
        time.sleep(0.4)
        first = visible_rows(app)
        for _ in range(4):
            app.scroll(r[0] + 100, r[1] + 100, -60)
            time.sleep(0.2)
        time.sleep(0.4)
        expect(first and visible_rows(app)[0][0] == first[0][0],
               "scroll: a new search starts at the top of the list")
        expect(app.errors() == [], f"scroll: no errors in the app log {app.errors()[:3]}")


def save_shot(app, name):
    shutil.copyfile(app.get("/g", scale=1.0)["png"], os.path.join(OUT, f"{name}.png"))


def check_stats():
    """The Stats section names the GPU and shows the preview's figures while
    it plays."""
    files = calibration_copy()
    with launch(files) as app:
        expect(wait_for(lambda: app.rect("preview"), 30) is not None, "stats: the preview opens")
        open_advanced(app, "stats_section")
        gpu = wait_for(lambda: text_of(app, "stats_gpu") if text_of(app, "stats_gpu") not in (None, "", "—") else None, 5)
        expect(bool(gpu), f"stats: the GPU is named ({gpu})")
        click(app, "play_pause")
        fps = wait_for(lambda: text_of(app, "stats_fps") if (text_of(app, "stats_fps") or "").endswith("fps") else None, 5)
        expect(bool(fps) and float(fps.split()[0]) > 20,
               f"stats: the first second of playing reads near the source's 30 fps ({fps})")
        frame, slowest = text_of(app, "stats_frame") or "", text_of(app, "stats_slowest") or ""
        expect(frame.endswith("ms") and slowest.endswith("ms"), f"stats: the frame time and the slowest ({frame}, {slowest})")
        steady = wait_for(lambda: float((text_of(app, "stats_fps") or "0 fps").split()[0]) > 20, 4)
        expect(bool(steady), f"stats: a second of playing reads near the source's 30 fps ({text_of(app, 'stats_fps')})")
        expect((text_of(app, "stats_decode") or "").endswith("ms") and (text_of(app, "stats_render") or "").endswith("ms"),
               "stats: decode and render times")
        click(app, "play_pause")
        save_shot(app, "stats")
        expect(app.errors() == [], f"stats: no errors in the app log {app.errors()[:3]}")


def press_drag(app, x, y, dx):
    """Press at (x, y), drag `dx` points across, release."""
    app.get("/m", k="down", x=x, y=y, wait=1)
    for step in range(1, 7):
        app.get("/m", k="move", x=x + dx * step / 6, y=y, wait=1)
    app.get("/m", k="up", x=x + dx, y=y, wait=1)


def check_cursor():
    """Sliders show the arrow, as macOS's own sliders do (the owner chose it
    over Makepad's open and closed hand), never the text cursor (Makepad's
    slider holds a number field for typing its value: Reco hides its
    readout, and the owner saw the I-beam over the empty field). A press
    anywhere on the slider's box drags it, the track's ends and edges too
    (the empty field took presses at the right end). RECO_DESKTOP_LOG_CURSOR
    logs each cursor change."""
    files = calibration_copy()
    with launch(files, env={"RECO_DESKTOP_LOG_CURSOR": "1"}) as app:
        expect(wait_for(lambda: app.rect("preview"), 30) is not None, "cursor: the preview opens")
        for slider in ("fov_slider", "seam_blend"):
            x, y, w, h = app.rect(slider)
            for row in (0.3, 0.5, 0.7):
                for step in range(25):
                    app.get("/m", k="move", x=x + w * step / 24, y=y + h * row, wait=1)
        x, y, w, h = app.rect("fov_slider")
        app.get("/m", k="down", x=x + w / 2, y=y + h / 2, wait=1)
        app.get("/m", k="up", x=x + w / 2, y=y + h / 2, wait=1)
        cursors = {line.split("cursor: ")[1].strip() for line in app.log_lines() if "cursor: " in line}
        expect(bool(cursors), f"cursor: the moves are logged ({sorted(cursors)})")
        expect("Text" not in cursors, f"cursor: never a text cursor over a slider ({sorted(cursors)})")
        expect(not cursors & {"Grab", "Grabbing"},
               f"cursor: the arrow over a slider, pressed or not, as macOS's own ({sorted(cursors)})")
        # Left from the right end, then right from the top edge, so each
        # drag has room to move the value.
        for label, (px, py), dx in (("its right end", (x + w - 3, y + h / 2), -w * 0.4),
                                    ("its top edge", (x + w / 2, y + 2), w * 0.4)):
            before = text_of(app, "fov_value")
            press_drag(app, px, py, dx)
            time.sleep(0.4)
            expect(text_of(app, "fov_value") != before,
                   f"cursor: a press at {label} drags the slider ({before} -> {text_of(app, 'fov_value')})")
        expect(app.errors() == [], f"cursor: no errors in the app log {app.errors()[:3]}")


CHECKS = {"view": check_view, "lens": check_lens, "preview": check_preview, "picker": check_picker,
          "picker_scroll": check_picker_scroll, "stats": check_stats, "cursor": check_cursor}


def main():
    os.makedirs(OUT, exist_ok=True)
    names = sys.argv[1:] or list(CHECKS)
    for name in names:
        CHECKS[name]()
    if FAILURES:
        print(f"\n{len(FAILURES)} check(s) failed")
        sys.exit(1)
    print(f"Module 5 check passed. Screenshots: {OUT}")


if __name__ == "__main__":
    main()
