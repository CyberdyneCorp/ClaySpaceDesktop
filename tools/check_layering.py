#!/usr/bin/env python3
"""Asserts the layering rules the architecture depends on.

MVVM here is not a convention people are asked to remember; it is a set of
Cargo dependency facts. A View that cannot reach the engine cannot accidentally
call it, however convenient that would be in the moment.

Run directly, or as part of CI:

    python3 tools/check_layering.py
"""

from __future__ import annotations

import json
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

# (crate, forbidden dependency, why it matters)
FORBIDDEN: list[tuple[str, str, str]] = [
    (
        "clayspace-view",
        "claycore",
        "the View layer must not reach the engine; it reads ViewModel state and "
        "emits commands",
    ),
    (
        "clayspace-view",
        "claycore-sys",
        "the View layer must not reach the engine's FFI",
    ),
    (
        "clayspace-vm",
        "egui",
        "ViewModels must be testable with no interface library",
    ),
    (
        "clayspace-vm",
        "wgpu",
        "ViewModels must be testable with no GPU",
    ),
    (
        "clayspace-vm",
        "winit",
        "ViewModels must be testable with no window",
    ),
    (
        "clayspace-vm",
        "claycore",
        "ViewModels talk to the Model through its trait, not to the engine",
    ),
    (
        "clayspace-model",
        "egui",
        "the Model layer is domain logic and knows nothing of the interface",
    ),
    (
        "clayspace-model",
        "wgpu",
        "the Model layer is domain logic and knows nothing of rendering",
    ),
]

# Only these crates may contain `unsafe`. Everything else declares
# `#![forbid(unsafe_code)]`, which the compiler enforces; this catches a crate
# that quietly drops the declaration.
UNSAFE_ALLOWED = {"claycore-sys", "claycore"}


def workspace_metadata() -> dict:
    out = subprocess.run(
        ["cargo", "metadata", "--format-version", "1", "--no-deps"],
        cwd=ROOT,
        capture_output=True,
        text=True,
        check=True,
    )
    return json.loads(out.stdout)


def resolved_metadata() -> dict:
    """Metadata including transitive dependencies."""
    out = subprocess.run(
        ["cargo", "metadata", "--format-version", "1"],
        cwd=ROOT,
        capture_output=True,
        text=True,
        check=True,
    )
    return json.loads(out.stdout)


def transitive_dependencies(meta: dict, crate: str) -> set[str]:
    """Every crate reachable from `crate`, dev-dependencies excluded.

    Transitive matters: a View that depends on something that depends on the
    engine has still reached the engine.
    """
    nodes = {node["id"]: node for node in meta["resolve"]["nodes"]}
    names = {pkg["id"]: pkg["name"] for pkg in meta["packages"]}

    start = next((pid for pid, name in names.items() if name == crate), None)
    if start is None:
        raise SystemExit(f"crate {crate} is not in the workspace")

    seen: set[str] = set()
    stack = [start]
    while stack:
        current = stack.pop()
        for dep in nodes[current]["deps"]:
            # A dev-dependency is a test's business, not the crate's shipped
            # dependency graph.
            if all(kind.get("kind") == "dev" for kind in dep["dep_kinds"]):
                continue
            if dep["pkg"] in seen:
                continue
            seen.add(dep["pkg"])
            stack.append(dep["pkg"])
    return {names[pid] for pid in seen}


def check_dependencies(failures: list[str]) -> None:
    meta = resolved_metadata()
    for crate, forbidden, why in FORBIDDEN:
        reachable = transitive_dependencies(meta, crate)
        if forbidden in reachable:
            failures.append(
                f"{crate} depends on {forbidden} (directly or transitively) — {why}"
            )


def check_unsafe(failures: list[str]) -> None:
    meta = workspace_metadata()
    for package in meta["packages"]:
        name = package["name"]
        if name in UNSAFE_ALLOWED:
            continue
        src = Path(package["manifest_path"]).parent / "src"
        if not src.is_dir():
            continue

        entry_points = [p for p in (src / "lib.rs", src / "main.rs") if p.is_file()]
        for entry in entry_points:
            text = entry.read_text(encoding="utf-8")
            if "#![forbid(unsafe_code)]" not in text:
                failures.append(
                    f"{name}/{entry.name} does not declare #![forbid(unsafe_code)]; "
                    f"only {', '.join(sorted(UNSAFE_ALLOWED))} may contain unsafe"
                )

        for path in src.rglob("*.rs"):
            text = path.read_text(encoding="utf-8")
            # `unsafe impl` and `unsafe {` both count; a comment mentioning the
            # word does not.
            stripped = re.sub(r"//.*", "", text)
            stripped = re.sub(r"/\*.*?\*/", "", stripped, flags=re.S)
            if re.search(r"\bunsafe\s*[{(a-zA-Z]", stripped):
                relative = path.relative_to(ROOT)
                failures.append(f"{relative} contains unsafe, which only the bridge may")


def main() -> int:
    failures: list[str] = []
    check_dependencies(failures)
    check_unsafe(failures)

    if failures:
        print("Layering check failed:\n", file=sys.stderr)
        for failure in failures:
            print(f"  - {failure}", file=sys.stderr)
        print(
            "\nThese are the rules that make the architecture checkable rather "
            "than aspirational.",
            file=sys.stderr,
        )
        return 1

    print("Layering check passed:")
    print(f"  {len(FORBIDDEN)} forbidden dependency edges absent")
    print(f"  unsafe confined to {', '.join(sorted(UNSAFE_ALLOWED))}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
