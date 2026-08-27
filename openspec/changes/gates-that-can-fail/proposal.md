# Record what the gates now hold

## Why

An audit of the workspace found fifty-nine confirmed defects, and the ones that
mattered most were not wrong code — they were **gates that could not fail**.
The performance gate had three independent reasons it could never report a
regression. Five visual assertions could not fail for the reason they named.
Eight unit tests passed while the behaviour they were named for was broken.

Those are fixed. What is not yet true is that the *rules* which now hold are
written anywhere a reader would look. They live in code comments beside the
assertions that enforce them, which is where the last set of rules lived too —
and `the_smoothing_tools_smooth_rather_than_crumble` shows what that is worth:
its comment described a measurement the test had stopped making four releases
earlier, and nothing noticed because nothing else said what it was for.

A property enforced by one assertion and described by one comment is one
careless refactor from being neither.

## What changes

Nothing in the code. This records, as requirements, the properties the fixes
established:

- a stated reason excuses a missing figure only when it is the machine's
  inability or the baseline gave the same reason;
- a gate that declines to compare is not a gate that passed;
- a crash is told apart from a verdict, and quarantine carries an expiry;
- a visual assertion is measured against the render floor of the machine it
  runs on, not against a constant;
- an assertion that a tool did something gentle must first establish the tool
  did anything.

## Impact

- `performance-budgets`: three added requirements about what the gate refuses
  to call a pass.
- `visual-verification`: a new capability, stating how a rendered frame may be
  asserted on. There was no spec for this and there are twenty-five test files
  doing it, which is why the same defect appeared in five of them.
