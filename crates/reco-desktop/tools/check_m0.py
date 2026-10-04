#!/usr/bin/env python3
"""Module 0 check: shell and look (PARITY.md, Module 0).

Run after `cargo build --profile desktop -p reco-desktop`. Walks every
--look-preview state, saves screenshots to target/desktop-checks/m0/ and
exits non-zero if any check failed. Every failure is listed, not just the
first.
"""
import os
import sys
import time

import drive

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "..", ".."))
BIN = os.path.join(ROOT, "target", "desktop", "reco-desktop")
OUT = os.path.join(ROOT, "target", "desktop-checks", "m0")

APP_BG = "#0f1115"
PANEL = "#14171c"
SURFACE = "#1a1d23"
VIEWER = "#08090b"
ACCENT = "#34d399"
# A card's content edge sits this far inside the card (theme.reco_space_xl).
CARD_PAD = 12
# A status line's words start after its dot: theme.reco_dot + reco_space_s.
DOT_INDENT = 8 + 6

# Every --look-preview state in job order; None is a fresh start.
STATES = (None, "one-camera", "cameras", "calibrating", "ready", "exporting")
STITCHED = ("ready", "exporting")

FAILURES = []


def expect(ok, message):
    print(f"{'ok' if ok else 'FAIL'}: {message}")
    if not ok:
        FAILURES.append(message)


def name_of(state):
    return state or "start"


def visible(app, widget_id):
    return app.rect(widget_id) is not None


def shown(state):
    """Which of the state-dependent widgets each state shows."""
    left = state is not None
    right = state not in (None, "one-camera")
    stitched = state in STITCHED
    return {
        "empty_state": not stitched,
        "sample_frame": stitched,
        "next_actions": state in (None, "one-camera", "cameras"),
        "calibrate_progress": state == "calibrating",
        "export_card": state == "exporting",
        "add_left": not left,
        "change_left": left,
        "add_right": not right,
        "change_right": right,
        "link_idle": not right,
        "link_on": right,
        "auto_calibrate": not stitched,
        "recalibrate": stitched,
    }


def takes_input(state):
    """Which gated controls take input (Makepad's own enabled flag)."""
    stitched = state in STITCHED
    gates = {
        "export_button": state == "ready",
        "toggle_inspector": stitched,
        "step_back": stitched,
        "play_pause": stitched,
        "step_forward": stitched,
        "timeline": stitched,
        "aspect": stitched,
        "record_button": state == "ready",
    }
    if not stitched:
        gates["auto_calibrate"] = state == "cameras"
    return gates


def lum(rgba):
    r, g, b = rgba[:3]
    return 0.2126 * r + 0.7152 * g + 0.0722 * b


def ink_left(png, scale, box, bg, threshold=40):
    """Leftmost x (points) in box = (x0, y0, x1, y1) holding ink: a pixel whose
    luminance is more than threshold away from bg. None if the box is empty."""
    x0, y0, x1, y1 = box
    bg_lum = lum(drive.hex_rgb(bg))
    for px in range(int(x0 * scale), int(x1 * scale)):
        for py in range(int(y0 * scale), int(y1 * scale)):
            if abs(lum(png.pixel(px, py)) - bg_lum) > threshold:
                return px / scale
    return None


def pixel_at(png, scale, x, y):
    return png.pixel(int(x * scale), int(y * scale))


def settled(app, dest, width_pt, probe):
    """Grab until the pixel at probe=(x, y) stops changing, so enable and
    disable animations have finished. Returns (png, scale)."""
    last = None
    for _ in range(8):
        png = app.grab(dest)
        scale = png.width / width_pt
        value = pixel_at(png, scale, *probe)
        if value == last:
            break
        last = value
        time.sleep(0.25)
    return png, scale


def launch(size, state):
    args = ["--window-size", f"{size[0]}x{size[1]}"]
    if state:
        args.append(f"--look-preview={state}")
    return drive.App.launch(BIN, args)


def check_text_edge(app, png, scale, widget_id, edge, bg, name):
    """A label's ink starts on the content edge (a glyph's side bearing is
    under 2 pt)."""
    rect = app.rect(widget_id)
    if rect is None:
        expect(False, f"{name}: `{widget_id}` is on screen")
        return
    x, y, w, h = rect
    ink = ink_left(png, scale, (edge - 6, y, x + w + 6, y + h), bg)
    expect(ink is not None and edge - 0.5 <= ink <= edge + 2,
           f"{name}: `{widget_id}` text starts on the content edge {edge} (ink at {ink})")


def check_box_edge(app, widget_id, edge, name):
    rect = app.rect(widget_id)
    expect(rect is not None and abs(rect[0] - edge) <= 0.5,
           f"{name}: `{widget_id}` starts on the content edge {edge} ({rect and rect[0]})")


def check_state(size, state):
    name = f"{name_of(state)}-{size[0]}x{size[1]}"
    stitched = state in STITCHED
    with launch(size, state) as app:
        info = app.get("/s")["w"][0]
        width_pt = info["sz"][0]
        expect(info["t"].startswith("Reco"), f"{name}: window title is Reco ({info['t']!r})")
        expect(abs(info["sz"][0] - size[0]) <= 2 and abs(info["sz"][1] - size[1]) <= 2,
               f"{name}: window size {info['sz']} matches {size}")
        for wid in ("toggle_media", "export_button", "toggle_inspector", "status_text",
                    "version_text", "report_bug", "play_pause", "timeline"):
            expect(visible(app, wid), f"{name}: `{wid}` is on screen")
        expect(visible(app, "media_panel"), f"{name}: Setup panel open")
        for wid, want in shown(state).items():
            expect(visible(app, wid) == want, f"{name}: `{wid}` {'shown' if want else 'hidden'}")
        inspector = stitched and size[0] >= 960
        expect(visible(app, "inspector") == inspector,
               f"{name}: Adjust panel {'open' if inspector else 'closed'}")
        expect(visible(app, "fold_hint") == (stitched and not inspector),
               f"{name}: fold hint {'shown' if stitched and not inspector else 'hidden'}")
        for wid, want in takes_input(state).items():
            got = app.enabled(wid)
            expect(got == want, f"{name}: `{wid}` {'takes' if want else 'ignores'} input (enabled={got})")

        ex, ey, ew, eh = app.rect("export_button")
        png, scale = settled(app, os.path.join(OUT, f"{name}.png"), width_pt, (ex + 6, ey + eh / 2))
        corner = png.pixel(png.width - 3, png.height - 3)
        expect(drive.close_to(corner, APP_BG, tol=4), f"{name}: status bar background {corner[:3]} is {APP_BG}")
        face = pixel_at(png, scale, ex + 6, ey + eh / 2)
        export_on = state == "ready"
        expect(drive.close_to(face, ACCENT, tol=40) == export_on,
               f"{name}: Export face is {'accent' if export_on else 'not accent'} ({face[:3]})")
        if not export_on:
            # A disabled button greys its icon too: no green ink anywhere on it.
            tinted = [p[:3] for p in (pixel_at(png, scale, ex + dx, ey + dy)
                                      for dx in range(2, int(ew) - 2) for dy in range(2, int(eh) - 2))
                      if p[1] - max(p[0], p[2]) > 8]
            expect(not tinted, f"{name}: disabled Export has no green ink ({tinted[:2]})")

        tx, ty, tw, th = app.rect("toggle_inspector")
        probe = (tx + tw / 2 + 3.5, ty + th / 2)
        tpng, tscale = settled(app, os.path.join(OUT, f"{name}-toggle.png"), width_pt, probe)
        icon = pixel_at(tpng, tscale, *probe)
        expect((max(icon[:3]) > 130) == stitched,
               f"{name}: Adjust toggle icon {'bright' if stitched else 'dimmed'} ({icon[:3]})")

        # The camera pair's link lights up once both cameras have video.
        if shown(state)["link_on"]:
            lx, ly, lw, lh = app.rect("link_on")
            link = pixel_at(png, scale, lx + lw / 2, ly + lh / 2)
            expect(drive.close_to(link, ACCENT, tol=40), f"{name}: camera link is lit ({link[:3]})")

        # One primary per screen: the Setup panel's Auto-calibrate is a
        # secondary button, so the viewer's call to action stands alone.
        if state == "cameras":
            ax, ay, aw, ah = app.rect("auto_calibrate")
            face = pixel_at(png, scale, ax + 4, ay + ah / 2)
            expect(not drive.close_to(face, ACCENT, tol=40),
                   f"{name}: Setup panel's Auto-calibrate is secondary ({face[:3]})")

        # The transport is one row: every control centred on the same line,
        # the timeline's visible track included (not just its box).
        px, py, pw, ph = app.rect("play_pause")
        centre = py + ph / 2
        for wid in ("step_back", "timeline", "record_button", "aspect"):
            x, y, w, h = app.rect(wid)
            expect(abs(y + h / 2 - centre) <= 1,
                   f"{name}: `{wid}` centred on the transport row ({y + h / 2:.1f} vs {centre:.1f})")
        x, y, w, h = app.rect("timeline")
        panel = lum(drive.hex_rgb(PANEL))
        rows = [py_ / scale for py_ in range(int(y * scale), int((y + h) * scale))
                if abs(lum(pixel_at(png, scale, x + w * 0.75, py_ / scale)) - panel) > 15]
        track = (rows[0] + rows[-1]) / 2 if rows else None
        expect(track is not None and abs(track - centre) <= 1,
               f"{name}: timeline track centred on the transport row ({track} vs {centre:.1f})")

        # Every item in a Setup card starts on the card's content edge, and
        # the panel header lines up with it.
        if size[0] >= 1280:
            cx, cy, cw, ch = app.rect("calibration")
            edge = cx + CARD_PAD
            for wid in ("calibration_title", "calibration_hint"):
                check_text_edge(app, png, scale, wid, edge, SURFACE, name)
            # The status words and their detail share one edge, after the dot.
            words = edge + DOT_INDENT
            for wid in ("calibration_status", "calibration_detail"):
                check_text_edge(app, png, scale, wid, words, SURFACE, name)
            dots = [d for d in ("cal_dot_idle", "cal_dot_busy", "cal_dot_ok") if visible(app, d)]
            expect(len(dots) == 1, f"{name}: one calibration dot shown ({dots})")
            if dots:
                check_box_edge(app, dots[0], edge, name)
            check_box_edge(app, "recalibrate" if stitched else "auto_calibrate", edge, name)
            mx, my, mw, mh = app.rect("cameras")
            check_text_edge(app, png, scale, "cameras_title", mx + CARD_PAD, SURFACE, name)
            check_box_edge(app, "left_badge_on" if state else "left_badge", mx + CARD_PAD, name)
            check_text_edge(app, png, scale, "setup_header", mx + CARD_PAD, PANEL, name)

        # The next step fits the viewer: no text runs into its right edge.
        if not stitched:
            vx, vy, vw, vh = app.rect("canvas")
            sx, sy, sw, sh = app.rect("empty_state")
            ink = ink_left(png, scale, (vx + vw - 12, sy, vx + vw - 1, sy + sh), VIEWER)
            expect(ink is None, f"{name}: next step clear of the viewer's right edge (ink at {ink})")
        if state == "calibrating":
            bx, by, bw, bh = app.rect("step2_current")
            badge = pixel_at(png, scale, bx + bw / 2, by + 3)
            expect(drive.close_to(badge, ACCENT, tol=40),
                   f"{name}: stepper not covered while calibrating ({badge[:3]})")

        expect(app.errors() == [], f"{name}: no errors in the app log")


def check_adjust_edges():
    """Every item in an Adjust card starts on the card's content edge."""
    name = "adjust-1280x820"
    with launch((1280, 820), "ready") as app:
        png = app.grab(os.path.join(OUT, f"{name}.png"))
        scale = png.width / app.get("/s")["w"][0]["sz"][0]
        edge = app.rect("view_fold")[0]
        for wid in ("fov_hint", "seam_hint"):
            check_text_edge(app, png, scale, wid, edge, SURFACE, name)
        for wid in ("fov_slider", "seam_blend", "reset_view"):
            check_box_edge(app, wid, edge, name)
        check_text_edge(app, png, scale, "adjust_header", edge, PANEL, name)


def drag(app, x, y, to_x):
    """Drag horizontally from (x, y) to (to_x, y) with the left button."""
    app.get("/m", k="down", x=x, y=y)
    app.get("/m", k="move", x=(x + to_x) / 2, y=y)
    app.get("/m", k="move", x=to_x, y=y)
    app.get("/m", k="up", x=to_x, y=y, wait=1)


def check_toggles():
    with launch((1280, 820), "ready") as app:
        app.click_id("toggle_media")
        expect(not visible(app, "media_panel"), "toggle: Setup panel folds")
        app.grab(os.path.join(OUT, "ready-setup-folded.png"))
        app.click_id("toggle_media")
        expect(visible(app, "media_panel"), "toggle: Setup panel reopens")
        app.click_id("toggle_inspector")
        expect(not visible(app, "inspector"), "toggle: Adjust panel folds")
        app.click_id("toggle_inspector")
        expect(visible(app, "inspector"), "toggle: Adjust panel reopens")
        mx, my, mw, mh = app.rect("media_panel")
        bar_x, bar_y = mx + mw + 3, my + mh / 2
        app.get("/m", k="down", x=bar_x, y=bar_y)
        app.get("/m", k="move", x=bar_x + 60, y=bar_y)
        app.get("/m", k="up", x=bar_x + 60, y=bar_y, wait=1)
        expect(app.rect("media_panel")[2] > mw + 40, "resize: dragging the bar widens Setup")
        app.grab(os.path.join(OUT, "ready-setup-widened.png"))
        # Dragging far past a panel's minimum width stops at the minimum.
        mx, my, mw, mh = app.rect("media_panel")
        drag(app, mx + mw + 3, bar_y, 10)
        media = app.rect("media_panel")
        expect(media is not None and media[2] >= 175,
               f"resize: Setup stops at its 180 pt minimum ({media and media[2]})")
        ix, iy, iw, ih = app.rect("inspector")
        drag(app, ix - 3, bar_y, 1270)
        inspector = app.rect("inspector")
        expect(inspector is not None and inspector[2] >= 195,
               f"resize: Adjust stops at its 200 pt minimum ({inspector and inspector[2]})")
        expect(app.errors() == [], "toggle: no errors in the app log")
    with launch((1280, 820), None) as app:
        app.click_id("toggle_inspector")
        expect(not visible(app, "inspector"), "toggle: Adjust panel stays closed before a stitch")


def main():
    os.makedirs(OUT, exist_ok=True)
    for state in STATES:
        check_state((1280, 820), state)
    for size in ((720, 600), (1920, 1200)):
        for state in (None, "calibrating", "ready"):
            check_state(size, state)
    check_adjust_edges()
    check_toggles()
    if FAILURES:
        print(f"\nModule 0 check FAILED: {len(FAILURES)} failure(s):")
        for message in FAILURES:
            print(f"  - {message}")
        sys.exit(1)
    print(f"Module 0 check passed. Screenshots: {OUT}")


if __name__ == "__main__":
    main()
