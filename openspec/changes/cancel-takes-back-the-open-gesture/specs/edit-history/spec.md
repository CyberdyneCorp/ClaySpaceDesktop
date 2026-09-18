## ADDED Requirements

### Requirement: Cancelling a gesture spends only what that gesture wrote
Cancelling the gesture in progress SHALL take the document back to where it
stood when the gesture opened, and SHALL NOT step past that point. How far back
that is SHALL be measured from the document rather than counted from the
segments the gesture was sent in: a representation may record an entry per
segment or bank the whole gesture as one record, and a cancel that assumed the
first destroyed work on the second.

Nothing a cancel takes back SHALL be offered as something to put forward again:
a cancelled gesture is not an action the user can ask for back.

#### Scenario: A cancel reaches no further than its own gesture
- **WHEN** the user commits gestures, begins another and cancels it
- **THEN** the history offers exactly the committed gestures to undo, and every
  one of them is still on the document

#### Scenario: A cancel is not something to redo
- **WHEN** the user cancels a gesture
- **THEN** the history offers nothing more to put forward than it did before the
  gesture began

### Requirement: A cancel with no gesture open changes nothing and says so
A release arrives whether or not the press that should have preceded it opened
anything, so a cancel with nothing open SHALL NOT be a refusal — a caller may
safely repeat one. It SHALL change nothing: no history is stepped, and the
model is not told that a gesture ended.

It SHALL be reported as a cancel that had nothing to cancel, rather than
leaving the last edit that did happen standing as what the application last
did.

#### Scenario: Cancelling with nothing open costs nothing
- **WHEN** the user cancels with no gesture open, twice
- **THEN** the document is untouched, the history offers exactly what it did
  before, and the application reports a cancel that changed nothing
