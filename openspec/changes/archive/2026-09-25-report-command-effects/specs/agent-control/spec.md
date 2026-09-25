## ADDED Requirements

### Requirement: Harmless commands with no target report no effect
The agent interface SHALL return `outcome: nothing_to_do` with a reason when a cancel has no stroke, redo has no history entry, an outline end has no outline, a curve edit has no curve, pass recording is not active, or a rename commit has no rename. It SHALL return `outcome: applied` after an effective command.

#### Scenario: Repeating a harmless close
- **WHEN** an agent cancels a stroke with no stroke open
- **THEN** the result SHALL report `nothing_to_do` without changing the document or history

### Requirement: Diagnostics copy uses the system clipboard
The agent interface SHALL copy the current diagnostics report when `session/copy_diagnostics` is called and SHALL report a failure if the clipboard write fails.

#### Scenario: Copy diagnostics
- **WHEN** an agent calls `session/copy_diagnostics`
- **THEN** the system clipboard SHALL contain the current diagnostics report
