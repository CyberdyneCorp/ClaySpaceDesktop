## ADDED Requirements

### Requirement: The chrome can be cleared away
The application SHALL offer a mode that hides the tool rail, the tool options
bar, the representation bar, both inspector regions, the brush shelf and the
status area, leaving the sculpt. It SHALL be reachable by a keyboard shortcut
and from a menu.

The menu bar SHALL remain on screen, so that the mode can be left without
knowing the shortcut.

While the chrome is hidden, the application SHALL show the active brush, the
representation a stroke would land on, and the brush's primary numbers, so that
sculpting does not become blind.

The mode SHALL be a presentation override: it SHALL NOT change which regions
the user had put away or how wide they are, and leaving it SHALL restore exactly
what was on screen before. It SHALL NOT persist across sessions.

#### Scenario: The chrome goes and the sculpt stays
- **WHEN** the user clears the chrome away
- **THEN** the inspectors, the shelf, the rail, the options bar and the status area are not drawn, and the menu bar is

#### Scenario: The brush is still readable
- **WHEN** the chrome is hidden
- **THEN** the active brush, its representation, and its size, intensity and flow are shown over the viewport

#### Scenario: The arrangement is untouched
- **WHEN** the user has put a region away, clears the chrome, and brings it back
- **THEN** that region is still put away and the others are as they were

#### Scenario: It does not survive a restart
- **WHEN** the chrome is hidden and the application is restarted
- **THEN** the interface opens with its chrome

### Requirement: A preference the user set is remembered
Where the application offers a choice that belongs to the person rather than to
the document — the arrangement of the regions, a shortlist of brushes, how much
an idle frame is worth spending on — that choice SHALL be stored between
sessions, beside the recent documents and the chosen language.

A stored preference SHALL be written under a name that does not change with the
interface's language or with presentation order. An entry this build does not
recognise SHALL be dropped rather than failing the file, and an unrecognised
value SHALL NOT become a preference the user never set.

#### Scenario: A shortlist survives a restart
- **WHEN** the user stars brushes and restarts the application
- **THEN** the same brushes are starred

#### Scenario: An unknown entry costs its own line
- **WHEN** a stored shortlist names something this build does not recognise
- **THEN** that entry is dropped and the rest are read

#### Scenario: An unrecognised value is not adopted
- **WHEN** a stored preference holds a value this build does not recognise
- **THEN** the application uses its own default rather than treating it as a choice
