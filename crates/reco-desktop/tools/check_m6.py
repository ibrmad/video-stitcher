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


def launch(files, extra=(), config_dir=None, answers=None):
    args = ["--window-size", "1280x820", "--left", files[0], "--right", files[1],
            "--calibration", files[2], *extra]
    env = {}
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
        before = text_of(app, "time_current")
        click(app, "export_browse")
        expect(wait_for(lambda: text_of(app, "export_output") == answered + ".mp4", 5),
               f"export: Save to… sets the file, with .mp4 added ({text_of(app, 'export_output')})")
        click(app, "sheet_export")
        expect(bool(wait_for(lambda: app.rect("export_card"), 5)), "export: the card shows over the picture")
        expect(app.enabled("play_pause") is False, "export: playback is locked while exporting")
        progress = wait_for(lambda: (text_of(app, "export_detail") or "").startswith("Frame"), 30)
        if progress:
            save_shot(app, "exporting")
        done = wait_for(lambda: logged(app, "export: done"), 90)
        expect(bool(done), "export: it finishes")
        expect(bool(wait_for(lambda: title_rect(app, "Export complete"), 5)), "export: a notice says so")
        expect(not sheet_shows(app, face), "export: the sheet closed")
        expect(app.rect("export_card") is None, "export: the card goes")
        expect(wait_for(lambda: app.enabled("play_pause"), 5) is True, "export: playback comes back")
        expect(text_of(app, "time_current") == before,
               f"export: the preview stays where it was ({before} → {text_of(app, 'time_current')})")
        expect(app.rect("show_in_folder") is not None, "export: Show in folder offers the file")
        width, height, frames = probe(answered + ".mp4")
        expect((width, height) == (1920, 1080), f"export: 1080p by default ({width}x{height})")
        expect(55 <= frames <= 65, f"export: about two seconds at 30 fps ({frames} frames)")
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
        type_into(app, "range_end_text", "0:10")
        app.key("return")
        type_into(app, "range_start_text", "0:20")
        app.key("return")
        expect(text_of(app, "range_start_text") == "0:10",
               f"rules: the start stops at the end ({text_of(app, 'range_start_text')})")
        expect(app.rect("range_empty") is not None, "rules: an empty range says so")
        expect(app.enabled("sheet_export") is False, "rules: an empty range can't be exported")
        type_into(app, "range_start_text", "soon")
        app.key("return")
        expect(text_of(app, "range_start_text") == "0:10", "rules: a time that doesn't read is put back")
        save_shot(app, "rules")
        app.key("escape")
        expect(wait_for(lambda: not sheet_shows(app, face), 5), "rules: Escape closes the sheet")
        expect(not app.errors(), f"rules: no errors in the app log {app.errors()[:3]}")
    expect(not any(n.endswith("_stitched.mp4") for n in os.listdir(folder)), "rules: nothing was exported")


CHECKS = {"export": check_export, "cancel": check_cancel, "rules": check_rules}


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
