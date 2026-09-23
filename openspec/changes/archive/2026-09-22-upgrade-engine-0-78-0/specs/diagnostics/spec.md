## ADDED Requirements

### Requirement: The report says what a mesh stroke's seed cost
A stamp handed a seed naming a numbering that has been retired is refused by the
engine and falls back to a scan. That is the correct outcome and it is invisible:
nothing on screen changes, and the only difference is one stamp's cost. The
diagnostics report SHALL carry the count of those refusals, so that a reader can
tell a working fallback from a fallback that has started happening on every
stamp.

The count SHALL be reported **beside the number of mesh sculpting sessions the
document is holding**, and not on its own. Zero refusals over no sessions and
zero refusals over four are the same number and different facts, and a reader
given only the first cannot tell which they are looking at.

Both figures SHALL appear in the report that is copied to the clipboard, not only
in the window, because the report is what a sculptor pastes into an issue.

#### Scenario: The two figures are shown together
- **WHEN** the diagnostics window is opened
- **THEN** the number of held mesh sculpting sessions and the number of refused
  seeds are both shown

#### Scenario: The pasted report carries them
- **WHEN** the diagnostics report is copied
- **THEN** the copied text contains both figures
