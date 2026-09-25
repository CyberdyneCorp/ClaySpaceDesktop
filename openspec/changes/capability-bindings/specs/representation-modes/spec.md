## ADDED Requirements

### Requirement: The shelf and diagnostics share binding evidence
The default shelf SHALL offer exactly the tools whose bindings exist for the
active representation in the authoritative capability table. Browsing another
representation or a personal shortlist MAY show other tools, but an unbound
tool SHALL remain unavailable with its `ToolNote`. Availability and diagnostic
descriptions SHALL derive from the same binding and fidelity evidence.

#### Scenario: Switching representations changes the shelf
- **WHEN** the active layer changes from field to voxel
- **THEN** the default shelf displays the bindings for voxel and no unbound field-only tool

#### Scenario: Diagnostics match the shelf
- **WHEN** a tool is offered on the shelf
- **THEN** the diagnostic entry names the binding and fidelity the dispatcher uses

### Requirement: Binding substitutions are explained across representations
The application SHALL keep brush settings by tool and representation. A layer
switch SHALL preserve the selected tool when it has a binding on the new
representation. When it does not, the application SHALL select a bound tool
and report the old tool, substitute and reason to the user and agent caller.

#### Scenario: Settings survive a round trip
- **WHEN** a sculptor adjusts a tool on a mesh, switches to voxel and returns
- **THEN** that mesh tool has its previous settings

#### Scenario: A substitution is explained
- **WHEN** the selected tool has no binding on the newly active layer
- **THEN** a bound replacement is selected and the substitution is reported without converting the layer
