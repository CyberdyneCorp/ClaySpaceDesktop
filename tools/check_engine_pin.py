#!/usr/bin/env python3
"""Asserts every vendored engine is pinned to a release, not to a commit.

The submodules are pinned deliberately to tags. `just engine-pin` takes a tag
and says why: "a release is a thing that stays still, and their main is where
they are still working."

Nothing stopped that pin moving by accident, and one workflow moves it
routinely. `just engine-main` checks ClayCore out at their main so a fix can be
tried before it ships, which leaves the submodule's working copy ahead of the
gitlink on purpose — `claycore-sys/build.rs` warns and carries on for exactly
that case. But `git commit -a` stages a modified submodule pointer along with
everything else, so an investigation that was meant to last an afternoon
becomes a committed pin. CI checks out `submodules: recursive` from the
gitlink, so it would then build that commit and could go green on it, and the
engine this application ships against would have changed without anyone
deciding to change it.

This is the check that makes that impossible to do quietly. `just
engine-restore` is the fix it points at.

Run directly, or as part of `just check`:

    python3 tools/check_engine_pin.py
"""

from __future__ import annotations

import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

SUBMODULES = ("vendor/ClayCore", "vendor/CyberRemesherAndUV")


def git(cwd: Path, *args: str) -> str | None:
    """Stdout of a git command, or None if it failed for any reason.

    Every git failure here is a reason to say nothing rather than to fail: a
    source tree with no git — a vendored tarball, a package build — has no pin
    to be wrong about. The same rule `build.rs` follows.
    """
    try:
        done = subprocess.run(
            ["git", "-C", str(cwd), *args],
            capture_output=True,
            text=True,
            check=False,
        )
    except OSError:
        return None
    return done.stdout.strip() if done.returncode == 0 else None


def check(path: str) -> tuple[bool, str]:
    """(ok, message) for one submodule."""
    # The INDEX, not HEAD. The mistake this catches is a submodule pointer
    # swept into a commit by `git commit -a`, and reading HEAD would only
    # notice once that commit exists — after the thing worth preventing has
    # happened. `:path` is what is about to be committed, and equals HEAD when
    # nothing is staged, so the same check serves both.
    pinned = git(ROOT, "rev-parse", f":{path}")
    if pinned is None:
        pinned = git(ROOT, "rev-parse", f"HEAD:{path}")
    if pinned is None:
        return True, f"{path}: no gitlink to check (not a git checkout)"

    checkout = ROOT / path
    if git(checkout, "rev-parse", "--git-dir") is None:
        return True, f"{path}: pinned {pinned[:10]}, not checked out — cannot judge"

    # Whether the pin is a tag can only be answered where tags exist. A
    # submodule fetched without them — which is what a shallow CI checkout
    # gives — would fail this for the wrong reason, so it is asked first and
    # answered honestly.
    if not git(checkout, "tag", "--list"):
        return True, f"{path}: pinned {pinned[:10]}, no tags fetched — cannot judge"

    tag = git(checkout, "describe", "--tags", "--exact-match", pinned)
    if tag is None:
        return False, (
            f"{path} is pinned to {pinned[:10]}, which is not a release tag.\n"
            f"      If you were trying a fix against their main, the pin was "
            f"committed by accident.\n"
            f"      Put it back with `just engine-restore`, then commit "
            f"{path} on its own."
        )
    return True, f"{path}: {tag}"


def main() -> int:
    results = [check(path) for path in SUBMODULES]
    failures = [message for ok, message in results if not ok]

    if failures:
        print("Engine pin check failed:\n", file=sys.stderr)
        for failure in failures:
            print(f"  - {failure}", file=sys.stderr)
        print(
            "\nThe engine this application ships against is a decision, not a "
            "side effect of a commit.",
            file=sys.stderr,
        )
        return 1

    print("Engine pin check passed:")
    for _, message in results:
        print(f"  {message}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
