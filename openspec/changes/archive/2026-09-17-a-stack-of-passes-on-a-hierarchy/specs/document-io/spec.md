## ADDED Requirements

### Requirement: A save that could not be completed says so
When a save fails, the application SHALL NOT mark the document as saved, SHALL
NOT adopt the path it could not write, and SHALL show the reason on the line
beside the viewport that carries the reasons other refusals arrive on.

This matters most for a hierarchy: its sculpt is written beside the document
and a save that cannot write it fails, so a failed save that says nothing is
the one refusal in this application that costs work rather than a click.

#### Scenario: A sculpt that could not be written beside its document
- **WHEN** a save fails because the hierarchy's sculpt could not be written
- **THEN** the document is still shown as unsaved and the reason is drawn
  beside the viewport

#### Scenario: The next save that works clears it
- **WHEN** a later save succeeds
- **THEN** the reason is gone
