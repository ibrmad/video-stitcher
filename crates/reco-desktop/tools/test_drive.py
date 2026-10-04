"""Unit tests for drive.py's parsing helpers (no app needed)."""
import os
import struct
import tempfile
import unittest
import zlib

import drive


def write_png(path, width, height, rgba_rows, filter_type=0):
    """Write a minimal 8-bit RGBA PNG with the given rows."""
    raw = b"".join(bytes([filter_type]) + row for row in rgba_rows)

    def chunk(kind, data):
        body = kind + data
        return struct.pack(">I", len(data)) + body + struct.pack(">I", zlib.crc32(body))

    ihdr = struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0)
    with open(path, "wb") as f:
        f.write(b"\x89PNG\r\n\x1a\n")
        f.write(chunk(b"IHDR", ihdr))
        f.write(chunk(b"IDAT", zlib.compress(raw)))
        f.write(chunk(b"IEND", b""))


class ParseListenLine(unittest.TestCase):
    def test_finds_port_and_pid(self):
        text = "noise\n[makepad-remote] listening on 127.0.0.1:53412 pid=9931 app=reco-desktop grabs=/tmp/x\n"
        self.assertEqual(drive.parse_listen_line(text), (53412, 9931))

    def test_none_when_missing(self):
        self.assertIsNone(drive.parse_listen_line("[I] starting\n"))


class ReadPng(unittest.TestCase):
    def test_reads_unfiltered_pixels(self):
        with tempfile.TemporaryDirectory() as d:
            path = os.path.join(d, "a.png")
            write_png(path, 2, 1, [bytes([15, 17, 21, 255, 52, 211, 153, 255])])
            png = drive.read_png(path)
            self.assertEqual((png.width, png.height), (2, 1))
            self.assertEqual(png.pixel(0, 0), (15, 17, 21, 255))
            self.assertEqual(png.pixel(1, 0), (52, 211, 153, 255))

    def test_reads_up_filtered_rows(self):
        # Row 2 uses filter 2 (Up): stored bytes are differences to row 1.
        with tempfile.TemporaryDirectory() as d:
            path = os.path.join(d, "b.png")
            row1 = bytes([10, 20, 30, 255])
            diff = bytes([5, 5, 5, 0])
            raw = b"\x00" + row1 + b"\x02" + diff

            def chunk(kind, data):
                body = kind + data
                return struct.pack(">I", len(data)) + body + struct.pack(">I", zlib.crc32(body))

            with open(path, "wb") as f:
                f.write(b"\x89PNG\r\n\x1a\n")
                f.write(chunk(b"IHDR", struct.pack(">IIBBBBB", 1, 2, 8, 6, 0, 0, 0)))
                f.write(chunk(b"IDAT", zlib.compress(raw)))
                f.write(chunk(b"IEND", b""))
            png = drive.read_png(path)
            self.assertEqual(png.pixel(0, 1), (15, 25, 35, 255))


class ColourClose(unittest.TestCase):
    def test_tolerance(self):
        self.assertTrue(drive.close_to((15, 17, 21, 255), "#0f1115", tol=2))
        self.assertFalse(drive.close_to((40, 17, 21, 255), "#0f1115", tol=2))


if __name__ == "__main__":
    unittest.main()
