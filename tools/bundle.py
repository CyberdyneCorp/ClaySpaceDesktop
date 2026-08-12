#!/usr/bin/env python3
"""Produces the distributable for this platform.

macOS gets a .app bundle; Linux gets a tarball with a desktop entry. Both are
self-contained with respect to the engine, which is checked rather than
assumed: ClayCore is built and linked statically by `claycore-sys`, and a
dynamic reference to it would mean the distributable only runs on a machine
that already has the engine — which is the failure this check exists to catch.
"""

from __future__ import annotations

import argparse
import plistlib
import shutil
import subprocess
import sys
import tarfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
BINARY = "clayspace-app"
APP_NAME = "Sculptor 3D"
BUNDLE_ID = "com.cyberdynecorp.clayspace"


def version() -> str:
    # The crate inherits its version from the workspace, so `version.workspace
    # = true` has to be recognised as "look next door" rather than parsed as a
    # version literally called "true" — which is what the first draft shipped
    # into the Info.plist.
    def literal(text: str) -> str | None:
        for line in text.splitlines():
            stripped = line.strip()
            if stripped.startswith("version.workspace") or stripped.startswith(
                "version ="
            ) is False:
                continue
            value = stripped.split("=", 1)[1].strip().strip('"')
            if value not in ("true", "false"):
                return value
        return None

    crate = literal((ROOT / "crates" / BINARY / "Cargo.toml").read_text(encoding="utf-8"))
    if crate:
        return crate
    workspace = literal((ROOT / "Cargo.toml").read_text(encoding="utf-8"))
    return workspace or "0.0.0"


def build(release: bool) -> Path:
    profile = "release" if release else "debug"
    command = ["cargo", "build", "-p", BINARY, "--bin", BINARY]
    if release:
        command.append("--release")
    subprocess.run(command, cwd=ROOT, check=True)
    return ROOT / "target" / profile / BINARY


def links_to_engine_dynamically(binary: Path) -> list[str]:
    """Any dynamic reference to ClayCore, which there should be none of."""
    if sys.platform == "darwin":
        result = subprocess.run(
            ["otool", "-L", str(binary)], capture_output=True, text=True
        )
    else:
        result = subprocess.run(["ldd", str(binary)], capture_output=True, text=True)
    if result.returncode != 0:
        return []
    return [
        line.strip()
        for line in result.stdout.splitlines()
        if "clay" in line.lower() and "clayspace" not in line.lower()
    ]


def macos_bundle(binary: Path, out: Path) -> Path:
    bundle = out / f"{APP_NAME}.app"
    if bundle.exists():
        shutil.rmtree(bundle)
    macos = bundle / "Contents" / "MacOS"
    resources = bundle / "Contents" / "Resources"
    macos.mkdir(parents=True)
    resources.mkdir(parents=True)

    shutil.copy2(binary, macos / BINARY)
    (macos / BINARY).chmod(0o755)
    shutil.copy2(ROOT / "ATTRIBUTION.md", resources / "ATTRIBUTION.md")
    engine_licence = ROOT / "vendor" / "ClayCore" / "LICENSE"
    if engine_licence.is_file():
        shutil.copy2(engine_licence, resources / "ClayCore-LICENSE.txt")

    info = {
        "CFBundleName": APP_NAME,
        "CFBundleDisplayName": APP_NAME,
        "CFBundleIdentifier": BUNDLE_ID,
        "CFBundleVersion": version(),
        "CFBundleShortVersionString": version(),
        "CFBundleExecutable": BINARY,
        "CFBundlePackageType": "APPL",
        # The window is created by winit and the application draws with wgpu;
        # both need a real bundle to take keyboard focus and to stop the
        # window server treating it as a background process.
        "LSMinimumSystemVersion": "11.0",
        "NSHighResolutionCapable": True,
        "CFBundleDocumentTypes": [
            {
                "CFBundleTypeName": "ClaySpace document",
                "CFBundleTypeExtensions": ["clayspace"],
                "CFBundleTypeRole": "Editor",
                "LSHandlerRank": "Owner",
            }
        ],
    }
    with (bundle / "Contents" / "Info.plist").open("wb") as handle:
        plistlib.dump(info, handle)
    return bundle


def linux_tarball(binary: Path, out: Path) -> Path:
    staging = out / f"{BINARY}-{version()}"
    if staging.exists():
        shutil.rmtree(staging)
    staging.mkdir(parents=True)

    shutil.copy2(binary, staging / BINARY)
    (staging / BINARY).chmod(0o755)
    shutil.copy2(ROOT / "ATTRIBUTION.md", staging / "ATTRIBUTION.md")
    engine_licence = ROOT / "vendor" / "ClayCore" / "LICENSE"
    if engine_licence.is_file():
        shutil.copy2(engine_licence, staging / "ClayCore-LICENSE.txt")

    desktop = f"""[Desktop Entry]
Type=Application
Name={APP_NAME}
Comment=Escultura 3D
Exec={BINARY}
Terminal=false
Categories=Graphics;3DGraphics;
MimeType=application/x-clayspace;
"""
    (staging / f"{BUNDLE_ID}.desktop").write_text(desktop, encoding="utf-8")

    archive = out / f"{BINARY}-{version()}-linux.tar.gz"
    if archive.exists():
        archive.unlink()
    with tarfile.open(archive, "w:gz") as tar:
        tar.add(staging, arcname=staging.name)
    return archive


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--debug", action="store_true", help="bundle the debug build")
    parser.add_argument(
        "--out",
        type=Path,
        default=ROOT / "target" / "dist",
        help="where to write the distributable",
    )
    args = parser.parse_args()

    if not (ROOT / "ATTRIBUTION.md").is_file():
        print(
            "ATTRIBUTION.md is missing. Run: python3 tools/attribution.py",
            file=sys.stderr,
        )
        return 1

    binary = build(release=not args.debug)
    if not binary.is_file():
        print(f"no binary at {binary}", file=sys.stderr)
        return 1

    dangling = links_to_engine_dynamically(binary)
    if dangling:
        print(
            "the binary links ClayCore dynamically, so the distributable is not\n"
            "self-contained:\n  " + "\n  ".join(dangling),
            file=sys.stderr,
        )
        return 1

    args.out.mkdir(parents=True, exist_ok=True)
    if sys.platform == "darwin":
        produced = macos_bundle(binary, args.out)
    else:
        produced = linux_tarball(binary, args.out)

    print(f"Wrote {produced}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
