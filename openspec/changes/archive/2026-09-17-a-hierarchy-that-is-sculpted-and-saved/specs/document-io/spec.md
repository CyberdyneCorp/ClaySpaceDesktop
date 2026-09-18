## ADDED Requirements

### Requirement: A hierarchy's sculpt is saved beside the document
A `.clayspace` carries a hierarchy's cage and nothing standing on it. The
application SHALL therefore write the sculpt to a file beside the document, and
saving SHALL reproduce it exactly when the document is opened again — the level
count, where the brush writes, what the viewport draws, and the detail itself.

Because that file is the work rather than bookkeeping, a failure to write it
SHALL fail the save.

The bytes SHALL be priced before they are allocated.

#### Scenario: A sculpt survives a save and a reopen
- **WHEN** the user sculpts a hierarchy, saves, and opens the document again
- **THEN** the row is a hierarchy again, at the same levels, with the sculpt on
  it

#### Scenario: A save that cannot write the sculpt fails
- **WHEN** the sculpt cannot be written beside the document
- **THEN** the save reports a failure rather than leaving a file that looks
  complete and holds a flat cage

### Requirement: A document whose sculpt is missing opens as the cage it holds
Opening a document whose companion file is missing or unreadable SHALL NOT
refuse the document, and SHALL NOT go on describing the row as a hierarchy that
has silently lost every level. The row SHALL be presented as the mesh layer it
now is, so the change is visible where the sculptor already looks.

Where a record was found and could not be honoured, the row SHALL be named in
the diagnostics report.

#### Scenario: The document still opens
- **WHEN** a document is opened without the file holding its sculpt
- **THEN** the document opens, and the row holds the cage

#### Scenario: The row says what it is
- **WHEN** a hierarchy's sculpt could not be restored
- **THEN** the row is shown as a mesh layer rather than as a hierarchy with no
  levels

#### Scenario: A damaged record costs one row
- **WHEN** one record among several cannot be reconstructed
- **THEN** the other rows keep their hierarchies, and the one that did not is
  named in the diagnostics report
