## MODIFIED Requirements

### Requirement: Documents are saved and opened in the engine's format
The application SHALL save and open documents through the engine's own document
format, preserving layers, their representation, stack order, visibility,
protection, names and transforms.

A mask painted on a subtool SHALL be part of the saved document and SHALL come
back covering the same region when the document is reopened. A document written
before masks were saved SHALL open with no mask rather than failing.

#### Scenario: A round trip preserves the stack
- **WHEN** a document with several layers is saved and opened again
- **THEN** the layers come back in the same order, with the same
  representations, names and protection

#### Scenario: A mask survives the round trip
- **WHEN** a mask is painted on a subtool, the document is saved, closed and
  opened again
- **THEN** the mask covers the same region and still gates the brushes on that
  subtool

#### Scenario: A document with no mask opens unchanged
- **WHEN** a document written before masks were saved is opened
- **THEN** it opens normally and carries no mask
