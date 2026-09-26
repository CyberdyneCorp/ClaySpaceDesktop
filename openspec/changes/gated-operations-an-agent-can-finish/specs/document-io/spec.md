## ADDED Requirements

### Requirement: The recovery offer does not hold the application
The offer to recover a previous session's unsaved work SHALL be a window drawn
in the application, not a native alert, and SHALL NOT block the interface
thread while it stands. It SHALL stay up until it is answered.

While the offer stands no autosave SHALL be written over the file on offer, and
a clean exit SHALL leave it for the next session rather than clearing it.
Recovering over work done while the offer stood SHALL ask about that work as
any replacement of the document does.

#### Scenario: An agent is served while recovery is on offer
- **WHEN** the application starts with recovered work on offer and an agent
  connects before anyone answers
- **THEN** the agent's calls are served and the offer is still up

#### Scenario: An unanswered offer survives a clean quit
- **WHEN** the application is closed without the offer being answered
- **THEN** the offer is made again on the next start

### Requirement: A document round-trips through the agent door
A document built through the door SHALL be savable to a named path and
reopenable from it through the door, and its layers, masks, grid passes,
armature, transforms and hierarchy SHALL come back as they were saved.

#### Scenario: Save, reopen and compare
- **WHEN** a document with masks, grid passes, a rig, a transform and a
  hierarchy is saved to a path, replaced, and opened again from that path
- **THEN** the document read back matches the one that was saved
