#!/usr/bin/env python3
"""Package Reco Desktop for release, so it runs without its source tree.

A plain `cargo build` binary reads Makepad's fonts and icons from the source
folders it was built from. A packaged one is built with
`MAKEPAD_PACKAGE_DIR=resources` and carries them:

- macOS: `cargo makepad desktop bundle` (Makepad's packager, from the pinned
  revision) makes a signed `Reco.app`; files given with `--extra` (the ONNX
  Runtime library) go into `Contents/MacOS` and the bundle is signed again.
- Windows and Linux: this script builds in its own target folder (the plain
  binary is left alone) and lays out cargo-makepad's layout beside the
  binary:

      reco-desktop[.exe]
      reco-desktop[.exe].makepad-package-paths   crate<TAB>resources/crate
      resources/<crate>/resources/...             every crate's resources
      resources/<crate>/fonts/...                 the fonts the binary names

  The fonts come from the manifest `app_main!` links into the binary, read
  here from its text: cargo-makepad reads it from ELF and Mach-O sections
  only, and a Windows binary keeps no long section names.

Usage (from the workspace root):

    package.py --out dist [--name reco-desktop-v0.5.4-linux-x86_64]
               [--target TRIPLE] [--features load-dynamic] [--extra FILE ...]
    package.py --out dist --binary PATH       (package a binary built with
                                               MAKEPAD_PACKAGE_DIR=resources)

macOS takes `--adhoc` (the default) or `--cert=IDENTITY`, and needs
cargo-makepad (`--makepad-tool DIR` puts it on the PATH):

    cargo install --git https://github.com/makepad/makepad \\
        --rev 62691a290eb58f7d5234960524429aaada3e572c cargo-makepad --locked
"""
import argparse
import glob
import json
import os
import shutil
import subprocess
import sys

PACKAGE = "reco-desktop"
MANIFEST_FORMAT = b"format=makepad.font-assets.v1\n"
FONT_EXTENSIONS = (".ttf", ".otf", ".ttc")


class PackageError(Exception):
    """Packaging can't go on; the message says why."""


class FontManifest:
    """The fonts a binary names: their set, and their logical paths
    (`crate/resources/...` or `crate/fonts/...`)."""

    def __init__(self, set_name, assets):
        self.set = set_name
        self.assets = set(assets)


def read_font_manifest(data):
    """The font manifest in a binary's bytes: each block the binary links
    (one per `app_main!`) starts with its format line, then `set=` and
    `asset=` lines. Blocks are united; they must agree on the set."""
    set_name, assets, start = None, set(), data.find(MANIFEST_FORMAT)
    if start < 0:
        raise PackageError("the binary carries no font manifest (makepad.font-assets.v1)")
    while start >= 0:
        cursor = start + len(MANIFEST_FORMAT)
        while True:
            end = data.find(b"\n", cursor)
            if end < 0:
                break
            line = data[cursor:end]
            if line.startswith(b"set="):
                value = line[4:].decode()
                if set_name not in (None, value):
                    raise PackageError(f"the binary names two font sets ({set_name}, {value})")
                set_name = value
            elif line.startswith(b"asset="):
                assets.add(line[6:].decode())
            else:
                break
            cursor = end + 1
        start = data.find(MANIFEST_FORMAT, cursor)
    if set_name is None or not assets:
        raise PackageError("the binary's font manifest names no fonts")
    return FontManifest(set_name, assets)


def dependency_dirs(metadata, package):
    """`package` and every crate it links (normal dependencies, all the
    way down), from `cargo metadata --filter-platform`: (crate name with
    underscores, crate folder), the package first, then by name."""
    packages = {p["id"]: p for p in metadata["packages"]}
    nodes = {n["id"]: n for n in metadata["resolve"]["nodes"]}
    root = next((p["id"] for p in metadata["packages"] if p["name"] == package), None)
    if root is None:
        raise PackageError(f"cargo metadata has no package {package}")
    seen, todo = {root}, [root]
    while todo:
        for dep in nodes[todo.pop()]["deps"]:
            normal = any(kind.get("kind") is None for kind in dep.get("dep_kinds", []))
            if normal and dep["pkg"] not in seen:
                seen.add(dep["pkg"])
                todo.append(dep["pkg"])

    def entry(pid):
        p = packages[pid]
        return p["name"].replace("-", "_"), os.path.dirname(p["manifest_path"])

    out, names = [entry(root)], {entry(root)[0]}
    for name, folder in sorted(entry(pid) for pid in seen if pid != root):
        if name not in names:
            names.add(name)
            out.append((name, folder))
    return out


def is_font(path):
    return path.lower().endswith(FONT_EXTENSIONS)


def copy_tree(source, dest, logical, manifest, packaged, keep):
    """Copy `source` into `dest`: a font only when the manifest names it
    (`logical/relative`), any file only when `keep(relative)`."""
    for folder, dirs, files in os.walk(source):
        dirs.sort()
        for name in sorted(files):
            path = os.path.join(folder, name)
            relative = os.path.relpath(path, source).replace(os.sep, "/")
            if not keep(relative):
                continue
            if is_font(name):
                logical_path = f"{logical}/{relative}"
                if logical_path not in manifest.assets:
                    continue
                packaged[logical_path] = os.path.getsize(path)
            target = os.path.join(dest, *relative.split("/"))
            os.makedirs(os.path.dirname(target), exist_ok=True)
            shutil.copy2(path, target)


def assemble(out_dir, binary, exe_name, crates, manifest):
    """Lay out a package in `out_dir`: the binary as `exe_name`, each
    crate's resources and named fonts under `resources/`, and the package
    map. Returns the fonts packaged (logical path to bytes)."""
    os.makedirs(out_dir, exist_ok=True)
    shutil.copy2(binary, os.path.join(out_dir, exe_name))
    packaged, mapped = {}, []
    for name, folder in crates:
        dest = os.path.join(out_dir, "resources", name)
        resources = os.path.join(folder, "resources")
        copy_tree(resources, os.path.join(dest, "resources"), f"{name}/resources", manifest, packaged,
                  lambda relative: not relative.split("/")[0].startswith("android"))
        # A font kept under fonts/ that also sits in resources/ is there already.
        copy_tree(os.path.join(folder, "fonts"), os.path.join(dest, "fonts"), f"{name}/fonts", manifest,
                  packaged, lambda relative, resources=resources: is_font(relative)
                  and not os.path.isfile(os.path.join(resources, *relative.split("/"))))
        if os.path.isdir(dest):
            mapped.append(name)
    missing = sorted(manifest.assets - set(packaged))
    if missing:
        raise PackageError(f"fonts the binary names are missing on disk: {', '.join(missing)}")
    rows = "".join(f"{name}\tresources/{name}\n" for name in mapped)
    with open(os.path.join(out_dir, f"{exe_name}.makepad-package-paths"), "w", newline="\n") as f:
        f.write(rows)
    return packaged


def run(command, env=None):
    print("+", " ".join(command), flush=True)
    result = subprocess.run(command, env=env)
    if result.returncode != 0:
        raise PackageError(f"{command[0]} failed ({result.returncode})")


def host_triple():
    out = subprocess.run(["rustc", "-vV"], capture_output=True, text=True, check=True).stdout
    return next(line.split(": ", 1)[1] for line in out.splitlines() if line.startswith("host: "))


def cargo_args(args):
    out = ["-p", PACKAGE, "--bin", PACKAGE, "--release"]
    if args.features:
        out += ["--features", args.features]
    return out


def package_plain(args):
    """Windows and Linux: build (unless given a binary) and lay out."""
    triple = args.target or host_triple()
    exe_suffix = ".exe" if "windows" in triple else ""
    if args.binary:
        binary = args.binary
    else:
        target_dir = os.path.abspath(args.target_dir)
        env = dict(os.environ, CARGO_TARGET_DIR=target_dir, MAKEPAD_PACKAGE_DIR="resources")
        command = ["cargo", "build", *cargo_args(args)]
        if args.target:
            command += ["--target", args.target]
        run(command, env)
        binary = os.path.join(target_dir, *([args.target] if args.target else []), "release",
                              PACKAGE + exe_suffix)
    with open(binary, "rb") as f:
        manifest = read_font_manifest(f.read())
    metadata = json.loads(subprocess.run(
        ["cargo", "metadata", "--format-version", "1", "--filter-platform", triple],
        capture_output=True, text=True, check=True).stdout)
    out_dir = os.path.join(args.out, args.name or f"{PACKAGE}-{triple}")
    if os.path.exists(out_dir):
        shutil.rmtree(out_dir)
    packaged = assemble(out_dir, binary, PACKAGE + exe_suffix, dependency_dirs(metadata, PACKAGE), manifest)
    for extra in args.extra:
        shutil.copy2(extra, out_dir)
    print(f"fonts ({manifest.set}): "
          + ", ".join(f"{path} ({size:,} bytes)" for path, size in sorted(packaged.items())))
    print(f"package: {out_dir}")
    return out_dir


def package_macos(args):
    """macOS: cargo-makepad's signed bundle, the extras inside, signed
    again."""
    env = dict(os.environ)
    if args.makepad_tool:
        env["PATH"] = os.path.abspath(args.makepad_tool) + os.pathsep + env.get("PATH", "")
    if shutil.which("cargo-makepad", path=env.get("PATH")) is None:
        raise PackageError("cargo-makepad isn't installed (see this script's help)")
    sign = f"--cert={args.cert}" if args.cert else "--adhoc"
    run(["cargo", "makepad", "desktop", "bundle", sign, *cargo_args(args)], env)
    target = os.environ.get("CARGO_TARGET_DIR", "target")
    bundles = glob.glob(os.path.join(target, "makepad-bundle", "bundles", "release", "*.app"))
    if len(bundles) != 1:
        raise PackageError(f"expected one .app from cargo-makepad, found {bundles}")
    os.makedirs(args.out, exist_ok=True)
    app = os.path.join(args.out, os.path.basename(bundles[0]))
    if os.path.exists(app):
        shutil.rmtree(app)
    shutil.copytree(bundles[0], app, symlinks=True)
    if args.extra:
        identity = args.cert or "-"
        for extra in args.extra:
            placed = os.path.join(app, "Contents", "MacOS", os.path.basename(extra))
            shutil.copy2(extra, placed)
            run(["codesign", "--force", "--sign", identity, placed])
        run(["codesign", "--force", "--sign", identity, app])
        run(["codesign", "--verify", "--strict", app])
    print(f"package: {app}")
    return app


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--out", required=True, help="folder the package goes in")
    parser.add_argument("--name", help="the package folder's name (Windows and Linux)")
    parser.add_argument("--target", help="target triple (Windows and Linux)")
    parser.add_argument("--features", help="cargo features, e.g. load-dynamic")
    parser.add_argument("--extra", action="append", default=[], help="a file to ship beside the binary")
    parser.add_argument("--binary", help="package this binary instead of building (Windows and Linux)")
    parser.add_argument("--target-dir", default=os.path.join("target", "makepad-package"),
                        help="the package build's own target folder (Windows and Linux)")
    parser.add_argument("--cert", help="macOS signing identity (default: ad hoc)")
    parser.add_argument("--adhoc", action="store_true", help="macOS: sign ad hoc (the default)")
    parser.add_argument("--makepad-tool", help="macOS: a folder holding cargo-makepad")
    args = parser.parse_args(argv)
    try:
        if sys.platform == "darwin" and not args.binary and not args.target:
            package_macos(args)
        else:
            package_plain(args)
    except (PackageError, subprocess.CalledProcessError, OSError) as error:
        print(f"package: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
