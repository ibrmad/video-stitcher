#!/usr/bin/env python3
"""Fetch the app's icons from Lucide (lucide.dev, ISC; see
THIRD_PARTY_NOTICES.md) into resources/icons, in the form the app draws:

- painted in #000: a widget tints its icon (`draw_icon.color`);
- an invisible rect pins the 24x24 box first, because Makepad scales an icon
  by its drawn content, not its viewBox;
- record and stop are filled (a solid dot and square read as states).

    lucide_icons.py            fetch every icon in ICONS
    lucide_icons.py FILE...    fetch the named files only (e.g. play)

Icons come from the npm package lucide-static at VERSION, through unpkg.
A new icon: add it to ICONS (our file name -> Lucide name, filled) and run
this script.
"""
import os
import re
import sys
import urllib.request

VERSION = "1.52.0"
SOURCE = "https://unpkg.com/lucide-static@{version}/icons/{name}.svg"
ICONS_DIR = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "resources", "icons"))

# Our file name (without .svg) -> (Lucide icon name, filled).
ICONS = {
    "aperture": ("aperture", False),
    "check": ("check", False),
    "chevron_down": ("chevron-down", False),
    "close": ("x", False),
    "export_video": ("square-play", False),
    "help": ("circle-question-mark", False),
    "history": ("history", False),
    "panel_bottom": ("panel-bottom", False),
    "pause": ("pause", False),
    "play": ("play", False),
    "plus": ("plus", False),
    "record": ("circle", True),
    "reset": ("rotate-ccw", False),
    "sidebar_left": ("panel-left", False),
    "sidebar_right": ("panel-right", False),
    "step_back": ("step-back", False),
    "step_forward": ("step-forward", False),
    "stop": ("square", True),
}

PIN = '<rect x="0" y="0" width="24" height="24" fill="none" stroke="none" />'
SHAPES = ("path", "circle", "rect", "line", "polyline", "polygon", "ellipse")


def convert(svg, filled):
    """A Lucide SVG in the app's form (see the module doc)."""
    out = svg.replace('stroke="currentColor"', 'stroke="#000"')
    if filled:
        shapes = "|".join(SHAPES)
        out = re.sub(rf"<({shapes})(\s)", r'<\1 fill="#000"\2', out)
    # The pin goes first, right after the root tag closes.
    root_end = out.index(">", out.index("<svg")) + 1
    return out[:root_end] + "\n  " + PIN + out[root_end:]


def fetch(name):
    url = SOURCE.format(version=VERSION, name=name)
    with urllib.request.urlopen(url, timeout=30) as response:
        return response.read().decode("utf-8")


def main():
    files = sys.argv[1:] or sorted(ICONS)
    for file in files:
        lucide, filled = ICONS[file]
        path = os.path.join(ICONS_DIR, f"{file}.svg")
        with open(path, "w") as f:
            f.write(convert(fetch(lucide), filled))
        print(f"{file}.svg <- lucide {lucide}{' (filled)' if filled else ''}")


if __name__ == "__main__":
    main()
