#!/usr/bin/env python3
"""Drive a Reco Desktop (Makepad 2) instance through its --remote HTTP API.

Used by the module checks (tools/check_m<N>.py). Standard library only.

    with App.launch(BIN, ["--window-size", "1280x820"]) as app:
        app.click_id("toggle_media")
        png = app.grab("target/desktop-checks/m0/media-closed.png")

Input routes are sent with wait=1, so the next request sees the frame that
followed the input; no sleeps are needed between steps.
"""
import json
import os
import re
import shutil
import struct
import subprocess
import tempfile
import time
import urllib.error
import urllib.parse
import urllib.request
import zlib

LISTEN_RE = re.compile(r"\[makepad-remote\] listening on 127\.0\.0\.1:(\d+) pid=(\d+)")


class DriveError(Exception):
    """A remote request failed or the app did not behave as required."""


def parse_listen_line(text):
    """Return (port, pid) from the app's startup output, or None."""
    m = LISTEN_RE.search(text)
    return (int(m.group(1)), int(m.group(2))) if m else None


def hex_rgb(hex_colour):
    """'#0f1115' -> (15, 17, 21)."""
    h = hex_colour.lstrip("#")
    return tuple(int(h[i:i + 2], 16) for i in (0, 2, 4))


def close_to(rgba, hex_colour, tol=3):
    """True when rgba's RGB is within tol of the hex colour on every channel."""
    return all(abs(a - b) <= tol for a, b in zip(rgba[:3], hex_rgb(hex_colour)))


class Png:
    """A decoded 8-bit RGBA image."""

    def __init__(self, width, height, rgba):
        self.width, self.height, self.rgba = width, height, rgba

    def pixel(self, x, y):
        """RGBA tuple at integer pixel (x, y)."""
        i = (y * self.width + x) * 4
        return tuple(self.rgba[i:i + 4])


def _paeth(a, b, c):
    p = a + b - c
    pa, pb, pc = abs(p - a), abs(p - b), abs(p - c)
    if pa <= pb and pa <= pc:
        return a
    return b if pb <= pc else c


def read_png(path):
    """Decode an 8-bit RGB or RGBA, non-interlaced PNG."""
    with open(path, "rb") as f:
        data = f.read()
    if data[:8] != b"\x89PNG\r\n\x1a\n":
        raise DriveError(f"{path}: not a PNG")
    pos, idat = 8, b""
    width = height = colour_type = None
    while pos < len(data):
        length, kind = struct.unpack(">I4s", data[pos:pos + 8])
        body = data[pos + 8:pos + 8 + length]
        if kind == b"IHDR":
            width, height, depth, colour_type, _, _, interlace = struct.unpack(">IIBBBBB", body)
            if depth != 8 or colour_type not in (2, 6) or interlace != 0:
                raise DriveError(f"{path}: unsupported PNG (depth {depth}, type {colour_type})")
        elif kind == b"IDAT":
            idat += body
        elif kind == b"IEND":
            break
        pos += 12 + length
    bpp = 4 if colour_type == 6 else 3
    raw = zlib.decompress(idat)
    stride = width * bpp
    out = bytearray()
    prev = bytearray(stride)
    i = 0
    for _ in range(height):
        ftype = raw[i]
        row = bytearray(raw[i + 1:i + 1 + stride])
        i += 1 + stride
        for x in range(stride):
            a = row[x - bpp] if x >= bpp else 0
            b = prev[x]
            c = prev[x - bpp] if x >= bpp else 0
            if ftype == 1:
                row[x] = (row[x] + a) & 0xFF
            elif ftype == 2:
                row[x] = (row[x] + b) & 0xFF
            elif ftype == 3:
                row[x] = (row[x] + ((a + b) >> 1)) & 0xFF
            elif ftype == 4:
                row[x] = (row[x] + _paeth(a, b, c)) & 0xFF
        if bpp == 3:
            for x in range(width):
                out += row[x * 3:x * 3 + 3] + b"\xff"
        else:
            out += row
        prev = row
    return Png(width, height, bytes(out))


class App:
    """One running instance launched with --remote. Close it with quit()."""

    def __init__(self, proc, port, log_path):
        self.proc, self.port, self.log_path = proc, port, log_path

    @classmethod
    def launch(cls, binary, args=(), hidden=True, timeout=60.0):
        """Start binary with --remote and wait for its control port."""
        env = dict(os.environ)
        if hidden:
            env["MAKEPAD_HIDE_WINDOWS"] = "1"
        log_fd, log_path = tempfile.mkstemp(prefix="reco-desktop-", suffix=".log")
        log = os.fdopen(log_fd, "w")
        proc = subprocess.Popen([binary, "--remote", *args], stdout=log, stderr=subprocess.STDOUT, env=env)
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            with open(log_path) as f:
                found = parse_listen_line(f.read())
            if found:
                app = cls(proc, found[0], log_path)
                app._wait_for_window(deadline)
                return app
            if proc.poll() is not None:
                break
            time.sleep(0.1)
        proc.kill()
        with open(log_path) as f:
            raise DriveError(f"app did not start remote control:\n{f.read()[-2000:]}")

    def _wait_for_window(self, deadline):
        """The control port opens before the first window exists; wait for it."""
        while time.monotonic() < deadline:
            try:
                if self.get("/s", timeout=5)["w"]:
                    return
            except (DriveError, OSError):
                pass
            time.sleep(0.1)
        self.quit()
        raise DriveError("app opened its control port but no window appeared")

    def __enter__(self):
        return self

    def __exit__(self, *exc):
        self.quit()

    def get(self, route, timeout=60.0, **params):
        """GET a route and return its JSON answer; raises on {"err": ...}."""
        query = urllib.parse.urlencode({k: v for k, v in params.items() if v is not None})
        url = f"http://127.0.0.1:{self.port}{route}" + (f"?{query}" if query else "")
        try:
            with urllib.request.urlopen(url, timeout=timeout) as r:
                answer = json.loads(r.read().decode())
        except urllib.error.HTTPError as e:
            raise DriveError(f"{route}: HTTP {e.code} {e.read().decode()[:300]}") from e
        if isinstance(answer, dict) and "err" in answer:
            raise DriveError(f"{route}: {answer['err']}")
        return answer

    def snap(self, q):
        """Widgets whose id, type or text contains q (window-local rects)."""
        return self.get("/snap", q=q)["s"]

    def rect(self, widget_id):
        """(x, y, w, h) of the widget with exactly this id, or None if not drawn."""
        for item in self.snap(widget_id):
            if item.get("i") == widget_id and item["r"][2] > 1 and item["r"][3] > 1:
                return tuple(item["r"])
        return None

    def enabled(self, widget_id):
        """Whether the widget with exactly this id takes input (Makepad's own
        `enabled` flag from /snap), or None when it is not found."""
        for item in self.snap(widget_id):
            if item.get("i") == widget_id and "enabled" in item:
                return bool(item["enabled"])
        return None

    def click_id(self, widget_id):
        """Click the centre of a widget found by id."""
        r = self.rect(widget_id)
        if r is None:
            raise DriveError(f"no visible widget `{widget_id}` to click")
        self.get("/click", x=r[0] + r[2] / 2, y=r[1] + r[3] / 2, wait=1)

    def key(self, code, **mods):
        """Press one key, e.g. key("Key1", cmd=1)."""
        self.get("/k", k="press", c=code, wait=1, **mods)

    def grab(self, dest, scale=1.0):
        """Capture the window to dest and return it decoded."""
        answer = self.get("/g", scale=scale)
        os.makedirs(os.path.dirname(dest), exist_ok=True)
        shutil.copyfile(answer["png"], dest)
        return read_png(dest)

    def errors(self):
        """Error lines ([E] or [!]) from the app's own log ring."""
        lines = self.get("/log", n=500)["l"]
        return [line for line in lines if line.startswith("[E]") or line.startswith("[!]")]

    def quit(self):
        """Grab-and-quit, then make sure the process is gone."""
        if self.proc.poll() is None:
            try:
                self.get("/gq", scale=0.25, timeout=30)
            except (DriveError, OSError):
                pass
            try:
                self.proc.wait(timeout=15)
            except subprocess.TimeoutExpired:
                self.proc.terminate()
                self.proc.wait(timeout=5)
