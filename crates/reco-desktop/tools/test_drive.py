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



class ParseCputime(unittest.TestCase):
    def test_parse_cputime(self):
        self.assertAlmostEqual(drive.parse_cputime("0:01.25"), 1.25)
        self.assertAlmostEqual(drive.parse_cputime("1:02:03.50"), 3723.5)



class LaunchEnv(unittest.TestCase):
    def test_each_launch_gets_its_own_settings_folder(self):
        a = drive.launch_env({"PATH": "/bin"}, None, hidden=True)
        b = drive.launch_env({"PATH": "/bin"}, None, hidden=True)
        self.assertEqual(a["MAKEPAD_HIDE_WINDOWS"], "1")
        self.assertTrue(os.path.isdir(a["RECO_CONFIG_DIR"]))
        self.assertNotEqual(a["RECO_CONFIG_DIR"], b["RECO_CONFIG_DIR"])

    def test_each_launch_gets_its_own_log_file(self):
        a = drive.launch_env({"HOME": "/Users/ann"}, None, hidden=True)["RECO_DESKTOP_LOG_FILE"]
        b = drive.launch_env({"HOME": "/Users/ann"}, None, hidden=True)["RECO_DESKTOP_LOG_FILE"]
        self.assertNotEqual(a, b)
        self.assertFalse(a.startswith("/Users/ann"), "never the person's own log")
        given = drive.launch_env({}, {"RECO_DESKTOP_LOG_FILE": "/tmp/x.log"}, hidden=True)
        self.assertEqual(given["RECO_DESKTOP_LOG_FILE"], "/tmp/x.log")

    def test_checks_stay_off_the_network(self):
        self.assertEqual(drive.launch_env({}, None, hidden=True)["RECO_DESKTOP_NO_NETWORK"], "1")
        self.assertEqual(drive.launch_env({}, None, hidden=True)["RECO_DESKTOP_NO_BROWSER"], "1")
        self.assertEqual(drive.launch_env({}, None, hidden=True)["RECO_DESKTOP_NO_CLIPBOARD"], "1")
        online = drive.launch_env({}, {"RECO_DESKTOP_NO_NETWORK": None}, hidden=True)
        self.assertNotIn("RECO_DESKTOP_NO_NETWORK", online)

    def test_a_given_folder_wins(self):
        env = drive.launch_env({}, {"RECO_CONFIG_DIR": "/tmp/x"}, hidden=False)
        self.assertEqual(env["RECO_CONFIG_DIR"], "/tmp/x")
        self.assertNotIn("MAKEPAD_HIDE_WINDOWS", env)


class GapAfterMark(unittest.TestCase):
    def strip(self, columns):
        """A one-row image: 0 is background, 1 is ink."""
        rgba = bytearray()
        for ink in columns:
            rgba += bytes([200, 200, 200, 255] if ink else [13, 13, 13, 255])
        return drive.Png(len(columns), 1, bytes(rgba))

    def test_measures_the_space_between_box_and_text(self):
        png = self.strip([1] * 30 + [0] * 16 + [1, 1, 0, 1] + [0] * 4)
        self.assertEqual(drive.gap_after_mark(png, (0, 0, 27, 0.5), 2.0), 8.0)

    def test_stuck_text_has_no_gap(self):
        png = self.strip([1] * 40 + [0] * 4)
        self.assertEqual(drive.gap_after_mark(png, (0, 0, 22, 0.5), 2.0), 0.0)


class ConflictWait(unittest.TestCase):
    def test_waits_out_the_quiet_window(self):
        answer = {"err": "user_interacting", "applied": False, "activity": {"idle_ms": 669, "quiet_ms": 2000}}
        self.assertAlmostEqual(drive.conflict_wait(answer), 1.581)

    def test_other_answers_do_not_retry(self):
        self.assertIsNone(drive.conflict_wait({"err": "no widget `x`"}))
        self.assertIsNone(drive.conflict_wait({"err": "user_interacting", "applied": True, "activity": {}}))


if __name__ == "__main__":
    unittest.main()
