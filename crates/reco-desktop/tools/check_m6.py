#!/usr/bin/env python3
"""Module 6 check: export (PARITY.md, Module 6).

Run after `cargo build --profile desktop -p reco-desktop`. Opens the fast
fixture pair, linked into a temporary folder with a copy of its calibration
(so the default export file lands there, never beside the fixtures), and
exports short ranges through the export sheet. The Save dialog is answered
through RECO_DESKTOP_DIALOG_ANSWERS. Each launch gets its own settings
folder. Screenshots go to target/desktop-checks/m6/. `check_m6.py NAME...`
runs the named checks (all by default); exits non-zero if any failed.
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
OUT = os.path.join(ROOT, "target", "desktop-checks", "m6")
HOME = os.path.expanduser("~")
FAST = (
    os.environ.get("RECO_FIXTURE_LEFT", f"{HOME}/dev/pitchcam-data/alfheim/cam0.mp4"),
    os.environ.get("RECO_FIXTURE_RIGHT", f"{HOME}/dev/pitchcam-data/alfheim/cam1.mp4"),
    os.environ.get("RECO_FIXTURE_CAL", f"{HOME}/dev/pitchcam-data/alfheim/reco/match.json"),
)

MODEL = os.environ.get("RECO_FIXTURE_MODEL", f"{HOME}/dev/pitchcam-data/alfheim/reco/yolo26n.onnx")

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
    """The rect of a toast title showing `text`, or None."""
    for item in app.snap(text):
        if item.get("t") == text and item.get("i") == "title":
            return item["r"]
    return None


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


def linked(name):
    """A fresh folder with the pair linked in and a copy of the calibration."""
    folder = tempfile.mkdtemp(prefix=f"reco-m6-{name}-")
    names = []
    for source, link in zip(FAST[:2], ("cam0.mp4", "cam1.mp4")):
        os.symlink(source, os.path.join(folder, link))
        names.append(os.path.join(folder, link))
    cal = os.path.join(folder, "match.json")
    shutil.copyfile(FAST[2], cal)
    return folder, (names[0], names[1], cal)


def launch(files, extra=(), config_dir=None, answers=None, size="1280x820", env=None):
    args = ["--window-size", size, "--left", files[0], "--right", files[1],
            "--calibration", files[2], *extra]
    env = dict(env or {})
    if config_dir:
        env["RECO_CONFIG_DIR"] = config_dir
    if answers is not None:
        fd, path = tempfile.mkstemp(prefix="reco-m6-answers-", suffix=".json")
        with os.fdopen(fd, "w") as f:
            json.dump(answers, f)
        env["RECO_DESKTOP_DIALOG_ANSWERS"] = path
    return drive.App.launch(BIN, args, env=env or None)


def pixel(app, x, y):
    """The window's colour at (x, y) points, as (r, g, b)."""
    png = app.grab(os.path.join(OUT, "probe.png"), scale=0.5)
    scale = png.width / app.get("/s")["w"][0]["sz"][0]
    return png.pixel(int(x * scale), int(y * scale))[:3]


def brightest(app, box):
    """The brightest channel value inside box (x0, y0, x1, y1) in points."""
    png = app.grab(os.path.join(OUT, "probe.png"))
    scale = png.width / app.get("/s")["w"][0]["sz"][0]
    x0, y0, x1, y1 = (int(v * scale) for v in box)
    return max(max(png.pixel(x, y)[:3]) for y in range(y0, y1) for x in range(x0, x1))


def mark_gap(app, widget_id):
    """Points between a checkbox's box and its text (0 when it isn't
    drawn)."""
    r = app.rect(widget_id)
    if r is None:
        return 0
    png = app.grab(os.path.join(OUT, "probe.png"))
    scale = png.width / app.get("/s")["w"][0]["sz"][0]
    return drive.gap_after_mark(png, r, scale)


def size_face(app):
    """The colour inside the Size field, right of its text."""
    r = app.rect("export_size")
    return pixel(app, r[0] + r[2] - 40, r[1] + r[3] / 2) if r else None


def sheet_shows(app, face):
    """Whether the sheet is on screen. A closed sheet's widgets stay in the
    snapshot where they were drawn, so look at the screen: the Size field's
    face, or the picture under it."""
    now = size_face(app)
    return now is not None and all(abs(a - b) <= 6 for a, b in zip(now, face))


def open_advanced(app, fold_id):
    """Open an Advanced tier by its chevron."""
    r = app.rect(fold_id)
    if r:
        app.get("/click", x=r[0] + 18, y=r[1] + 12, wait=1)
        time.sleep(0.5)


def open_sheet(app):
    """Wait for the preview and click Export; the Size field's face colour
    once the sheet shows (for `sheet_shows`), or None."""
    if not wait_for(lambda: app.rect("preview"), 30):
        return None
    wait_for(lambda: text_of(app, "time_total") == "1:00", 15)
    wait_for(lambda: app.enabled("export_button"), 10)
    click(app, "export_button")
    if not wait_for(lambda: app.rect("sheet_export"), 5):
        return None
    time.sleep(0.5)
    return size_face(app)


def pick_row(app, dropdown_id, index):
    """Choose row `index` of a dropdown with the keyboard (menu rows are
    not in the snapshot; the menu opens on the current row)."""
    click(app, dropdown_id)
    time.sleep(0.3)
    for _ in range(8):
        app.key("up")
    for _ in range(index):
        app.key("down")
    app.key("return")
    time.sleep(0.5)


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


def reveal(app, widget_id):
    """Scroll the sheet until the widget is wholly in its visible part; its
    rect then, or None. Rows out of sight aren't drawn (nor in the
    snapshot), so look down first, then up."""
    view = app.rect("export_rows")
    if view is None:
        return None
    x, y = view[0] + view[2] / 2, view[1] + view[3] / 2
    top, bottom = view[1], view[1] + view[3]
    for direction in (40, -40):
        for _ in range(30):
            r = app.rect(widget_id)
            if r is not None and r[1] >= top and r[1] + r[3] <= bottom:
                return r
            if r is not None:
                direction = 40 if r[1] + r[3] > bottom else -40
            app.scroll(x, y, direction)
    return None


def show_and_click(app, widget_id):
    """Scroll a sheet control into view and click it; whether it was there."""
    if reveal(app, widget_id) is None:
        return False
    return click(app, widget_id)


def probe(path):
    """(width, height, frames) of a video, by ffprobe."""
    out = subprocess.run(
        ["ffprobe", "-v", "error", "-select_streams", "v:0", "-count_frames",
         "-show_entries", "stream=width,height,nb_read_frames", "-of", "json", path],
        capture_output=True, text=True).stdout
    stream = json.loads(out or "{}").get("streams", [{}])[0]
    return stream.get("width"), stream.get("height"), int(stream.get("nb_read_frames", 0))


def check_export():
    """The sheet opens filled in; Save to… picks the file; Export writes two
    seconds at 1080p while the card shows progress; the preview stays where
    it was; the choices are remembered."""
    folder, files = linked("export")
    out_dir = tempfile.mkdtemp(prefix="reco-m6-out-")
    answered = os.path.join(out_dir, "final")
    config = tempfile.mkdtemp(prefix="reco-m6-config-")
    with launch(files, ["--export-range", "0-2"], config, {"export": [answered]}) as app:
        face = open_sheet(app)
        expect(face is not None, "export: Export opens the sheet")
        default = os.path.join(folder, "cam0_stitched.mp4")
        expect(text_of(app, "export_output") == default,
               f"export: the file defaults to beside the left video ({text_of(app, 'export_output')})")
        expect(text_of(app, "range_length") == "0:02 of 1:00",
               f"export: the range comes from --export-range ({text_of(app, 'range_length')})")
        expect(text_of(app, "range_start_text") == "0:00" and text_of(app, "range_end_text") == "0:02",
               "export: the time fields show the range")
        expect(app.enabled("sheet_export") is True, "export: Export is enabled")
        # The sliders show no value of their own (the time fields do): no
        # ink above the start slider's track at its right end.
        x, y, w, h = app.rect("range_start")
        ink = brightest(app, (x + w - 30, y + 1, x + w - 6, y + h / 2 - 3))
        expect(ink < 40, f"export: the range sliders show no readout (brightest {ink})")
        wait_for(lambda: logged(app, "export codecs:"), 10)
        save_shot(app, "sheet")
        for box in ("export_replay", "export_events"):
            gap = mark_gap(app, box)
            expect(gap >= 6, f"export: {box}'s text clears its box ({gap} pt)")
        before = text_of(app, "time_current")
        click(app, "export_browse")
        expect(wait_for(lambda: text_of(app, "export_output") == answered + ".mp4", 5),
               f"export: Save to… sets the file, with .mp4 added ({text_of(app, 'export_output')})")
        # A time typed but not entered counts: Export takes no focus.
        type_into(app, "range_end_text", "0:03")
        click(app, "sheet_export")
        expect(bool(wait_for(lambda: app.rect("export_card"), 5)), "export: the card shows over the picture")
        expect(app.enabled("play_pause") is False and app.enabled("timeline") is False,
               "export: playback and the ruler are locked while exporting")
        progress = wait_for(lambda: (text_of(app, "export_detail") or "").startswith("Frame"), 30)
        if progress:
            save_shot(app, "exporting")
        done = wait_for(lambda: logged(app, "export: done"), 90)
        expect(bool(done), "export: it finishes")
        expect(bool(wait_for(lambda: title_rect(app, "Export complete"), 5)), "export: a notice says so")
        expect(not sheet_shows(app, face), "export: the sheet closed")
        expect(app.rect("export_card") is None, "export: the card goes")
        expect(wait_for(lambda: app.enabled("play_pause") and app.enabled("timeline"), 5) is True,
               "export: playback and the ruler come back")
        expect(text_of(app, "time_current") == before,
               f"export: the preview stays where it was ({before} → {text_of(app, 'time_current')})")
        expect(app.rect("show_in_folder") is not None, "export: Show in folder offers the file")
        width, height, frames = probe(answered + ".mp4")
        expect((width, height) == (1920, 1080), f"export: 1080p by default ({width}x{height})")
        expect(85 <= frames <= 95, f"export: the typed end counts, about three seconds at 30 fps ({frames} frames)")
        expect(not os.path.exists(default), "export: nothing written at the default file")
        expect(not app.errors(), f"export: no errors in the app log {app.errors()[:3]}")
        save_shot(app, "exported")
    saved = json.load(open(os.path.join(config, "desktop.json")))
    expect((saved.get("export_size"), saved.get("export_codec"), saved.get("export_quality"))
           == ("1080p", "h264", "balanced"), f"export: the choices are remembered ({saved})")


def check_cancel():
    """Cancel on the card stops the export; the notice says what was
    written, and playback comes back."""
    folder, files = linked("cancel")
    config = tempfile.mkdtemp(prefix="reco-m6-config-")
    # 4K over the whole match: long enough to cancel.
    with open(os.path.join(config, "desktop.json"), "w") as f:
        json.dump({"export_size": "4K"}, f)
    with launch(files, config_dir=config) as app:
        expect(open_sheet(app) is not None, "cancel: Export opens the sheet")
        expect(text_of(app, "range_length") == "1:00 of 1:00", "cancel: the whole match by default")
        click(app, "sheet_export")
        started = wait_for(lambda: (text_of(app, "export_detail") or "").startswith("Frame"), 30)
        expect(bool(started), f"cancel: the export runs ({text_of(app, 'export_detail')})")
        click(app, "export_cancel")
        expect(bool(wait_for(lambda: logged(app, "export: cancelled"), 30)), "cancel: it stops")
        notice = wait_for(lambda: title_rect(app, "Export cancelled"), 5)
        expect(bool(notice), "cancel: a notice says so")
        expect(app.rect("export_card") is None, "cancel: the card goes")
        expect(wait_for(lambda: app.enabled("play_pause"), 5) is True, "cancel: playback comes back")
        expect(not logged(app, "export: done"), "cancel: it never reports done")
        expect(not app.errors(), f"cancel: no errors in the app log {app.errors()[:3]}")
        save_shot(app, "cancelled")
    partial = os.path.join(folder, "cam0_stitched.mp4")
    if os.path.exists(partial):
        os.remove(partial)


def check_rules():
    """Export's rules: an input video is refused with the reason, no file
    disables Export, an empty range says so and disables it, and Escape
    closes the sheet."""
    folder, files = linked("rules")
    with launch(files) as app:
        face = open_sheet(app)
        expect(face is not None, "rules: Export opens the sheet")
        type_into(app, "export_output", files[0])
        click(app, "sheet_export")
        error = wait_for(lambda: text_of(app, "export_error_text"), 5)
        expect(bool(error) and "one of the videos" in error, f"rules: an input video is refused ({error})")
        expect(sheet_shows(app, face), "rules: the sheet stays open to fix it")
        type_into(app, "export_output", "")
        expect(wait_for(lambda: app.enabled("sheet_export") is False, 5), "rules: no file, no Export")
        expect(app.rect("export_error") is None, "rules: editing the file clears the reason")
        type_into(app, "export_output", "match")
        type_into(app, "range_start_text", "0:20")
        type_into(app, "range_end_text", "0:40")
        app.key("return")
        expect(text_of(app, "range_length") == "0:20 of 1:00",
               f"rules: both times apply together ({text_of(app, 'range_length')})")
        # A new sync offset shortens the match; the range keeps its place.
        app.key("escape")
        wait_for(lambda: not sheet_shows(app, face), 5)
        open_advanced(app, "stitch_advanced")
        type_into(app, "sync_input", "30")
        click(app, "sync_apply")
        expect(bool(wait_for(lambda: text_of(app, "time_total") == "0:59", 20)),
               f"rules: a sync offset shortens the match ({text_of(app, 'time_total')})")
        click(app, "export_button")
        expect(bool(wait_for(lambda: sheet_shows(app, face), 5)), "rules: the sheet opens again")
        expect(text_of(app, "range_length") == "0:20 of 0:59",
               f"rules: the range keeps its place ({text_of(app, 'range_length')})")
        type_into(app, "range_start_text", "0:50")
        app.key("return")
        expect(text_of(app, "range_start_text") == "0:40",
               f"rules: the start stops at the end ({text_of(app, 'range_start_text')})")
        expect(app.rect("range_empty") is not None, "rules: an empty range says so")
        expect(app.enabled("sheet_export") is False, "rules: an empty range can't be exported")
        type_into(app, "range_start_text", "soon")
        app.key("return")
        expect(text_of(app, "range_start_text") == "0:40", "rules: a time that doesn't read is put back")
        save_shot(app, "rules")
        app.key("escape")
        expect(wait_for(lambda: not sheet_shows(app, face), 5), "rules: Escape closes the sheet")
        expect(not app.errors(), f"rules: no errors in the app log {app.errors()[:3]}")
    expect(not any(n.endswith("_stitched.mp4") for n in os.listdir(folder)), "rules: nothing was exported")


def check_ai():
    """AI tracking in the sheet: Enable waits for the machine's answer, which
    the status line gives; off, the rows are hidden; tracking without a
    model is refused with the reason, Sweep needs none, and Choose… fixes
    it; a style preset sets the knobs; the lookahead shows the GPU's zones;
    a two-second export tracks, says so on the card and in the notice, and
    writes the detections; the choices are remembered, the model as the
    default."""
    if not os.path.exists(MODEL):
        expect(False, f"ai: the fixture model is missing ({MODEL})")
        return
    folder, files = linked("ai")
    config = tempfile.mkdtemp(prefix="reco-m6-config-")
    out_dir = tempfile.mkdtemp(prefix="reco-m6-out-")
    answered = os.path.join(out_dir, "tracked")
    answers = {"model": [MODEL], "export": [answered]}
    with launch(files, ["--export-range", "0-2"], config, answers) as app:
        face = open_sheet(app)
        expect(face is not None, "ai: Export opens the sheet")
        ready = wait_for(lambda: (text_of(app, "ai_status") or "").startswith("Ready: runs on "), 30)
        expect(bool(ready), f"ai: the status says where tracking runs ({text_of(app, 'ai_status')})")
        expect(app.enabled("ai_enable") is True, "ai: Enable takes input once the machine answers")
        expect(app.rect("ai_model") is None, "ai: off, the tracking rows are hidden")
        gap = mark_gap(app, "ai_enable")
        expect(gap >= 6, f"ai: Enable's text clears its box ({gap} pt)")
        save_shot(app, "ai-off")
        show_and_click(app, "ai_enable")
        expect(bool(wait_for(lambda: app.rect("ai_model"), 3)), "ai: on, the rows show")
        problem = text_of(app, "ai_model_problem_text")
        expect(reveal(app, "ai_model_problem") is not None
               and problem == "Choose the AI model (an .onnx file) to track.",
               f"ai: no model says so ({problem})")
        expect(app.enabled("sheet_export") is False, "ai: no model, no Export")
        reveal(app, "ai_mode")
        pick_row(app, "ai_mode", 2)
        expect(text_of(app, "ai_mode") == "Sweep (no AI)", f"ai: Sweep is chosen ({text_of(app, 'ai_mode')})")
        expect(app.rect("ai_model_problem") is None and app.enabled("sheet_export") is True,
               "ai: Sweep needs no model")
        pick_row(app, "ai_mode", 0)
        expect(app.enabled("sheet_export") is False, "ai: following the players needs the model again")
        show_and_click(app, "ai_model_browse")
        expect(bool(wait_for(lambda: text_of(app, "ai_model") == MODEL, 5)),
               f"ai: Choose… sets the model ({text_of(app, 'ai_model')})")
        expect(app.rect("ai_model_problem") is None and app.enabled("sheet_export") is True,
               "ai: with the model, Export is enabled")
        # A style preset sets every knob it covers, the Advanced tier's too.
        expect(text_of(app, "ai_preset") == "Broadcast" and text_of(app, "ai_interval") == "Every 15 frames",
               f"ai: Broadcast, every 15 frames to start ({text_of(app, 'ai_preset')}, {text_of(app, 'ai_interval')})")
        reveal(app, "ai_advanced")
        open_advanced(app, "ai_advanced")
        expect(bool(wait_for(lambda: app.rect("ai_dead_zone"), 3)), "ai: Advanced opens")
        reveal(app, "ai_dead_zone_value")
        expect(text_of(app, "ai_dead_zone_value") == "0.03" and text_of(app, "ai_fov_tight_value") == "22°",
               f"ai: Broadcast's knobs ({text_of(app, 'ai_dead_zone_value')}, {text_of(app, 'ai_fov_tight_value')})")
        reveal(app, "ai_preset")
        pick_row(app, "ai_preset", 1)
        expect(text_of(app, "ai_dead_zone_value") == "0.02",
               f"ai: Action narrows the dead zone ({text_of(app, 'ai_dead_zone_value')})")
        pick_row(app, "ai_preset", 2)
        expect(text_of(app, "ai_framing") == "Keep everyone in frame",
               f"ai: Frame all frames everyone ({text_of(app, 'ai_framing')})")
        pick_row(app, "ai_preset", 0)
        expect(text_of(app, "ai_framing") == "Follow the action" and text_of(app, "ai_dead_zone_value") == "0.03",
               "ai: Broadcast puts its knobs back")
        # The lookahead's zones come from the GPU's memory.
        track = reveal(app, "ai_lookahead_zones")
        note = text_of(app, "ai_lookahead_note")
        expect(track is not None and note == "Fits this GPU's memory.",
               f"ai: the lookahead fits this machine ({note})")
        expect(text_of(app, "ai_lookahead_value") == "2.5 s",
               f"ai: the lookahead starts at 2.5 s ({text_of(app, 'ai_lookahead_value')})")
        if track:
            x, y, w, h = track
            left = pixel(app, x + 10, y + h / 2)
            expect(left[1] > left[0] + 30 and left[1] > left[2], f"ai: the track's safe zone is green ({left})")
        drag(app, "ai_lookahead", -0.6)
        value = text_of(app, "ai_lookahead_value")
        expect(value not in (None, "2.5 s"), f"ai: dragging the lookahead shows its value ({value})")
        save_shot(app, "ai-on")
        # Events on, so the detections can be read back.
        show_and_click(app, "export_events")
        show_and_click(app, "export_browse")
        expect(bool(wait_for(lambda: text_of(app, "export_output") == answered + ".mp4", 5)),
               f"ai: Save to… names the file ({text_of(app, 'export_output')})")
        click(app, "sheet_export")
        expect(bool(wait_for(lambda: logged(app, "export: AI tracking active"), 60)),
               "ai: the export says tracking is active")
        line = wait_for(lambda: text_of(app, "export_tracking"), 5)
        expect(line == "AI tracking: active", f"ai: so does the card ({line})")
        done = wait_for(lambda: logged(app, "export: done"), 120)
        expect(bool(done), "ai: the export finishes")
        notice = wait_for(lambda: [i.get("t") for i in app.snap("tracked with AI") if i.get("i") == "body"], 5)
        expect(bool(notice), f"ai: the notice says it tracked ({notice})")
        events = answered + ".events.jsonl"
        log = open(events).read() if os.path.exists(events) else ""
        expect("detect" in log, f"ai: the events file has the detections ({len(log)} bytes)")
        expect(not app.errors(), f"ai: no errors in the app log {app.errors()[:3]}")
        save_shot(app, "ai-exported")
    saved = json.load(open(os.path.join(config, "desktop.json")))
    remembered = {k: saved.get(k) for k in ("ai_enabled", "ai_mode", "ai_interval", "ai_preset",
                                             "ai_framing", "ai_lock_pitch", "ai_model_path")}
    expect(remembered == {"ai_enabled": True, "ai_mode": "field", "ai_interval": 15,
                          "ai_preset": "broadcast", "ai_framing": "action", "ai_lock_pitch": False,
                          "ai_model_path": MODEL},
           f"ai: the choices are remembered, the model as the default ({remembered})")
    expect(0.0 < saved.get("ai_lookahead", 2.5) < 2.5,
           f"ai: and the dragged lookahead ({saved.get('ai_lookahead')})")
    for leftover in (answered + ".mp4", answered + ".events.jsonl"):
        if os.path.exists(leftover):
            os.remove(leftover)


def check_ai_short():
    """In a short window, the sheet with AI tracking on and Advanced open
    stays inside the window, Export included, and scrolls to its last
    row."""
    folder, files = linked("ai-short")
    config = tempfile.mkdtemp(prefix="reco-m6-config-")
    with open(os.path.join(config, "desktop.json"), "w") as f:
        json.dump({"ai_enabled": True}, f)
    with launch(files, config_dir=config, size="720x600") as app:
        expect(open_sheet(app) is not None, "short: Export opens the sheet")
        wait_for(lambda: app.enabled("ai_enable"), 30)
        expect(bool(wait_for(lambda: app.rect("ai_model"), 3)), "short: remembered on, the rows show")

        reveal(app, "ai_advanced")
        open_advanced(app, "ai_advanced")
        window = app.get("/s")["w"][0]["sz"]
        export = app.rect("sheet_export")
        expect(export is not None and export[1] + export[3] <= window[1],
               f"short: Export stays inside the window ({export}, window {window})")
        last = reveal(app, "ai_fov_wide")
        expect(last is not None, "short: the sheet scrolls to its last row")
        expect(reveal(app, "export_output") is not None, "short: and back to the top")
        save_shot(app, "ai-short")
        expect(not app.errors(), f"short: no errors in the app log {app.errors()[:3]}")


def check_ai_unavailable():
    """A machine that can't run the detector: the status says why, Enable
    stays off and its rows hidden, and the export goes on without it."""
    folder, files = linked("ai-off")
    config = tempfile.mkdtemp(prefix="reco-m6-config-")
    with open(os.path.join(config, "desktop.json"), "w") as f:
        json.dump({"ai_enabled": True}, f)
    env = {"RECO_DESKTOP_FAKE_AI": "no inference engine loads on this machine"}
    with launch(files, config_dir=config, env=env) as app:
        expect(open_sheet(app) is not None, "unavailable: Export opens the sheet")
        line = text_of(app, "ai_status_error")
        expect(line == "Not available: no inference engine loads on this machine",
               f"unavailable: the status says why ({line})")
        expect(app.enabled("ai_enable") is False, "unavailable: Enable is dimmed")
        show_and_click(app, "ai_enable")
        time.sleep(0.5)
        checked = [i.get("c") for i in app.snap("ai_enable") if i.get("i") == "ai_enable"]
        expect(checked == [0], f"unavailable: a click leaves it off ({checked})")
        expect(app.rect("ai_model") is None, "unavailable: the rows stay hidden")
        expect(app.enabled("sheet_export") is True, "unavailable: Export goes on without it")
        save_shot(app, "ai-unavailable")
        expect(not app.errors(), f"unavailable: no errors in the app log {app.errors()[:3]}")


def check_ai_figures():
    """An export tracked without a lookahead puts the detector's figures in
    Stats (with one, the engine measures none: FRICTION.md)."""
    if not os.path.exists(MODEL):
        expect(False, f"figures: the fixture model is missing ({MODEL})")
        return
    folder, files = linked("ai-figures")
    config = tempfile.mkdtemp(prefix="reco-m6-config-")
    with open(os.path.join(config, "desktop.json"), "w") as f:
        json.dump({"ai_enabled": True, "ai_model_path": MODEL, "ai_lookahead": 0.0}, f)
    with launch(files, ["--export-range", "0-2"], config) as app:
        expect(open_sheet(app) is not None, "figures: Export opens the sheet")
        wait_for(lambda: app.enabled("ai_enable"), 30)
        expect(reveal(app, "ai_lookahead_value") is not None and text_of(app, "ai_lookahead_value") == "Off",
               f"figures: the lookahead is off ({text_of(app, 'ai_lookahead_value')})")
        expect(app.rect("stats_ai") is None, "figures: Stats has none before an export")
        click(app, "sheet_export")
        expect(bool(wait_for(lambda: logged(app, "export: done"), 120)), "figures: the export finishes")
        open_advanced(app, "stats_section")
        r = app.rect("stats_section")
        if r:
            app.scroll(r[0] + r[2] / 2, r[1] + 10, 200)
        detection = wait_for(lambda: (text_of(app, "stats_detection") or "").endswith(" a frame")
                             and text_of(app, "stats_detection"), 5)
        expect(bool(detection), f"figures: Stats has the detector's time and finds ({text_of(app, 'stats_detection')})")
        tracking = text_of(app, "stats_tracking") or ""
        expect(" tracked · ball " in tracking, f"figures: and the tracks and the ball ({tracking})")
        save_shot(app, "ai-figures")
        expect(not app.errors(), f"figures: no errors in the app log {app.errors()[:3]}")
    written = os.path.join(folder, "cam0_stitched.mp4")
    if os.path.exists(written):
        os.remove(written)


def check_ai_fold():
    """The closed Advanced tier looks closed when the sheet opens with AI
    tracking on. (A fold's first draw shows its body whole, Makepad
    measuring it; the owner saw the tier open until a scroll closed it.)"""
    folder, files = linked("ai-fold")
    config = tempfile.mkdtemp(prefix="reco-m6-config-")
    with open(os.path.join(config, "desktop.json"), "w") as f:
        json.dump({"ai_enabled": True}, f)
    with launch(files, config_dir=config) as app:
        ready = wait_for(lambda: app.rect("preview"), 30)
        # The machine's answer first, as when the app has been open a while.
        wait_for(lambda: logged(app, "AI tracking: "), 30)
        expect(open_sheet(app) is not None and bool(ready), "fold: Export opens the sheet")
        expect(bool(wait_for(lambda: app.rect("ai_advanced"), 5)), "fold: the Advanced tier is in view")
        time.sleep(1.0)
        expect(app.rect("ai_cluster_mode") is None, "fold: and closed, with no scroll or click")
        save_shot(app, "ai-fold")


CHECKS = {"export": check_export, "cancel": check_cancel, "rules": check_rules,
          "ai": check_ai, "ai_short": check_ai_short, "ai_unavailable": check_ai_unavailable,
          "ai_figures": check_ai_figures, "ai_fold": check_ai_fold}


def main():
    os.makedirs(OUT, exist_ok=True)
    names = sys.argv[1:] or list(CHECKS)
    for name in names:
        CHECKS[name]()
    if FAILURES:
        print(f"\n{len(FAILURES)} check(s) failed")
        sys.exit(1)
    print(f"Module 6 check passed. Screenshots: {OUT}")


if __name__ == "__main__":
    main()
