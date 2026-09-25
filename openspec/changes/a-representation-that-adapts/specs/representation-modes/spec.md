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
