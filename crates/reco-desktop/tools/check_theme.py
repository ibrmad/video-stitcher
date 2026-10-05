"""Theme smoke check: the window background is the Reco panel colour (#0d0d0d)
and the app log is clean; text on a 1x display is drawn fuller than its exact
coverage, and Retina text is left exact. Run from the repo root after
`cargo build --profile desktop -p reco-desktop`."""
import sys
import time

sys.path.insert(0, "crates/reco-desktop/tools")
import drive

BIN = "target/desktop/reco-desktop"

app = drive.App.launch(BIN)
try:
    png = app.grab("target/desktop-checks/m0/theme-smoke.png")
    corner = png.pixel(png.width - 4, png.height - 4)
    errors = app.errors()
finally:
    app.quit()
print("corner", corner[:3], "errors", errors)
assert drive.close_to(corner, "#0d0d0d", tol=3), f"background {corner[:3]} is not #0d0d0d"
assert not errors, errors


def title_ink(dpi):
    """The Setup title's ink at `dpi` pixels a point, in square points: its
    white coverage over the panel, summed and divided by dpi squared. Exact
    coverage keeps a glyph's area at any density."""
    # The window's size is in the display's points: twice the size at 2x
    # keeps the layout wide enough for the Setup panel on a 1x display.
    size = f"{800 * dpi}x{500 * dpi}"
    with drive.App.launch(BIN, ["--window-size", size, "--dpi", str(dpi), "--look-preview=ready"]) as app:
        time.sleep(1.5)
        first = next(i["r"] for i in app.snap("setup_header") if i.get("i") == "setup_header")
        png = app.grab(f"target/desktop-checks/m0/theme-text-{dpi}x.png")
        scale = png.width / app.get("/s")["w"][0]["sz"][0]
        assert abs(scale - dpi) < 0.01, f"drawn at {scale}x, not {dpi}x"
        x0, y0 = int((first[0] - 3) * scale), int((first[1] - 5) * scale)
        x1, y1 = int((first[0] + 45) * scale), int((first[1] + first[3] + 6) * scale)
        values = [max(png.pixel(x, y)[:3]) for y in range(y0, y1) for x in range(x0, x1)]
        background = min(values)
        return sum((v - background) / (255 - background) for v in values) / (scale * scale)


# Exact coverage at 1x gives about 0.94 of the 2x ink (thin and grey next to
# macOS's own text); the theme's curve gives about 1.12.
exact, drawn = title_ink(2), title_ink(1)
print(f"title ink: {exact:.1f} pt² at 2x, {drawn:.1f} pt² at 1x ({drawn / exact:.2f}x)")
assert drawn / exact >= 1.05, f"1x text is no fuller than its exact coverage ({drawn / exact:.2f}x)"
print("theme check passed")
