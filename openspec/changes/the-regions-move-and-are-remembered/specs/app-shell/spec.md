## MODIFIED Requirements

### Requirement: Panels can be resized, collapsed and restored
The user SHALL be able to resize and collapse each panel region and restore the
default layout in one action. Layout SHALL persist across sessions.

A resize SHALL be clamped so that a region can neither vanish nor swallow the
viewport. A collapsed region SHALL be given no space at all rather than a narrow
one, and SHALL remember the size it had, so that bringing it back returns the
size the user chose rather than a default.

The arrangement SHALL NOT be document state: it SHALL emit no command, enter no
edit history, and reach no saved document. Where it cannot be stored, or where
what was stored is unreadable, the application SHALL open at the design's own
sizes rather than failing to open.

#### Scenario: Layout survives a restart
- **WHEN** the user resizes and collapses panels and restarts the application
- **THEN** the layout is as it was left

#### Scenario: Restoring defaults is one action
- **WHEN** the user chooses to reset the layout
- **THEN** every region returns to its default size and expansion state

#### Scenario: A collapsed region gives up its space
- **WHEN** a region is collapsed
- **THEN** it draws nothing, and the space it held goes to the viewport

#### Scenario: Bringing a region back restores the chosen size
- **WHEN** a region is resized, collapsed, and brought back
- **THEN** it returns to the size it was resized to

#### Scenario: A corrupt stored layout does not stop the application
- **WHEN** the stored arrangement is unreadable
- **THEN** the application opens at the design's sizes

#### Scenario: Rearranging the regions changes no document
- **WHEN** the user resizes, collapses or resets the regions
- **THEN** no command is emitted and the edit history is unchanged

### Requirement: The menu bar carries the application's commands
The menu bar SHALL present File, Edit, View, Sculpt, Brushes, Masks, Window and Help menus. Every menu item SHALL dispatch through the same command path as its equivalent control elsewhere in the interface, SHALL display its keyboard shortcut where one exists, and SHALL be disabled with the same conditions as that equivalent control.

No menu the bar presents SHALL be empty. A menu with nothing under it is a
promise the interface does not keep.

#### Scenario: The Window menu carries the regions
- **WHEN** the user opens the Window menu
- **THEN** each resizable region is offered, showing whether it is on screen, together with a reset
