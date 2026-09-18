#!/usr/bin/env python3
"""Asserts that the living specification says each thing once.

`openspec validate` reads one spec or one change at a time, so it catches a
malformed delta and cannot catch the failure that actually cost this repository
an audit: one requirement written down in two capabilities, the two texts
drifting apart, and nothing to say which of them the application obeys. Before
the archive pass of #197 there were three such pairs in flight at once — a
subtool's scale uniform in one delta and per axis in another, a mesh refused as a
boolean operand in one and accepted in another, a second field subtool falling
back in one and still drawn in another — and each one turned a question about
behaviour into a question about which document was newer.

This is the grep-level check that catches the obvious cases:

- a requirement heading standing in two capabilities, with different text;
- a capability still carrying the placeholder Purpose that `openspec archive`
  writes when it creates a spec, which means nobody said what the capability is
  for after folding a change into it.

Run directly, or as part of CI:

    python3 tools/check_specs.py
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SPECS = ROOT / "openspec" / "specs"

REQUIREMENT = re.compile(r"^###\s+Requirement:\s*(.+?)\s*$")
SECTION = re.compile(r"^(#|##)\s+")

# What `openspec archive` writes into a spec it had to create, for a human to
# replace. A spec that still carries it describes nothing.
PLACEHOLDER = "TBD - created by archiving change"

# A requirement two capabilities may both state, with the reason each is allowed
# to. Empty today, and kept so that an intentional overlap is recorded here
# rather than argued about in review.
SHARED: dict[str, str] = {}


def normalise(body: list[str]) -> str:
    """The text of a requirement, with wrapping and blank lines taken out.

    Two capabilities stating the same rule rarely wrap it the same way, and a
    difference in line breaks is not a difference in what the rule says.
    """
    return " ".join(" ".join(body).split())


def requirements(path: Path) -> dict[str, str]:
    """Every requirement in one spec, as name to normalised text."""
    found: dict[str, list[str]] = {}
    current: str | None = None
    for line in path.read_text(encoding="utf-8").splitlines():
        heading = REQUIREMENT.match(line)
        if heading:
            current = heading.group(1)
            found[current] = []
        elif current is not None and SECTION.match(line):
            current = None
        elif current is not None:
            found[current].append(line)
    return {name: normalise(body) for name, body in found.items()}


def check_duplicates(failures: list[str]) -> int:
    """Fails where one requirement name stands in two capabilities."""
    seen: dict[str, tuple[str, str]] = {}
    counted = 0
    for path in sorted(SPECS.glob("*/spec.md")):
        capability = path.parent.name
        for name, text in requirements(path).items():
            counted += 1
            if name in SHARED:
                continue
            first = seen.get(name)
            if first is None:
                seen[name] = (capability, text)
                continue
            where, other = first
            if other == text:
                failures.append(
                    f'"{name}" is written out in both {where} and {capability}. '
                    "One of the two owns it; the other should name it instead."
                )
            else:
                failures.append(
                    f'"{name}" stands in both {where} and {capability}, and the '
                    "two texts differ — so neither says what the application "
                    "does. Decide which is true, and leave one."
                )
    return counted


def check_purposes(failures: list[str]) -> None:
    """Fails where a spec still carries the placeholder the archive wrote."""
    for path in sorted(SPECS.glob("*/spec.md")):
        if PLACEHOLDER in path.read_text(encoding="utf-8"):
            failures.append(
                f"{path.parent.name} still carries the archive's placeholder "
                "Purpose. Say what the capability covers, or the spec names a "
                "change nobody will look up."
            )


def main() -> int:
    if not SPECS.is_dir():
        print(
            f"No living specification at {SPECS.relative_to(ROOT)}. "
            "Archive the completed changes before running this.",
            file=sys.stderr,
        )
        return 1

    failures: list[str] = []
    counted = check_duplicates(failures)
    check_purposes(failures)

    if failures:
        print("Specification check failed:\n", file=sys.stderr)
        for failure in failures:
            print(f"  - {failure}", file=sys.stderr)
        return 1

    capabilities = len(list(SPECS.glob("*/spec.md")))
    print("Specification check passed:")
    print(f"  {counted} requirements across {capabilities} capabilities")
    print("  no requirement stated in two of them, no placeholder Purpose left")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
