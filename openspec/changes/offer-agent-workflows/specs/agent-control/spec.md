## ADDED Requirements

### Requirement: Offered agent actions match command routing
Every command declared as agent-addressable by `Home::In` SHALL be offered by
the catalogue, belong to a declared group, and dispatch to the same model
command. Every action the catalogue offers SHALL have both a command home
and a dispatch route. Catalogue entries SHALL have distinct group and action
names and declare the arguments the builder reads.

The agent catalogue SHALL offer the dispatchable cut, retopology, UV, conform
and bake actions. It SHALL offer the existing brush pressure, taper and rake
controls and curve point insertion. Starting a texture bake SHALL remain
unoffered while it requires the application's destination file panel.

#### Scenario: A client discovers geometry workflows
- **WHEN** a client lists tools and describes the cut, retopo, uv, conform or
  bake group
- **THEN** it receives the group's dispatchable actions and their arguments
- **AND** a call to an offered action reaches its model command

#### Scenario: The catalogue and router stay in agreement
- **WHEN** the catalogue contract test compares command homes, dispatch arms
  and catalogue rows
- **THEN** the three action sets agree in both directions, with no duplicate
  rows or rows belonging to an undeclared group

#### Scenario: A bake still requires a destination
- **WHEN** a client describes the bake group
- **THEN** it can configure or cancel a bake but is not offered a run action
