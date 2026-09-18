## ADDED Requirements

### Requirement: A hierarchy is sculpted with the mesh vocabulary less its colour
The application SHALL offer, on a layer holding a subdivision hierarchy, the
fixed-topology brushes it offers on a mesh layer, together with the mask brush.
It SHALL NOT offer the brushes that write vertex colour. Which tools reach a
hierarchy SHALL be derived from the same declared table every other
representation is derived from, and the count SHALL be asserted against the
engine's own vocabulary so that a verb the engine gains is a failing count
rather than a silence.

#### Scenario: The mesh brushes reach a hierarchy
- **WHEN** the tools offered on a hierarchy are listed
- **THEN** they are the tools offered on a mesh layer, less the two colour
  brushes

#### Scenario: A tool the mesh sculptor does not have is not invented here
- **WHEN** a tool is offered on a hierarchy
- **THEN** it is also offered on a mesh layer

#### Scenario: The mask is the same call wherever it is painted
- **WHEN** the mask brush is used on any representation
- **THEN** it invokes the same engine verb, because a mask belongs to none of
  them

### Requirement: A colour brush on a hierarchy is refused with the reason
The application SHALL state, when a colour brush is asked for on a hierarchy,
that a hierarchy stores where a vertex went rather than what colour it is, in
addition to naming the representations where the brush does apply.

#### Scenario: The refusal says more than where else it works
- **WHEN** a colour brush is selected against a hierarchy
- **THEN** the refusal names both the representations that do carry colour and
  the reason this one does not

#### Scenario: Only this absence carries a reason
- **WHEN** any other tool is absent from any other representation
- **THEN** the refusal names where the tool applies and nothing further

### Requirement: A smooth on a hierarchy states that it picks a frequency
The application SHALL state, on the smoothing tool for a hierarchy alone, that
a smooth there acts on the form, on the detail alone, or on the form with the
detail carried through unchanged.

#### Scenario: The caveat is shown for the smooth on a hierarchy
- **WHEN** the smoothing tool is shown against a hierarchy
- **THEN** its caveat says the smooth picks a frequency

#### Scenario: The same tool on a mesh carries no such caveat
- **WHEN** the smoothing tool is shown against a mesh layer
- **THEN** no caveat is shown

### Requirement: A hierarchy with no cage yet says what it is waiting for
The application SHALL refuse the tools on a hierarchy row whose cage has not
arrived, naming the cage rather than naming geometry, so that the refusal sends
a sculptor to the crossing that builds one.

#### Scenario: A row before its cage
- **WHEN** a tool is asked for on a hierarchy carrying no geometry
- **THEN** it is refused, and the refusal names a cage

#### Scenario: A mesh row before its triangles says something different
- **WHEN** a tool is asked for on a mesh layer carrying no geometry
- **THEN** the refusal names a mesh
