"""Theme smoke check: the window background is the Reco panel colour (#0d0d0d)
and the app log is clean. Run from the repo root after
`cargo build --profile desktop -p reco-desktop`."""
import sys
sys.path.insert(0, "crates/reco-desktop/tools")
import drive
app = drive.App.launch("target/desktop/reco-desktop")
try:
    png = app.grab("target/desktop-checks/m0/theme-smoke.png")
    corner = png.pixel(png.width - 4, png.height - 4)
    errors = app.errors()
finally:
    app.quit()
print("corner", corner[:3], "errors", errors)
assert drive.close_to(corner, "#0d0d0d", tol=3), f"background {corner[:3]} is not #0d0d0d"
assert not errors, errors
print("theme check passed")
