#!/usr/bin/env python3
"""Tests for the packaging scripts.

Small, because the scripts are small — but the two failures worth catching are
both silent: a version read as the string "true" (which is what `version.
workspace = true` parses to if the inheritance is missed) and an attribution
manifest that no longer matches what is linked.
"""

import subprocess
import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "tools"))

import attribution  # noqa: E402
import bundle  # noqa: E402


class VersionTests(unittest.TestCase):
    def test_workspace_inheritance_is_resolved(self):
        # `version.workspace = true` must find the workspace's number, not put
        # the word "true" in an Info.plist.
        found = bundle.version()
        self.assertNotIn(found, ("true", "false"))
        self.assertRegex(found, r"^\d+\.\d+\.\d+")


class AttributionTests(unittest.TestCase):
    def test_the_manifest_is_current(self):
        result = subprocess.run(
            [sys.executable, str(ROOT / "tools" / "attribution.py"), "--check"],
            capture_output=True,
            text=True,
        )
        self.assertEqual(
            result.returncode,
            0,
            f"{result.stdout}\n{result.stderr}",
        )

    def test_the_manifest_names_the_engine_and_its_licence(self):
        text = (ROOT / "ATTRIBUTION.md").read_text(encoding="utf-8")
        self.assertIn("ClayCore", text)
        self.assertIn("vendor/ClayCore/LICENSE", text)

    def test_every_dependency_row_carries_a_licence(self):
        text = (ROOT / "ATTRIBUTION.md").read_text(encoding="utf-8")
        rows = [
            line
            for line in text.splitlines()
            if line.startswith("| ") and not line.startswith("| Package")
        ]
        self.assertGreater(len(rows), 50, "the manifest is suspiciously short")
        for row in rows:
            cells = [cell.strip() for cell in row.strip("|").split("|")]
            self.assertEqual(len(cells), 3, row)
            self.assertTrue(all(cells), f"a blank cell in: {row}")

    def test_the_workspaces_own_crates_are_not_listed_as_third_party(self):
        text = (ROOT / "ATTRIBUTION.md").read_text(encoding="utf-8")
        for ours in ("| clayspace-app ", "| clayspace-model ", "| claycore-sys "):
            self.assertNotIn(ours, text, f"{ours.strip()} is not a third party")


if __name__ == "__main__":
    unittest.main()
