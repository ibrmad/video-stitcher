#!/usr/bin/env python3
"""Module 0 check: shell and look (PARITY.md, Module 0).

Run after `cargo build --profile desktop -p reco-desktop`. Walks every
--look-preview state, saves screenshots to target/desktop-checks/m0/ and
exits non-zero if any check failed. Every failure is listed, not just the
first.

The look follows the Rerun viewer, with more room: neutral grey panels,
28 pt rows that start on a 14 pt content edge, section bands, a black
viewport and a time panel with camera lanes.
"""
import os
import sys
import time

import drive

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "..", ".."))
BIN = os.path.join(ROOT, "target", "desktop", "reco-desktop")
OUT = os.path.join(ROOT, "target", "desktop-checks", "m0")

PANEL = "#0d0d0d"
BAND = "#212121"
VIEWPORT = "#000000"
ACCENT = "#34d399"
ACCENT_FILL = "#007541"
LANE = "#1a7446"
LANE_EMPTY = "#171717"
PLAYHEAD = "#ffffff"
# Rows start this far inside a panel (theme.reco_pad), and a status line's
# words after its dot (theme.reco_dot + reco_gap).
PAD = 14
DOT_INDENT = 7 + 10
# Adjust rows: label cell, then a gap, then the control (theme.reco_label_width,
# reco_gap).
LABEL_WIDTH = 124
GAP = 10
# The sample match: 1:45:00 with the playhead at 12:34; the ruler's height
# and a lane's (theme.reco_ruler_height, reco_lane_row).
LENGTH, PLAYHEAD_AT = 6300.0, 754.0
RULER, LANE_HEIGHT = 18, 18
# A row (theme.reco_row): section bands and control rows.
ROW = 28

# Every --look-preview state in job order; None is a fresh start.
STATES = (None, "one-camera", "cameras", "calibrating", "calibration-failed", "ready", "exporting")
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
        "next_actions": state in (None, "one-camera", "cameras", "calibration-failed"),
        "calibrate_progress": state == "calibrating",
        "export_card": state == "exporting",
        "add_left": not left,
        "more_left": left,
        "left_fold": left,
        "add_right": not right,
        "more_right": right,
        "right_fold": right,
        "link_idle": not right,
        "link_on": right,
        # The time panel shows lanes once a camera has video, and the time
        # once there is a stitched preview to play.
        "lanes": left,
        "time_display": stitched,
        "lane_left_badge_on": left,
        "lane_right_badge_on": right,
        "auto_calibrate": not stitched,
        "recalibrate": stitched,
        "cal_dot_busy": state == "calibrating",
        "cal_dot_error": state == "calibration-failed",
        "cal_dot_ok": stitched,
    }


def takes_input(state):
    """Which gated controls take input (Makepad's own enabled flag). An
    export locks playback (Module 6): the transport and the ruler wait."""
    stitched = state in STITCHED
    gates = {
        "export_button": state == "ready",
        "toggle_inspector": stitched,
        "step_back": state == "ready",
        "play_pause": state == "ready",
        "step_forward": state == "ready",
        "aspect": stitched,
        "record_button": state == "ready",
    }
    # The timeline is drawn (and so gated) once the lanes show.
    if state is not None:
        gates["timeline"] = state == "ready"
    if not stitched:
        gates["auto_calibrate"] = state in ("cameras", "calibration-failed")
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


def ink_right(png, scale, box, bg, threshold=40):
    """Rightmost x (points) in box holding ink, as ink_left from the right."""
    x0, y0, x1, y1 = box
    bg_lum = lum(drive.hex_rgb(bg))
    for px in range(int(x1 * scale) - 1, int(x0 * scale), -1):
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
    app = drive.App.launch(BIN, args)
    first_frame(app, size)
    return app


def first_frame(app, size, secs=10.0):
    """Wait until the window has its size and its first frame: the app
    sizes and draws it a moment after it appears, and a check that looked
    sooner saw the default size (then probed the picture at the wrong
    scale) or no menu button yet. After `secs` it goes on, and the checks
    say what is wrong."""
    deadline = time.monotonic() + secs
    while time.monotonic() < deadline:
        width, height = app.get("/s")["w"][0]["sz"]
        if abs(width - size[0]) <= 2 and abs(height - size[1]) <= 2 and app.rect("app_menu"):
            return
        time.sleep(0.1)


def check_text_edge(app, png, scale, widget_id, edge, bg, name):
    """A label's ink starts on the edge (a glyph's side bearing is under 2 pt)."""
    rect = app.rect(widget_id)
    if rect is None:
        expect(False, f"{name}: `{widget_id}` is on screen")
        return
    x, y, w, h = rect
    ink = ink_left(png, scale, (edge - 6, y, x + w + 6, y + h), bg)
    expect(ink is not None and edge - 0.5 <= ink <= edge + 2,
           f"{name}: `{widget_id}` text starts on the edge {edge} (ink at {ink})")


def check_box_edge(app, widget_id, edge, name):
    rect = app.rect(widget_id)
    expect(rect is not None and abs(rect[0] - edge) <= 0.5,
           f"{name}: `{widget_id}` starts on the edge {edge} ({rect and rect[0]})")


def check_icon_right(app, png, scale, rect, edge, bg, label, name):
    """A trailing icon's ink ends on the content edge (an icon's drawing stops
    up to 2 pt inside its box)."""
    x, y, w, h = rect
    ink = ink_right(png, scale, (x, y + 2, edge + 6, y + h - 2), bg)
    expect(ink is not None and edge - 2.5 <= ink <= edge + 0.5,
           f"{name}: {label} ends on the content edge {edge} (ink at {ink})")


def check_state(size, state):
    name = f"{name_of(state)}-{size[0]}x{size[1]}"
    stitched = state in STITCHED
    with launch(size, state) as app:
        info = app.get("/s")["w"][0]
        width_pt = info["sz"][0]
        expect(info["t"].startswith("Reco"), f"{name}: window title is Reco ({info['t']!r})")
        expect(abs(info["sz"][0] - size[0]) <= 2 and abs(info["sz"][1] - size[1]) <= 2,
               f"{name}: window size {info['sz']} matches {size}")
        for wid in ("app_menu", "export_button", "toggle_media", "toggle_timeline", "toggle_inspector",
                    "view_bar", "play_pause"):
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

        # Rerun's surfaces: neutral grey panels and bands, a black viewport.
        mx, my, mw, mh = app.rect("media_panel")
        panel = pixel_at(png, scale, mx + mw / 2, my + mh - 8)
        expect(drive.close_to(panel, PANEL, tol=2), f"{name}: Setup panel is {PANEL} ({panel[:3]})")
        cx, cy, cw, ch = app.rect("cameras")
        band = pixel_at(png, scale, cx + cw / 2, cy + 3)
        expect(drive.close_to(band, BAND, tol=2), f"{name}: section band is {BAND} ({band[:3]})")
        vx, vy, vw, vh = app.rect("canvas")
        corner = pixel_at(png, scale, vx + 3, vy + 3)
        expect(drive.close_to(corner, VIEWPORT, tol=2), f"{name}: viewport is {VIEWPORT} ({corner[:3]})")

        face = pixel_at(png, scale, ex + 6, ey + eh / 2)
        export_on = state == "ready"
        expect(drive.close_to(face, ACCENT_FILL, tol=12) == export_on,
               f"{name}: Export face is {'green' if export_on else 'grey'} ({face[:3]})")
        if not export_on:
            # A disabled button greys its icon too: no green ink anywhere on it.
            tinted = [p[:3] for p in (pixel_at(png, scale, ex + dx, ey + dy)
                                      for dx in range(2, int(ew) - 2) for dy in range(2, int(eh) - 2))
                      if p[1] - max(p[0], p[2]) > 8]
            expect(not tinted, f"{name}: disabled Export has no green ink ({tinted[:2]})")

        tx, ty, tw, th = app.rect("toggle_inspector")
        # Lucide's panel-right: an outline whose divider stands 2 pt right of
        # the centre. The brightest pixel around it is the icon's colour.
        probe = (tx + tw / 2 + 2.0, ty + th / 2)
        tpng, tscale = settled(app, os.path.join(OUT, f"{name}-toggle.png"), width_pt, probe)
        icon = max((pixel_at(tpng, tscale, probe[0] + dx / 2, probe[1] + dy / 2)
                    for dx in range(-3, 4) for dy in range(-3, 4)), key=lambda p: max(p[:3]))
        expect((max(icon[:3]) > 130) == stitched,
               f"{name}: Adjust toggle icon {'bright' if stitched else 'dimmed'} ({icon[:3]})")

        # The camera pair's link lights up once both cameras have video.
        if shown(state)["link_on"]:
            # Snapshot rects are whole points; the hairline may sit on a half.
            lx, ly, lw, lh = app.rect("link_on")
            near = [pixel_at(png, scale, lx + dx / scale, ly + lh / 2)
                    for dx in range(int(-1.5 * scale), int((lw + 1.5) * scale) + 1)]
            lit = [p[:3] for p in near if drive.close_to(p, ACCENT, tol=40)]
            expect(bool(lit), f"{name}: camera link is lit ({[p[:3] for p in near]})")

        # One primary per screen: the Setup panel's Auto-calibrate is a
        # secondary button, so the viewer's call to action stands alone.
        if state == "cameras":
            ax, ay, aw, ah = app.rect("auto_calibrate")
            face = pixel_at(png, scale, ax + 4, ay + ah / 2)
            expect(not drive.close_to(face, ACCENT_FILL, tol=12),
                   f"{name}: Setup panel's Auto-calibrate is secondary ({face[:3]})")

        # The time panel: each camera's files in its lane, the playhead only
        # over a stitched preview, the controls on one line.
        if state is not None:
            rx, ry, rw, rh = app.rect("timeline")
            for lane, has in ((0, True), (1, state != "one-camera")):
                y = ry + RULER + lane * LANE_HEIGHT + LANE_HEIGHT / 2
                fill = pixel_at(png, scale, rx + rw * 0.73, y)
                want = LANE if has else LANE_EMPTY
                expect(drive.close_to(fill, want, tol=3),
                       f"{name}: lane {lane + 1} shows {'files' if has else 'no video'} ({fill[:3]})")
            head_x = rx + rw * PLAYHEAD_AT / LENGTH
            head = pixel_at(png, scale, head_x, ry + RULER + LANE_HEIGHT / 2)
            expect(drive.close_to(head, PLAYHEAD, tol=40) == stitched,
                   f"{name}: playhead {'shown' if stitched else 'hidden'} ({head[:3]})")
        # One family of transport icons: play is the largest; the steps are
        # quieter and smaller.
        heights = {}
        for wid in ("step_back", "play_pause", "step_forward"):
            x, y, w, h = app.rect(wid)
            rows = [py_ for py_ in range(int(y * scale), int((y + h) * scale))
                    if any(abs(lum(png.pixel(px_, py_)) - lum(drive.hex_rgb(PANEL))) > 40
                           for px_ in range(int(x * scale), int((x + w) * scale)))]
            heights[wid] = (rows[-1] - rows[0] + 1) / scale if rows else 0
        expect(heights["step_back"] <= heights["play_pause"] - 2
               and heights["step_forward"] <= heights["play_pause"] - 2,
               f"{name}: step icons smaller than play ({heights})")
        px, py, pw, ph = app.rect("play_pause")
        centre = py + ph / 2
        for wid in ("step_back", "step_forward", "time_current", "status_text"):
            rect = app.rect(wid)
            if rect:
                x, y, w, h = rect
                expect(abs(y + h / 2 - centre) <= 2,
                       f"{name}: `{wid}` centred on the control row ({y + h / 2:.1f} vs {centre:.1f})")

        # One content edge per panel: titles, rows and buttons start 14 pt
        # in; trailing icons end 14 pt from the right; the time panel's
        # first icon and lane badges share the same edge.
        if size[0] >= 1280:
            edge, right = mx + PAD, mx + mw - PAD
            check_text_edge(app, png, scale, "setup_header", edge, PANEL, name)
            check_box_edge(app, "left_badge_on" if state else "left_badge", edge, name)
            check_box_edge(app, "recalibrate" if stitched else "auto_calibrate", edge, name)
            for wid in ("calibration_status", "calibration_detail"):
                check_text_edge(app, png, scale, wid, edge + DOT_INDENT, PANEL, name)
            check_icon_right(app, png, scale, app.rect("recent_menu"), right, PANEL, "Recent files icon", name)
            check_icon_right(app, png, scale, (cx, cy, cw, ROW), right, BAND, "Cameras help icon", name)
            if state:
                check_icon_right(app, png, scale, app.rect("left_fold"), right, PANEL, "Files chevron", name)
            sx, sy, sw, sh = app.rect("step_back")
            ink = ink_left(png, scale, (0, sy + 2, sx + sw, sy + sh - 2), PANEL)
            expect(ink is not None and PAD - 1 <= ink <= PAD + 2,
                   f"{name}: time panel's first icon starts on the edge {PAD} (ink at {ink})")
            if state:
                check_box_edge(app, "lane_left_badge_on", PAD, name)

        # The next step fits the viewer: no text runs into its right edge.
        if not stitched:
            sx, sy, sw, sh = app.rect("empty_state")
            ink = ink_left(png, scale, (vx + vw - PAD, sy, vx + vw - 1, sy + sh), VIEWPORT)
            expect(ink is None, f"{name}: next step clear of the viewer's right edge (ink at {ink})")
        if state == "calibrating":
            bx, by, bw, bh = app.rect("step2_current")
            badge = pixel_at(png, scale, bx + bw / 2, by + 3)
            expect(drive.close_to(badge, ACCENT_FILL, tol=12),
                   f"{name}: stepper not covered while calibrating ({badge[:3]})")

        expect(app.errors() == [], f"{name}: no errors in the app log")


def check_adjust_edges():
    """Adjust rows: labels on the content edge, controls in one column after
    the label cell, values ending on the right content edge."""
    name = "adjust-1280x820"
    with launch((1280, 820), "ready") as app:
        png = app.grab(os.path.join(OUT, f"{name}.png"))
        scale = png.width / app.get("/s")["w"][0]["sz"][0]
        ix, iy, iw, ih = app.rect("inspector")
        edge, right = ix + PAD, ix + iw - PAD
        check_text_edge(app, png, scale, "adjust_header", edge, PANEL, name)
        for wid in ("fov_label", "match_label", "seam_label"):
            check_box_edge(app, wid, edge, name)
        for wid in ("fov_slider", "seam_blend"):
            check_box_edge(app, wid, edge + LABEL_WIDTH + GAP, name)
        for wid in ("fov_value", "seam_value"):
            x, y, w, h = app.rect(wid)
            ink = ink_right(png, scale, (right - 60, y, right + 6, y + h), PANEL)
            expect(ink is not None and right - 2 <= ink <= right + 0.5,
                   f"{name}: `{wid}` ends on the content edge {right} (ink at {ink})")
        vx, vy, vw, vh = app.rect("view_section")
        check_icon_right(app, png, scale, (vx, vy, vw, ROW), right, BAND, "View band icons", name)
        expect(app.errors() == [], f"{name}: no errors in the app log")


def drag(app, x, y, to_x):
    """Drag horizontally from (x, y) to (to_x, y) with the left button."""
    app.get("/m", k="down", x=x, y=y)
    app.get("/m", k="move", x=(x + to_x) / 2, y=y)
    app.get("/m", k="move", x=to_x, y=y)
    app.get("/m", k="up", x=to_x, y=y, wait=1)


def click(app, rect):
    x, y, w, h = rect
    app.get("/m", k="down", x=x + w / 2, y=y + h / 2)
    app.get("/m", k="up", x=x + w / 2, y=y + h / 2, wait=1)


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
        app.click_id("toggle_timeline")
        expect(not visible(app, "lanes") and visible(app, "play_pause"),
               "toggle: time panel folds to its control row")
        app.grab(os.path.join(OUT, "ready-time-folded.png"))
        app.click_id("toggle_timeline")
        expect(visible(app, "lanes"), "toggle: time panel reopens")
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
        expect(media is not None and media[2] >= 215,
               f"resize: Setup stops at its 220 pt minimum ({media and media[2]})")
        ix, iy, iw, ih = app.rect("inspector")
        drag(app, ix - 3, bar_y, 1270)
        inspector = app.rect("inspector")
        expect(inspector is not None and inspector[2] >= 215,
               f"resize: Adjust stops at its 220 pt minimum ({inspector and inspector[2]})")
        expect(app.errors() == [], "toggle: no errors in the app log")
    with launch((1280, 820), None) as app:
        app.click_id("toggle_inspector")
        expect(not visible(app, "inspector"), "toggle: Adjust panel stays closed before a stitch")
    # The app menu and the recent-files menu open as menus.
    with launch((1280, 820), "ready") as app:
        for menu in ("app_menu", "recent_menu"):
            click(app, app.rect(menu))
            time.sleep(0.3)
            opened = app.rect(f"{menu}_list")
            expect(opened is not None, f"menu: `{menu}` opens a menu ({opened})")
            app.grab(os.path.join(OUT, f"ready-{menu}.png"))
            app.key("Escape")
            time.sleep(0.3)
        expect(app.errors() == [], "menu: no errors in the app log")


def check_menus():
    """The app menu is Reco's own: its text starts on the menu's 14 pt
    content edge (Makepad's menu kept an empty mark column), a click on an
    item does it and closes the menu, and Escape closes it."""
    with launch((1280, 820), "ready") as app:
        click(app, app.rect("app_menu"))
        time.sleep(0.4)
        menu = app.rect("app_menu_list")
        expect(menu is not None, "menus: the app menu opens")
        items = {i.get("t"): i["r"] for i in app.snap("label") if i.get("i") == "label"}
        first = items.get("Keyboard shortcuts")
        expect(menu is not None and first is not None and abs(first[0] - menu[0] - 14) <= 1,
               f"menus: the text starts on the content edge ({first} in {menu})")
        app.grab(os.path.join(OUT, "app-menu.png"))
        app.get("/click", x=first[0] + 20, y=first[1] + first[3] / 2, wait=1)
        time.sleep(0.5)
        expect(app.rect("shortcuts_close") is not None and app.rect("app_menu_list") is None,
               "menus: a click on an item does it and closes the menu")
        app.key("Escape")
        time.sleep(0.3)
        click(app, app.rect("app_menu"))
        time.sleep(0.4)
        app.key("Escape")
        time.sleep(0.4)
        expect(app.rect("app_menu_list") is None, "menus: Escape closes it")
        expect(app.errors() == [], "menus: no errors in the app log")


def check_dropdowns():
    """A dropdown's menu opens below it, rows where they always are: a click
    on each row picks that row whatever was chosen, and no row lies in the
    title bar (where a real press drags the window and never reaches the
    menu; the owner found the top row unclickable)."""
    with launch((1280, 820), "ready") as app:
        r = app.rect("record_quality")
        bottom = r[1] + r[3]
        expect(bottom > 32, f"dropdown: the quality dropdown sits below the title bar ({r})")
        for row, label in ((0, "Fast"), (2, "High"), (1, "Balanced"), (0, "Fast")):
            click(app, r)
            time.sleep(0.4)
            # The menu's 4 pt padding, then one 28 pt row each (theme.reco_row).
            app.get("/click", x=r[0] + r[2] / 2, y=bottom + 4 + ROW / 2 + ROW * row, wait=1)
            time.sleep(0.4)
            picked = next((i.get("t") for i in app.snap("record_quality") if i.get("i") == "record_quality"), None)
            expect(picked == label, f"dropdown: the click on row {row + 1} picks {label} ({picked})")
        expect(app.errors() == [], "dropdown: no errors in the app log")


def main():
    os.makedirs(OUT, exist_ok=True)
    for state in STATES:
        check_state((1280, 820), state)
    for size in ((720, 600), (1920, 1200)):
        for state in (None, "calibrating", "ready"):
            check_state(size, state)
    check_adjust_edges()
    check_toggles()
    check_menus()
    check_dropdowns()
    if FAILURES:
        print(f"\nModule 0 check FAILED: {len(FAILURES)} failure(s):")
        for message in FAILURES:
            print(f"  - {message}")
        sys.exit(1)
    print(f"Module 0 check passed. Screenshots: {OUT}")


if __name__ == "__main__":
    main()
