#!/usr/bin/env python3
"""Tests for the packaging scripts and the specification check.

Small, because the scripts are small — but the failures worth catching are all
silent: a version read as the string "true" (which is what `version.workspace =
true` parses to if the inheritance is missed), an attribution manifest that no
longer matches what is linked, and a requirement written down in two
capabilities whose two texts have drifted apart.
"""

import contextlib
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "tools"))

import attribution  # noqa: E402
import bundle  # noqa: E402
import check_specs  # noqa: E402


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


class SpecificationTests(unittest.TestCase):
    """The living specification, and the check that keeps it saying each thing once.

    The regression these hold is #197: 35 completed changes sat unarchived, so
    requirements lived across dozens of deltas and several of them contradicted
    each other. An audit could not say whether a difference between the
    application and a spec was a defect or a deliberate change, because two
    specs disagreed about what the application was supposed to do.
    """

    def run_check(self, root: Path) -> subprocess.CompletedProcess:
        script = root / "tools" / "check_specs.py"
        return subprocess.run(
            [sys.executable, str(script)], capture_output=True, text=True
        )

    def sandbox(self, stack) -> Path:
        """A copy of the specs and the script, for a test that has to break one."""
        temporary = Path(tempfile.mkdtemp())
        stack.callback(shutil.rmtree, temporary, True)
        (temporary / "tools").mkdir()
        shutil.copy2(ROOT / "tools" / "check_specs.py", temporary / "tools")
        shutil.copytree(ROOT / "openspec" / "specs", temporary / "openspec" / "specs")
        return temporary

    def test_the_tree_passes_its_own_check(self):
        result = self.run_check(ROOT)
        self.assertEqual(result.returncode, 0, f"{result.stdout}\n{result.stderr}")

    def test_a_living_spec_exists_for_every_capability(self):
        specs = sorted(p.parent.name for p in check_specs.SPECS.glob("*/spec.md"))
        self.assertGreater(len(specs), 20, "the living specification is thin")
        for expected in ("sculpting-tools", "scene-and-layers", "viewport-rendering"):
            self.assertIn(expected, specs)

    def test_the_completed_changes_are_archived(self):
        archive = ROOT / "openspec" / "changes" / "archive"
        self.assertTrue(archive.is_dir(), "nothing has been archived")
        self.assertGreater(
            len(list(archive.glob("*/proposal.md"))),
            30,
            "the completed changes belong in the archive, not beside the "
            "proposals still being worked on",
        )

    def test_the_same_requirement_in_two_capabilities_fails(self):
        with contextlib.ExitStack() as stack:
            root = self.sandbox(stack)
            specs = root / "openspec" / "specs"
            borrowed = (specs / "scene-and-layers" / "spec.md").read_text(
                encoding="utf-8"
            )
            name = "### Requirement: Layer protection states are distinct and enforced"
            self.assertIn(name, borrowed)
            target = specs / "sculpting-tools" / "spec.md"
            target.write_text(
                target.read_text(encoding="utf-8")
                + f"\n{name}\nSomething else entirely.\n\n"
                "#### Scenario: A second telling\n- **WHEN** it is read\n"
                "- **THEN** it disagrees with the first\n",
                encoding="utf-8",
            )
            result = self.run_check(root)
            self.assertEqual(result.returncode, 1)
            self.assertIn("the two texts differ", result.stderr)

    def test_a_placeholder_purpose_fails(self):
        with contextlib.ExitStack() as stack:
            root = self.sandbox(stack)
            target = root / "openspec" / "specs" / "document-io" / "spec.md"
            text = target.read_text(encoding="utf-8")
            body = text.split("## Requirements", 1)[1]
            target.write_text(
                "# document-io Specification\n\n## Purpose\n"
                f"{check_specs.PLACEHOLDER} add-clayspace-desktop. "
                "Update Purpose after archive.\n## Requirements" + body,
                encoding="utf-8",
            )
            result = self.run_check(root)
            self.assertEqual(result.returncode, 1)
            self.assertIn("placeholder", result.stderr)


if __name__ == "__main__":
    unittest.main()
