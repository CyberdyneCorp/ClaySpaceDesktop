# Fold the completed changes into a living specification, and gate it

## Why

`openspec/specs/` was empty and `openspec/changes/archive/` did not exist, while
**35 changes were complete with every task done and unarchived**. Requirements
therefore lived across dozens of deltas, and several of the deltas contradicted
each other:

| subject | one delta said | another said |
|---|---|---|
| subtool scale | `place-and-transform-objects`: uniform | `a-subtool-stretches-per-axis`: per axis |
| boolean operands | `make-representations-first-class`: a mesh is never one | `subtools`: every representation is one |
| second subtool preview | `live-field-brushes`: falls back | `a-preview-that-holds-the-whole-scene`: still drawn |

The audit that found these could not say, in several places, whether a
difference between the application and a spec was a defect or a deliberate
change — because two specs disagreed about what the application was supposed to
do. That is #197, and it is the reason every other issue in that epic is harder
to verify than it should be.

`openspec validate --all --strict` was already a CI gate and it caught none of
this. It reads one spec or one change at a time, so a rule written down twice in
two capabilities is two valid documents.

## What Changes

- **A living specification.** The 35 complete changes are folded into
  `openspec/specs/` and moved to `openspec/changes/archive/`, in the order they
  were made, so each requirement's current text is the last one written.
- **A second gate**, `tools/check_specs.py`, run from `just spec` and from the
  OpenSpec job. It reads the living specification whole and fails on:
  - a requirement heading standing in two capabilities, whether the two texts
    differ (nobody can say which the application obeys) or agree (the two will
    drift, which is how the three pairs above happened);
  - a capability still carrying the placeholder Purpose `openspec archive`
    writes, which names a change rather than saying what the capability covers.
- **Every capability's Purpose written.** Seventeen specs the archive created
  carried the placeholder.

## Why two changes had to be archived that are not complete

`add-clayspace-desktop` (107/109) and `render-quality-and-performance` (45/48)
are the base that twenty-one of the 35 complete changes MODIFY. A delta cannot
modify a requirement that is not in the living spec, so folding the 35 without
them is not a thing OpenSpec can do — the archive refuses it, naming the
requirement. Their outstanding tasks are an open design question, a re-recorded
baseline and three deferred GPU optimisations; none of them is behaviour the
35 assume. This is recorded here rather than left as a surprise in the archive
listing.
