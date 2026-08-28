## ADDED Requirements

### Requirement: The window is organized into fixed functional regions
The application window SHALL present: a menu bar; a tool rail along the leading edge; a tool options bar under the menu bar carrying the active tool's primary parameters; a left region holding the scene tree, the layer stack and sculpting settings; a central viewport; a right region holding material, geometry, resolution and brush-control inspectors; a brush shelf along the trailing edge of the window; and a status area.

The tool rail SHALL offer, as icon buttons with their name and shortcut on
hover, the controls a sculptor reaches for between strokes: mask painting,
frame, polyframe, the reference images, the shapes panel, the deformation
cage, the curve, the deformations, undo and redo. Each SHALL dispatch the
same command as its menu entry under the same enabled conditions, and SHALL
show its on/off state where it has one.

#### Scenario: A panel is opened from the rail
- **WHEN** the shapes button on the rail is clicked
- **THEN** the same command the Escultura → Formas menu entry dispatches is
  emitted, and the button reads as on while the panel is open

#### Scenario: The rail greys what the menu greys
- **WHEN** the active layer cannot be caged
- **THEN** the rail's cage button is disabled with the same reason the menu
  entry carries

The tool options bar SHALL be headed by the active brush — its mark, its name
and a one-line description — separated from the brush's parameters by a rule,
and the head SHALL change with the active brush. Where the window is narrower
than the bar, the bar SHALL scroll rather than clip its last control.

#### Scenario: The options bar names its brush
- **WHEN** the active brush changes from Standard to Move
- **THEN** the head of the options bar shows Move's mark and name

#### Scenario: Regions are present on first run
- **WHEN** the application starts with no stored layout
- **THEN** every region is present and populated at its default size

#### Scenario: The viewport takes the remaining space
- **WHEN** the window is resized
- **THEN** the panel regions keep their widths and the viewport absorbs the difference

### Requirement: Panels can be resized, collapsed and restored
The user SHALL be able to resize and collapse each panel region and restore the default layout in one action. Layout SHALL persist across sessions.

#### Scenario: Layout survives a restart
- **WHEN** the user resizes and collapses panels and restarts the application
- **THEN** the layout is as it was left

#### Scenario: Restoring defaults is one action
- **WHEN** the user chooses to reset the layout
- **THEN** every region returns to its default size and expansion state

### Requirement: The menu bar carries the application's commands
The menu bar SHALL present File, Edit, View, Sculpt, Brushes, Masks, Window and Help menus. Every menu item SHALL dispatch through the same command path as its equivalent control elsewhere in the interface, SHALL display its keyboard shortcut where one exists, and SHALL be disabled with the same conditions as that equivalent control.

#### Scenario: A menu item and a panel control agree
- **WHEN** an operation is unavailable and is present both in a menu and as a panel control
- **THEN** both are disabled, and for the same stated reason

#### Scenario: Shortcuts are discoverable
- **WHEN** a menu is opened
- **THEN** each item with a shortcut displays it

### Requirement: Keyboard shortcuts cover the sculpting loop and are remappable
The application SHALL provide keyboard shortcuts for the operations used continuously while sculpting — brush selection, size, intensity, symmetry, masking, undo, redo, view presets and frame — and SHALL let the user remap them. A conflicting assignment SHALL be reported rather than silently overriding.

#### Scenario: A conflicting assignment is reported
- **WHEN** the user assigns a shortcut already bound to another command
- **THEN** the conflict is shown with the command that holds it, and the assignment is not applied until the user resolves it

### Requirement: The status area reports document, memory and backend state
The status area SHALL display the current document name and modified state, the working unit, the memory in use against the configured budget, and the active evaluation backend.

#### Scenario: Memory reflects the engine's own accounting
- **WHEN** memory usage is displayed
- **THEN** the figures come from the engine's brick cache statistics and budget, not from an estimate maintained by the application

#### Scenario: Approaching the budget is visible before it is reached
- **WHEN** memory in use approaches the configured budget
- **THEN** the indicator changes state before the budget is exhausted, rather than only at failure

### Requirement: Memory budget exhaustion is handled without data loss
When the engine reports that an operation would exceed the memory budget, the application SHALL present the shortfall, offer to raise the budget or reduce resolution, and SHALL leave the document and existing data intact.

#### Scenario: A budget-exceeded operation leaves the document valid
- **WHEN** an operation is refused for exceeding the memory budget
- **THEN** the document is unchanged, existing cached data remains valid, and the user is told what was needed

### Requirement: The window title identifies the document and its state
The window title SHALL show the document name and indicate unsaved changes.

#### Scenario: The title marks unsaved work
- **WHEN** a document has unsaved changes
- **THEN** the title indicates it, and the indication clears on save

### Requirement: Interface text is externalized and localizable
All user-facing text SHALL be externalized into resource files with no literal user-facing strings in code. The application SHALL ship Brazilian Portuguese and SHALL follow the system locale where a translation exists, falling back to a defined default otherwise.

#### Scenario: No literal user-facing strings
- **WHEN** the source is inspected for user-facing text
- **THEN** every such string is resolved from a resource file

#### Scenario: An untranslated locale falls back
- **WHEN** the system locale has no shipped translation
- **THEN** the interface presents in the default locale rather than showing untranslated keys

#### Scenario: Layout survives longer translations
- **WHEN** the interface is displayed in a locale whose labels are substantially longer
- **THEN** labels wrap or elide within their regions without overlapping or clipping adjacent controls

### Requirement: Errors are reported where they occurred, with a cause
Failures SHALL be reported near the action that caused them, stating what failed and why in the user's terms. Engine result codes and internal identifiers SHALL NOT be presented as the primary message, though they SHALL be available in the diagnostics view.

#### Scenario: A failed export explains itself
- **WHEN** an export fails
- **THEN** the message states what could not be written and why, and the engine's detail message is available in diagnostics

#### Scenario: An error does not discard work
- **WHEN** any recoverable error occurs
- **THEN** the open document and its undo history are preserved
