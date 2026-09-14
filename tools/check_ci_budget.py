#!/usr/bin/env python3
"""Hold the budget note in `.github/workflows/ci.yml` to its own measurements.

Issue #83 was not that the `test` budget was wrong. It was that the note
deriving it quoted figures nobody could re-derive, so a sentence that had
stopped being true went on reading like a measurement. Two revisions of the
replacement note were refuted the same way — a cross-commit pair of Build
totals presented as a cold/warm control — and neither could be caught by
anything in the repository, because nothing in the repository read the file.

This reads it. `tools/ci_budget.json` is the frozen Actions-API pull behind the
note; every figure the note states in the fixed shapes below is recomputed from
that table and compared. A number edited in the note without the table, or a
table re-pulled without the note, fails here.

It deliberately checks only the figures, not the prose. Prose that argues from
numbers that are checked is a much smaller thing to get wrong than prose that
argues from numbers nobody reads.

    python3 tools/check_ci_budget.py
"""

import json
import re
import statistics
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
WORKFLOW = ROOT / ".github" / "workflows" / "ci.yml"
TABLE = ROOT / "tools" / "ci_budget.json"

# The note writes minutes to one decimal and windows to two significant
# figures, so a comparison has to allow the rounding it was written with.
TOL = 0.06


def load():
    table = json.loads(TABLE.read_text(encoding="utf-8"))
    return WORKFLOW.read_text(encoding="utf-8"), table["jobs"]


def prose(text):
    """The note with its comment markers and line wrapping taken out.

    Sentences in the note wrap wherever the column ran out, so a claim has to
    be matched against the flattened text and not against the file's lines.
    The little tables are matched against the raw text instead, where their
    columns still mean something.
    """
    out = []
    for line in text.splitlines():
        stripped = line.strip()
        if stripped.startswith("#"):
            out.append(stripped.lstrip("#").strip())
    return re.sub(r"\s+", " ", " ".join(out))


def release(jobs):
    """The jobs the budget gates: `macOS, Metal (release)` that reached Test."""
    return [j for j in jobs if j["profile"] == "release" and j["test_s"] is not None]


def no_ccache(jobs, profile):
    return [j for j in jobs if j["profile"] == profile and j["ccache_hits"] is None
            and j["test_s"] is not None]


def warm(jobs, profile):
    return [j for j in jobs if j["profile"] == profile and j["ccache_hits"]
            and j["ccache_hits"][0] == j["ccache_hits"][1]]


def by_run(jobs, profile, run):
    for j in jobs:
        if j["profile"] == profile and j["run"] == run:
            return j
    return None


def close(a, b):
    return abs(a - b) <= TOL


# Every check takes (text, flat, jobs, fail): the file as written, the same
# with its comment markers and wrapping removed, the frozen table, and somewhere
# to put a disagreement. Uniform, so `main` reads as the list of claims checked.


def check_distribution(text, flat, jobs, fail):
    """`#     min 43.2    mean 53.8    sd 6.2    max 70.1`"""
    m = re.search(
        r"#\s+min\s+([\d.]+)\s+mean\s+([\d.]+)\s+sd\s+([\d.]+)\s+max\s+([\d.]+)", text)
    if not m:
        fail("the note no longer states the release-row distribution as "
             "`min A mean B sd C max D`; nothing else can be checked against it")
        return
    stated = [float(g) for g in m.groups()]
    d = sorted(j["job_s"] / 60 for j in release(jobs))
    actual = [min(d), statistics.mean(d), statistics.stdev(d), max(d)]
    for name, s, a in zip(("min", "mean", "sd", "max"), stated, actual):
        if not close(s, a):
            fail("the note says %s %.1f for the release row; the table's %d jobs "
                 "give %.2f" % (name, s, len(d), a))

    # The count the note states in words, and the budget it is compared against.
    words = {"twenty-one": 21, "twenty-two": 22, "twenty-three": 23,
             "twenty-four": 24, "twenty-five": 25}
    m = re.search(r"(twenty-\w+) of which reached the test step", flat)
    if not m:
        fail("the note no longer says how many jobs reached the test step")
    elif words.get(m.group(1)) != len(d):
        fail("the note says %s jobs reached the test step; the table has %d"
             % (m.group(1), len(d)))

    m = re.search(r"^\s*timeout-minutes:\s*(\d+)", text, re.M)
    if not m:
        fail("no `timeout-minutes:` on the test job")
    elif max(d) >= int(m.group(1)):
        fail("the note's worst job (%.1f min) is not under the budget it argues "
             "for (%s min); the argument no longer holds" % (max(d), m.group(1)))


def check_engine_window(text, flat, jobs, fail):
    """The `no step (N jobs)` row of the release-row engine table.

        #     ccache              engine window            Build
        #     no step (18 jobs)   3.3 - 6.0, median 4.7    20.3 - 35.0

    This is the row that carries the claim: with no ccache at all the engines
    took minutes, and with a warm one they take seconds. The three rows under
    it are checked by `check_engine_window_rows`.
    """
    m = re.search(r"#\s+no step \((\d+) jobs\)\s+([\d.]+) - ([\d.]+), median ([\d.]+)"
                  r"\s+([\d.]+) - ([\d.]+)", text)
    if not m:
        fail("the note no longer carries the `no step (N jobs)` row of the "
             "engine-window table")
        return
    pre = no_ccache(jobs, "release")
    n, lo, hi, med, blo, bhi = (int(m.group(1)),) + tuple(
        float(g) for g in m.groups()[1:])
    w = sorted(j["engine_window_s"] / 60 for j in pre)
    b = sorted(j["build_s"] / 60 for j in pre)
    if n != len(pre):
        fail("the note counts %d ccache-less release jobs; the table has %d"
             % (n, len(pre)))
    for name, s, a in (("window low", lo, min(w)), ("window high", hi, max(w)),
                       ("window median", med, statistics.median(w)),
                       ("Build low", blo, min(b)), ("Build high", bhi, max(b))):
        if not close(s, a):
            fail("the note's ccache-less %s is %.2f; the table gives %.2f"
                 % (name, s, a))


def check_engine_window_rows(text, flat, jobs, fail):
    """The three ccache-era rows of the same table.

        #       0 / 300 hits      4.6                      25.2
        #     300 / 300 hits      0.33, 0.36               23.3, 20.3
        #     208 / 303 hits      1.7                      23.9

    Windows ascending, Builds descending, one figure per job the table holds at
    that hit count — so a row cannot quietly drop the job that disagrees.
    """
    rel = [j for j in jobs if j["profile"] == "release"]
    for hits, calls in ((0, 300), (300, 300), (208, 303)):
        rows = [j for j in rel if j["ccache_hits"] == [hits, calls]]
        if not rows:
            fail("the note's `%d / %d hits` row has no job in the table"
                 % (hits, calls))
            continue
        m = re.search(r"#\s+%d / %d hits\s+([\d.,\s]+?)\s\s+([\d.,\s]+)$"
                      % (hits, calls), text, re.M)
        if not m:
            fail("the note no longer carries the `%d / %d hits` row" % (hits, calls))
            continue
        stated_w = [float(v) for v in m.group(1).split(",")]
        stated_b = [float(v) for v in m.group(2).split(",")]
        actual_w = sorted(j["engine_window_s"] / 60 for j in rows)
        actual_b = sorted((j["build_s"] / 60 for j in rows), reverse=True)
        if len(stated_w) != len(rows) or len(stated_b) != len(rows):
            fail("the note lists %d window / %d Build figures for `%d / %d hits`; "
                 "the table has %d such jobs"
                 % (len(stated_w), len(stated_b), hits, calls, len(rows)))
            continue
        for s, a in zip(sorted(stated_w), actual_w):
            if not close(s, a):
                fail("the note's engine window for `%d / %d hits` is %.2f; the "
                     "table gives %.2f" % (hits, calls, s, a))
        for s, a in zip(stated_b, actual_b):
            if not close(s, a):
                fail("the note's Build for `%d / %d hits` is %.1f; the table gives "
                     "%.2f" % (hits, calls, s, a))


def check_debug_ladder(text, flat, jobs, fail):
    """`#     34669732006     300 / 300     1.2`, and the range above it."""
    rows = re.findall(r"#\s+(\d{11})\s+(\d+) / (\d+)\s+([\d.]+)", text)
    if len(rows) < 4:
        fail("the note no longer carries the four-row debug ccache ladder")
        return
    seen = []
    for run, hits, calls, build in rows:
        j = by_run(jobs, "debug", int(run))
        if j is None:
            fail("the debug ladder cites run %s, which is not in the table" % run)
            continue
        if j["ccache_hits"] != [int(hits), int(calls)]:
            fail("the ladder says run %s logged %s / %s hits; the table says %s"
                 % (run, hits, calls, j["ccache_hits"]))
        if not close(float(build), j["build_s"] / 60):
            fail("the ladder says run %s built in %s min; the table says %.2f"
                 % (run, build, j["build_s"] / 60))
        seen.append(j["build_s"])
    if seen != sorted(seen):
        fail("the ladder is no longer monotone in the miss count, which is the "
             "only thing it is there to show")

    m = re.search(r"Build was ([\d.]+) - ([\d.]+) min \(median ([\d.]+)\) over the "
                  r"(\w+) runs with no ccache step and is ([\d.]+) - ([\d.]+)", flat)
    if not m:
        fail("the note no longer states the ccache-less debug Build range")
        return
    words = {"sixteen": 16, "seventeen": 17, "eighteen": 18, "nineteen": 19,
             "twenty": 20}
    pre = sorted(j["build_s"] / 60 for j in no_ccache(jobs, "debug"))
    hot = sorted(j["build_s"] / 60 for j in jobs
                 if j["profile"] == "debug" and j["ccache_hits"]
                 and j["ccache_hits"][0] >= 275)
    if words.get(m.group(4)) != len(pre):
        fail("the note says %s ccache-less debug runs; the table has %d"
             % (m.group(4), len(pre)))
    for name, s, a in (("low", float(m.group(1)), min(pre)),
                       ("high", float(m.group(2)), max(pre)),
                       ("median", float(m.group(3)), statistics.median(pre)),
                       ("warm low", float(m.group(5)), min(hot)),
                       ("warm high", float(m.group(6)), max(hot))):
        if not close(s, a):
            fail("the note's ccache-less debug Build %s is %.1f; the table gives "
                 "%.2f" % (name, s, a))


def check_pin_move_was_not_an_eviction(text, flat, jobs, fail):
    """The note claims #126's 95 misses were ClayCore's objects and not an
    evicted cache. That is only true while cyberremesh's window stayed warm."""
    m = re.search(r"95 of its (\d+) calls missed, but its cyberremesh window stayed "
                  r"at (\d+) s while claycore's went to (\d+) s", flat)
    if not m:
        return  # the claim is gone; nothing to hold it to
    j = by_run(jobs, "release", 34783040917)
    if j["ccache_hits"][1] - j["ccache_hits"][0] != 95:
        fail("the note says 95 of run 34783040917's calls missed; the table says %d"
             % (j["ccache_hits"][1] - j["ccache_hits"][0]))
    if int(m.group(1)) != j["ccache_hits"][1]:
        fail("the note says run 34783040917 made %s cacheable calls; the table "
             "says %d" % (m.group(1), j["ccache_hits"][1]))
    if abs(int(m.group(2)) - j["cyberremesh_window_s"]) > 1:
        fail("the note says cyberremesh's window was %s s on run 34783040917; the "
             "table says %.1f" % (m.group(2), j["cyberremesh_window_s"]))
    if abs(int(m.group(3)) - j["claycore_window_s"]) > 1:
        fail("the note says claycore's window was %s s on run 34783040917; the "
             "table says %.1f" % (m.group(3), j["claycore_window_s"]))
    hot = [x["cyberremesh_window_s"] for x in warm(jobs, "release")]
    if j["cyberremesh_window_s"] > max(hot) * 2:
        fail("run 34783040917's cyberremesh window (%.1f s) is not at the warm "
             "level (%s s), so calling its misses a pin move rather than an "
             "eviction is no longer supported"
             % (j["cyberremesh_window_s"], hot))


def check_the_red_job_is_named(text, flat, jobs, fail):
    """A failed job sits in the distribution. The note has to say which."""
    red = [j for j in release(jobs) if j["conclusion"] != "success"]
    for j in red:
        if str(j["run"]) not in flat:
            fail("run %d is in the sample and went red, and the note does not name "
                 "it" % j["run"])
            continue
        m = re.search(r"run %d.{0,400}?all (\d+) suites" % j["run"], flat)
        if not m:
            fail("the note names run %d but no longer says it still ran every "
                 "suite, which is why it stays in the sample" % j["run"])
        elif int(m.group(1)) != j["suites"]:
            fail("the note says run %d ran %s suites; the table says %d"
                 % (j["run"], m.group(1), j["suites"]))


def main():
    text, jobs = load()
    flat = prose(text)
    problems = []
    fail = problems.append
    check_distribution(text, flat, jobs, fail)
    check_engine_window(text, flat, jobs, fail)
    check_engine_window_rows(text, flat, jobs, fail)
    check_debug_ladder(text, flat, jobs, fail)
    check_pin_move_was_not_an_eviction(text, flat, jobs, fail)
    check_the_red_job_is_named(text, flat, jobs, fail)
    if problems:
        print("ci.yml's budget note disagrees with tools/ci_budget.json:\n")
        for p in problems:
            print("  - " + p)
        print("\nEither the note is wrong, or the table has been re-pulled and the "
              "note has not\nbeen rewritten against it. Both are the defect #83 "
              "was filed about.")
        return 1
    print("ci.yml's budget note matches tools/ci_budget.json (%d jobs)." % len(jobs))
    return 0


if __name__ == "__main__":
    sys.exit(main())
