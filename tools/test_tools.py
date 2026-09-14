#!/usr/bin/env python3
"""Tests for the packaging scripts.

Small, because the scripts are small — but the two failures worth catching are
both silent: a version read as the string "true" (which is what `version.
workspace = true` parses to if the inheritance is missed) and an attribution
manifest that no longer matches what is linked.
"""

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


class CiBudgetNoteTests(unittest.TestCase):
    """#83: `ci.yml` derived its budget from figures nobody could re-derive.

    Two replacement notes were then refuted for the same reason, so the fix is
    not a better paragraph — it is `tools/ci_budget.json` beside the file and
    `tools/check_ci_budget.py` holding one to the other.
    """

    CHECKER = ROOT / "tools" / "check_ci_budget.py"
    WORKFLOW = ROOT / ".github" / "workflows" / "ci.yml"

    def run_checker(self, root):
        return subprocess.run(
            [sys.executable, str(Path(root) / "tools" / "check_ci_budget.py")],
            capture_output=True,
            text=True,
        )

    def test_the_note_matches_the_measurements_it_argues_from(self):
        result = self.run_checker(ROOT)
        self.assertEqual(result.returncode, 0, f"{result.stdout}\n{result.stderr}")

    def sandbox(self, tmp, edit):
        """The three files the checker reads, copied out, with ci.yml edited."""
        root = Path(tmp)
        (root / "tools").mkdir()
        (root / ".github" / "workflows").mkdir(parents=True)
        shutil.copy(self.CHECKER, root / "tools" / "check_ci_budget.py")
        shutil.copy(ROOT / "tools" / "ci_budget.json", root / "tools" / "ci_budget.json")
        text = self.WORKFLOW.read_text(encoding="utf-8")
        edited = edit(text)
        self.assertNotEqual(edited, text, "the mutation did not change the file")
        (root / ".github" / "workflows" / "ci.yml").write_text(edited, encoding="utf-8")
        return root

    def assert_rejected(self, edit, expect):
        with tempfile.TemporaryDirectory() as tmp:
            result = self.run_checker(self.sandbox(tmp, edit))
        self.assertEqual(
            result.returncode, 1, f"the checker accepted it:\n{result.stdout}")
        self.assertIn(expect, result.stdout)

    # Each of these is a claim a reviewer actually refuted in a previous
    # revision of the note. They are here so the checker is known to reject
    # them, rather than assumed to.

    def test_it_rejects_a_cold_warm_pair_that_the_row_contradicts(self):
        # "20.3 cold and 20.3 warm, so the engine is invisible in that step" —
        # the same row measured 25.2 with ccache at 0/300 and 20.3 at 300/300.
        self.assert_rejected(
            lambda t: t.replace("0 / 300 hits      4.6                      25.2",
                                "0 / 300 hits      4.6                      20.3"),
            "the note's Build for `0 / 300 hits` is 20.3")

    def test_it_rejects_an_engine_cost_read_off_the_wrong_anchor(self):
        # "2.6 min, and that is all ccache can buy here" — 2.6 is 3.8 - 1.2,
        # and 3.8 is faster than fifteen of the eighteen ccache-less Builds.
        self.assert_rejected(
            lambda t: t.replace("Its Build was 3.5 - 5.9 min (median 4.7)",
                                "Its Build was 3.8 - 3.8 min (median 3.8)"),
            "ccache-less debug Build low is 3.8")

    def test_it_rejects_a_sample_size_that_no_longer_matches_the_table(self):
        self.assert_rejected(
            lambda t: t.replace("twenty-two of which reached\n    # the test step",
                                "twenty-four of which reached\n    # the test step"),
            "the note says twenty-four jobs reached the test step")

    def test_it_rejects_a_distribution_that_was_not_recomputed(self):
        self.assert_rejected(
            lambda t: t.replace("min 43.2    mean 53.8    sd 6.2    max 70.1",
                                "min 43.2    mean 50.1    sd 4.0    max 55.5"),
            "the note says mean 50.1 for the release row")

    def test_it_rejects_a_failed_job_folded_into_the_sample_unnamed(self):
        self.assert_rejected(
            lambda t: t.replace("run 34623657105 failed a single assertion",
                                "one job failed a single assertion"),
            "run 34623657105 is in the sample and went red")

    def test_it_rejects_calling_the_pin_move_an_eviction(self):
        # 95 of 303 missed on run 34783040917, but only ClayCore's objects did:
        # cyberremesh's window stayed at the warm 17 s. An evicted cache would
        # have missed both.
        self.assert_rejected(
            lambda t: t.replace("window stayed at 17 s while claycore's went to 105 s",
                                "window stayed at 99 s while claycore's went to 105 s"),
            "the note says cyberremesh's window was 99 s")

    def test_it_rejects_a_budget_the_notes_own_worst_case_exceeds(self):
        self.assert_rejected(
            lambda t: t.replace("timeout-minutes: 75", "timeout-minutes: 70", 1),
            "is not under the budget it argues for (70 min)")


if __name__ == "__main__":
    unittest.main()
