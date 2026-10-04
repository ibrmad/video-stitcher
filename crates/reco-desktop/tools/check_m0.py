#!/usr/bin/env python3
"""Module 0 check: shell and look (PARITY.md, Module 0).

Run after `cargo build --profile desktop -p reco-desktop`. Saves screenshots
to target/desktop-checks/m0/ and exits non-zero at the first failure.
"""
import os
import sys
import time

import drive

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "..", ".."))
BIN = os.path.join(ROOT, "target", "desktop", "reco-desktop")
OUT = os.path.join(ROOT, "target", "desktop-checks", "m0")

APP_BG = "#0f1115"
PANEL_BG = "#14171c"
ACCENT = "#34d399"

# Controls that need loaded files (Module 0 stand-in: both cameras loaded).
GATED = ("export_button", "toggle_inspector", "step_back", "play_pause", "step_forward",
         "record_button", "auto_calibrate", "timeline", "aspect")


def expect(ok, message):
    if not ok:
        print(f"FAIL: {message}")
        sys.exit(1)
    print(f"ok: {message}")


def visible(app, widget_id):
    return app.rect(widget_id) is not None


def settled_pixel(app, dest, x_pt, y_pt, width_pt):
    """Grab until the pixel at (x_pt, y_pt) stops changing, so enable and
    disable animations have finished. Returns (png, rgba)."""
    last = None
    for _ in range(8):
        png = app.grab(dest)
        scale = png.width / width_pt
        value = png.pixel(int(x_pt * scale), int(y_pt * scale))
        if value == last:
            break
        last = value
        time.sleep(0.25)
    return png, value


def launch(size, look_preview):
    args = ["--window-size", f"{size[0]}x{size[1]}"]
    if look_preview:
        args.append("--look-preview")
    return drive.App.launch(BIN, args)


def check_state(size, look_preview):
    name = f"{'loaded' if look_preview else 'empty'}-{size[0]}x{size[1]}"
    with launch(size, look_preview) as app:
        info = app.get("/s")["w"][0]
        expect(info["t"].startswith("Reco"), f"{name}: window title is Reco ({info['t']!r})")
        expect(abs(info["sz"][0] - size[0]) <= 2 and abs(info["sz"][1] - size[1]) <= 2,
               f"{name}: window size {info['sz']} matches {size}")
        for wid in ("toggle_media", "export_button", "toggle_inspector", "status_text",
                    "version_text", "report_bug", "play_pause", "timeline"):
            expect(visible(app, wid), f"{name}: `{wid}` is on screen")
        expect(visible(app, "media_panel"), f"{name}: Media panel open")
        if look_preview:
            expect(visible(app, "sample_frame"), f"{name}: sample frame shown")
            expect(not visible(app, "empty_state"), f"{name}: empty state hidden")
            inspector_expected = size[0] >= 960
            expect(visible(app, "inspector") == inspector_expected,
                   f"{name}: Inspector {'open' if inspector_expected else 'folded'}")
        else:
            expect(visible(app, "empty_state"), f"{name}: empty state shown")
            expect(not visible(app, "inspector"), f"{name}: Inspector closed before load")
        ex, ey, ew, eh = app.rect("export_button")
        png, centre_left = settled_pixel(app, os.path.join(OUT, f"{name}.png"),
                                         ex + 6, ey + eh / 2, info["sz"][0])
        corner = png.pixel(png.width - 3, png.height - 3)
        expect(drive.close_to(corner, APP_BG, tol=4), f"{name}: status bar background {corner[:3]} is {APP_BG}")
        expect(drive.close_to(centre_left, ACCENT, tol=40) == look_preview,
               f"{name}: Export is {'enabled (accent)' if look_preview else 'disabled'} ({centre_left[:3]})")
        # Every control gated on loaded files takes input exactly when loaded
        # (Makepad's own enabled flag), not just looks that way.
        for wid in GATED:
            expect(app.enabled(wid) == look_preview,
                   f"{name}: `{wid}` {'enabled' if look_preview else 'disabled'} (/snap enabled={app.enabled(wid)})")
        # ...and the Inspector toggle's icon is visibly dimmed before load.
        tx, ty, tw, th = app.rect("toggle_inspector")
        _, icon = settled_pixel(app, os.path.join(OUT, f"{name}-toggle.png"),
                                tx + tw / 2 + 3.5, ty + th / 2, info["sz"][0])
        expect((max(icon[:3]) > 130) == look_preview,
               f"{name}: Inspector toggle icon {'bright' if look_preview else 'dimmed'} ({icon[:3]})")
        expect(app.errors() == [], f"{name}: no errors in the app log")


def drag(app, x, y, to_x):
    """Drag horizontally from (x, y) to (to_x, y) with the left button."""
    app.get("/m", k="down", x=x, y=y)
    app.get("/m", k="move", x=(x + to_x) / 2, y=y)
    app.get("/m", k="move", x=to_x, y=y)
    app.get("/m", k="up", x=to_x, y=y, wait=1)


def check_toggles():
    with launch((1280, 820), True) as app:
        app.click_id("toggle_media")
        expect(not visible(app, "media_panel"), "toggle: Media folds")
        app.grab(os.path.join(OUT, "loaded-media-folded.png"))
        app.click_id("toggle_media")
        expect(visible(app, "media_panel"), "toggle: Media reopens")
        app.click_id("toggle_inspector")
        expect(not visible(app, "inspector"), "toggle: Inspector folds")
        app.click_id("toggle_inspector")
        expect(visible(app, "inspector"), "toggle: Inspector reopens")
        mx, my, mw, mh = app.rect("media_panel")
        bar_x, bar_y = mx + mw + 3, my + mh / 2
        app.get("/m", k="down", x=bar_x, y=bar_y)
        app.get("/m", k="move", x=bar_x + 60, y=bar_y)
        app.get("/m", k="up", x=bar_x + 60, y=bar_y, wait=1)
        expect(app.rect("media_panel")[2] > mw + 40, "resize: dragging the bar widens Media")
        app.grab(os.path.join(OUT, "loaded-media-widened.png"))
        # Dragging far past a panel's minimum width stops at the minimum.
        mx, my, mw, mh = app.rect("media_panel")
        drag(app, mx + mw + 3, bar_y, 10)
        media = app.rect("media_panel")
        expect(media is not None and media[2] >= 175,
               f"resize: Media stops at its 180 pt minimum ({media and media[2]})")
        ix, iy, iw, ih = app.rect("inspector")
        drag(app, ix - 3, bar_y, 1270)
        inspector = app.rect("inspector")
        expect(inspector is not None and inspector[2] >= 195,
               f"resize: Inspector stops at its 200 pt minimum ({inspector and inspector[2]})")
        expect(app.errors() == [], "toggle: no errors in the app log")
    with launch((1280, 820), False) as app:
        app.click_id("toggle_inspector")
        expect(not visible(app, "inspector"), "toggle: Inspector stays closed before load")


def main():
    os.makedirs(OUT, exist_ok=True)
    for size in ((720, 600), (1280, 820), (1920, 1200)):
        check_state(size, look_preview=False)
        check_state(size, look_preview=True)
    check_toggles()
    print(f"Module 0 check passed. Screenshots: {OUT}")


if __name__ == "__main__":
    main()
