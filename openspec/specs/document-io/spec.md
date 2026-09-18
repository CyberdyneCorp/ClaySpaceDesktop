# document-io Specification

## Purpose
Opening and saving `.clayspace` documents through the engine's own format,
importing and exporting meshes, and the surrounding promises — a working unit, a
crash-survivable draft, a decision before unsaved work is closed, and a failure
that says so rather than leaving a half-written file.
## Requirements
### Requirement: Documents are saved and opened in the engine's format
The application SHALL open and save `.clayspace` documents through the engine's
document I/O. It SHALL NOT define a container format of its own, and SHALL NOT
write engine documents by any path other than the engine's writer. A saved
document SHALL preserve its layers, their representation, stack order,
visibility, protection, names and transforms.

A mask painted on a subtool SHALL be part of the saved document and SHALL come
back covering the same region when the document is reopened. A document written
before masks were saved SHALL open with no mask rather than failing.

#### Scenario: Round-trip preserves the document
- **WHEN** a document with layers, masks, an armature and a mesh layer is saved and reopened
- **THEN** every layer, mask, armature node and mesh layer is present with its content and settings intact, in the same stack order and with the same representations, names, visibility and protection

#### Scenario: Documents are portable across platforms
- **WHEN** the same document is saved on macOS and on Linux from equivalent state
- **THEN** the two files are byte-identical

#### Scenario: A document from a newer engine is refused clearly
- **WHEN** the user opens a document whose scene chunk version is newer than the engine supports
- **THEN** the application refuses to open it and states the version mismatch, rather than opening it partially

#### Scenario: A mask survives the round trip
- **WHEN** a mask is painted on a subtool, the document is saved, closed and
  opened again
- **THEN** the mask covers the same region and still gates the brushes on that
  subtool

#### Scenario: A document with no mask opens unchanged
- **WHEN** a document written before masks were saved is opened
- **THEN** it opens normally and carries no mask

### Requirement: Meshes are imported and exported in the engine's supported formats
The application SHALL import and export meshes in the formats the engine supports: OBJ with MTL, PLY, FBX and glTF 2.0 GLB. Export SHALL use the watertight mesher by default, and SHALL let the user choose the resolution and whether to decimate.

#### Scenario: Export is watertight by default
- **WHEN** the user exports without changing the mesher
- **THEN** the exported mesh is produced by marching tetrahedra and is watertight and 2-manifold

#### Scenario: Attributes are carried where the format supports them
- **WHEN** a model carrying vertex colors is exported to a format that supports them
- **THEN** the colors are written, and where the format does not support them the application says so before exporting

#### Scenario: A malformed mesh file is refused, not partially read
- **WHEN** the user imports a file whose declared vertex count does not match its payload
- **THEN** the import is refused with a stated reason and no partial geometry enters the document

### Requirement: Import limits are stated and adjustable
The application SHALL apply the engine's import guardrails, SHALL present the limit when an import exceeds it, and SHALL let the user raise the ceiling for that import rather than only failing.

#### Scenario: An oversized import offers a raised ceiling
- **WHEN** an import exceeds the configured read ceiling
- **THEN** the application states the limit and the file's size and offers to proceed with a raised ceiling

### Requirement: Unsaved work survives a crash
The application SHALL autosave recovery state for open documents at a configurable interval and after significant edits. On starting after an abnormal termination, it SHALL offer to recover each document that has recovery state newer than its saved file.

#### Scenario: Recovery is offered after a crash
- **WHEN** the application starts after terminating abnormally with unsaved changes
- **THEN** it lists the recoverable documents and lets the user open or discard each

#### Scenario: Autosave does not overwrite the user's file
- **WHEN** autosave runs on a document with unsaved changes
- **THEN** recovery state is written separately and the user's saved file is unchanged

#### Scenario: Clean exit clears recovery state
- **WHEN** the application exits normally with all documents saved
- **THEN** no recovery is offered on the next start

### Requirement: Closing with unsaved changes requires a decision
The application SHALL NOT discard unsaved changes without an explicit choice. Closing a modified document or quitting with modified documents open SHALL offer save, discard and cancel.

#### Scenario: Quitting with several modified documents
- **WHEN** the user quits with more than one modified document open
- **THEN** each is presented for a decision, and cancelling any one cancels the quit

### Requirement: Recent documents are offered
The application SHALL maintain a list of recently opened documents, present it for reopening, and remove entries whose files no longer exist when they are found to be missing.

#### Scenario: A moved file is not offered forever
- **WHEN** the user selects a recent document whose file no longer exists
- **THEN** the application says the file could not be found and removes it from the list

### Requirement: Documents carry a working unit
Each document SHALL carry a working unit, displayed in the interface and used for every length the interface shows — brush size, voxel size and dimensions. Changing the display unit SHALL NOT alter the document's geometry.

#### Scenario: Unit change is presentation only
- **WHEN** the user changes the displayed unit
- **THEN** displayed values convert and the geometry is unchanged and the document is not marked modified

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

