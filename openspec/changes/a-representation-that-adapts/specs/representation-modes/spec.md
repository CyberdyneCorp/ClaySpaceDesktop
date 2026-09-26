## ADDED Requirements

### Requirement: Dynamic is a first-class representation
The document SHALL represent DynamicSurface with a distinct representation
identity and serialized tag. The viewport bar, layer stack, inspector,
capability table and agent state SHALL identify it as Dynamic. A saved Dynamic
layer SHALL reopen as Dynamic with the same topology, geometry and attributes.

Dynamic SHALL offer only bindings that execute on DynamicSurface. The Layer
brush SHALL be absent because adaptive edits create vertices without a
stroke-start reference surface for its ceiling; the absence SHALL be
explained to the sculptor and agent.

#### Scenario: A Dynamic layer survives save and load
- **WHEN** a Dynamic layer is edited, saved and reopened
- **THEN** it remains Dynamic and its connectivity, positions and attributes match the saved state

#### Scenario: Layer is deliberately absent
- **WHEN** a Dynamic layer is active
- **THEN** Layer is absent from the default shelf and browsing it explains why it has no Dynamic binding

### Requirement: Adaptive remeshing is scheduled per verb
A Dynamic stroke SHALL remesh around each stamp at the time its verb needs:
Move SHALL refine after its deformation, the deposit brushes (including Clay)
SHALL refine before it, and Snake Hook SHALL refine both before and after. The
schedule SHALL be stated per tool where the interface and the agent can read it
rather than offered as one global setting, and a colour brush SHALL be refused
on a Dynamic layer that carries no vertex colour rather than remeshing it.

#### Scenario: A deposit refines before it deposits
- **WHEN** a Standard stroke lands on a coarse Dynamic layer
- **THEN** the layer has more triangles afterwards, and the same stroke on a mesh layer leaves the triangle count unchanged

#### Scenario: A move refines what it stretched
- **WHEN** a Move stroke drags a Dynamic layer's surface far from where it was
- **THEN** the stretched region has gained triangles

#### Scenario: Paint over a surface with no colour is refused
- **WHEN** Paint is applied to a Dynamic layer read from a mesh without vertex colour
- **THEN** the stroke is refused as a missing attribute and the surface and history are unchanged
