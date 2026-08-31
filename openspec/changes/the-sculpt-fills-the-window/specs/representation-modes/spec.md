## MODIFIED Requirements

### Requirement: The tool shelf offers the verbs the active representation has
The application SHALL present, for the active layer, the tools that exist for
that representation. A tool that has no verb on the active representation SHALL
NOT be offered. Whether a tool exists for a representation SHALL be derived from
one declared table rather than from a rule written per tool.

The shelf MAY additionally let a sculptor browse another representation's
vocabulary, or a shortlist of their own, on request. While browsing either, a
tool the active layer has no verb for SHALL be shown as unavailable — dimmed,
with the reason on hover — and SHALL NOT be selectable. Browsing SHALL NOT be
the default: with nothing chosen the shelf SHALL show exactly the tools the
active layer can be sculpted with.

A shortlist SHALL be the user's own and SHALL span every representation, since
its purpose is finding a brush again rather than describing the active layer.
Where the shortlist is chosen and is empty, the shelf SHALL say how to add to
it.

Which set the shelf is showing is interface state. It SHALL emit no command and
SHALL NOT survive the application closing. The shortlist itself SHALL survive,
because it is a preference rather than a view.

#### Scenario: The shelf follows the active layer
- **WHEN** the user makes a voxel layer active and then a mesh layer active
- **THEN** the shelf's contents change to the verbs each representation has

#### Scenario: A tool with no verb here is absent
- **WHEN** a representation has no verb for a tool, and neither another representation nor the shortlist is being browsed
- **THEN** the tool is not shown in the shelf for that layer

#### Scenario: Browsing answers what another representation has
- **WHEN** the user asks to see another representation's tools
- **THEN** that representation's tools are listed

#### Scenario: A browsed tool cannot be picked
- **WHEN** the user clicks a tool the active layer has no verb for while browsing
- **THEN** the active tool does not change, and the reason is available on hover

#### Scenario: A shortlist spans the representations
- **WHEN** the user stars a brush that the active layer has no verb for and then browses the shortlist
- **THEN** that brush is listed, dimmed, and cannot be picked

#### Scenario: An empty shortlist explains itself
- **WHEN** the shortlist is chosen and nothing has been starred
- **THEN** the shelf says so, and says where the gesture is

#### Scenario: Browsing is not remembered
- **WHEN** the application is closed while another representation is being browsed and opened again
- **THEN** the shelf shows the active layer's own tools

#### Scenario: The active tool survives where it can
- **WHEN** the active tool exists on the newly active layer's representation
- **THEN** it stays active rather than being reset

#### Scenario: The active tool is replaced where it cannot
- **WHEN** the active tool does not exist on the newly active layer's
  representation
- **THEN** the application selects one that does and states that it changed
