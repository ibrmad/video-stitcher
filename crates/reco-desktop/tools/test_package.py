#!/usr/bin/env python3
"""Unit tests for package.py (run: /usr/bin/python3 test_package.py)."""
import os
import tempfile
import unittest

import package


def block(*lines):
    return ("format=makepad.font-assets.v1\n" + "".join(f"{line}\n" for line in lines)).encode()


class FontManifest(unittest.TestCase):
    def test_reads_the_manifest_out_of_a_binary(self):
        data = b"\x00\x7fELF junk" + block("set=international", "asset=makepad_widgets/resources/Inter.ttf",
                                           "asset=makepad_widgets/fonts/NotoSans.ttf") + b"\x00\x01more"
        manifest = package.read_font_manifest(data)
        self.assertEqual(manifest.set, "international")
        self.assertEqual(manifest.assets, {"makepad_widgets/resources/Inter.ttf",
                                           "makepad_widgets/fonts/NotoSans.ttf"})

    def test_blocks_are_united(self):
        data = block("set=international", "asset=a/resources/A.ttf") + b"\x00" + \
            block("set=international", "asset=b/fonts/B.otf")
        self.assertEqual(package.read_font_manifest(data).assets, {"a/resources/A.ttf", "b/fonts/B.otf"})

    def test_a_binary_without_one_is_refused(self):
        with self.assertRaises(package.PackageError):
            package.read_font_manifest(b"no manifest here")

    def test_conflicting_sets_are_refused(self):
        data = block("set=international", "asset=a/resources/A.ttf") + block("set=latin", "asset=a/resources/A.ttf")
        with self.assertRaises(package.PackageError):
            package.read_font_manifest(data)


def node(pid, deps):
    return {"id": pid, "deps": [{"pkg": dep, "dep_kinds": [{"kind": kind, "target": None}]}
                                for dep, kind in deps]}


METADATA = {
    "packages": [
        {"id": "app", "name": "reco-desktop", "manifest_path": "/w/crates/reco-desktop/Cargo.toml"},
        {"id": "widgets", "name": "makepad-widgets", "manifest_path": "/g/makepad/widgets/Cargo.toml"},
        {"id": "draw", "name": "makepad-draw", "manifest_path": "/g/makepad/draw/Cargo.toml"},
        {"id": "devonly", "name": "makepad-key-code", "manifest_path": "/g/makepad/key/Cargo.toml"},
        {"id": "buildonly", "name": "cc", "manifest_path": "/r/cc/Cargo.toml"},
    ],
    "resolve": {
        "root": None,
        "nodes": [
            node("app", [("widgets", None), ("devonly", "dev"), ("buildonly", "build")]),
            node("widgets", [("draw", None)]),
            node("draw", []),
            node("devonly", []),
            node("buildonly", []),
        ],
    },
}


class Crates(unittest.TestCase):
    def test_the_app_and_what_it_links(self):
        crates = package.dependency_dirs(METADATA, "reco-desktop")
        self.assertEqual(crates, [
            ("reco_desktop", "/w/crates/reco-desktop"),
            ("makepad_draw", "/g/makepad/draw"),
            ("makepad_widgets", "/g/makepad/widgets"),
        ])


def write(path, text="x"):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "w") as f:
        f.write(text)


class Assemble(unittest.TestCase):
    def setUp(self):
        self.src = tempfile.mkdtemp(prefix="pkg-src-")
        self.out = tempfile.mkdtemp(prefix="pkg-out-")
        self.binary = os.path.join(self.src, "reco-desktop")
        write(self.binary, "binary")
        app = os.path.join(self.src, "app")
        write(os.path.join(app, "resources", "icons", "play.svg"))
        widgets = os.path.join(self.src, "widgets")
        write(os.path.join(widgets, "resources", "Inter.ttf"))
        write(os.path.join(widgets, "resources", "Unused.ttf"))
        write(os.path.join(widgets, "resources", "android", "only.xml"))
        write(os.path.join(widgets, "fonts", "NotoSans.ttf"))
        write(os.path.join(widgets, "fonts", "README.txt"))
        self.crates = [("reco_desktop", app), ("makepad_widgets", widgets),
                       ("makepad_draw", os.path.join(self.src, "draw"))]
        self.manifest = package.FontManifest("international", {
            "makepad_widgets/resources/Inter.ttf", "makepad_widgets/fonts/NotoSans.ttf"})

    def test_lays_out_resources_fonts_and_the_map(self):
        fonts = package.assemble(self.out, self.binary, "reco-desktop", self.crates, self.manifest)
        res = os.path.join(self.out, "resources")
        self.assertTrue(os.path.isfile(os.path.join(self.out, "reco-desktop")))
        self.assertTrue(os.path.isfile(os.path.join(res, "reco_desktop", "resources", "icons", "play.svg")))
        self.assertTrue(os.path.isfile(os.path.join(res, "makepad_widgets", "resources", "Inter.ttf")))
        self.assertTrue(os.path.isfile(os.path.join(res, "makepad_widgets", "fonts", "NotoSans.ttf")))
        self.assertFalse(os.path.exists(os.path.join(res, "makepad_widgets", "resources", "Unused.ttf")),
                         "only the fonts the binary names")
        self.assertFalse(os.path.exists(os.path.join(res, "makepad_widgets", "resources", "android")))
        self.assertFalse(os.path.exists(os.path.join(res, "makepad_widgets", "fonts", "README.txt")),
                         "fonts/ holds fonts only")
        self.assertFalse(os.path.exists(os.path.join(res, "makepad_draw")), "a crate with nothing to ship")
        with open(os.path.join(self.out, "reco-desktop.makepad-package-paths")) as f:
            self.assertEqual(f.read(), "reco_desktop\tresources/reco_desktop\n"
                                       "makepad_widgets\tresources/makepad_widgets\n")
        self.assertEqual(sorted(fonts), ["makepad_widgets/fonts/NotoSans.ttf",
                                         "makepad_widgets/resources/Inter.ttf"])

    def test_a_named_font_missing_on_disk_is_refused(self):
        manifest = package.FontManifest("international", {"makepad_widgets/resources/Gone.ttf"})
        with self.assertRaises(package.PackageError):
            package.assemble(self.out, self.binary, "reco-desktop", self.crates, manifest)

    def test_the_map_is_named_after_the_executable(self):
        exe = os.path.join(self.src, "reco-desktop.exe")
        write(exe, "binary")
        package.assemble(self.out, exe, "reco-desktop.exe", self.crates, self.manifest)
        self.assertTrue(os.path.isfile(os.path.join(self.out, "reco-desktop.exe.makepad-package-paths")))


if __name__ == "__main__":
    unittest.main()
