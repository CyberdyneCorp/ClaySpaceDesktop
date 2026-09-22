## ADDED Requirements

### Requirement: An idle application does no work proportional to the document
With a document open, no gesture in progress, no command running and no
animation, the application SHALL NOT perform per-frame work whose cost grows
with the size of the document. An application nobody is touching SHALL settle
to a small, flat cost that a worked sculpture does not raise.

A figure that is only displayed — a meter, a counter, a status line — SHALL be
refreshed on a stated interval rather than once a frame where obtaining it
costs more than reading a field. The interval SHALL be named where it is
defined, and SHALL be short enough that a person cannot tell the figure from an
exact one.

This is a budget in its own right and not only a matter of power: work paid on
every idle frame is the noise floor of every other measurement taken of this
application, and a profile whose largest entry is a status bar cannot be used
to find anything else.

#### Scenario: Idling with a worked document is cheap
- **WHEN** a document with a worked layer is left open with no input for a
  minute
- **THEN** the application uses a small fraction of one core, and the figure
  does not rise with the size of the sculpture

#### Scenario: A displayed figure is refreshed on an interval
- **WHEN** a figure shown in the interface costs a walk of an engine structure
  to obtain
- **THEN** it is obtained at most once per stated interval, and the frames in
  between show the figure already obtained

#### Scenario: The interval is short enough to be honest
- **WHEN** the value behind such a figure changes
- **THEN** what the interface shows catches up within the stated interval
