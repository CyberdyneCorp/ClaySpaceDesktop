## ADDED Requirements

### Requirement: The report says where the document's memory is, not only how much
The diagnostics report SHALL carry the document's memory broken down by what
releasing it would cost: the user's work, which is never released; what
reconstructs identically, whose release costs only a stall; and undo depth,
which is the application's own policy. It SHALL carry the total beside them,
and SHALL NOT carry the total alone.

The breakdown SHALL be the engine's own classification of its own figures. The
application SHALL NOT re-derive it, so that a category added to the engine and
left unclassified cannot make the parts and the total disagree.

The figures SHALL include every surface the application holds beside the
document. A sculpting session is an owning handle the host keeps next to its
document rather than inside it, so the engine cannot walk one and reports it as
nothing; the application SHALL ask each session what it costs and hand the
result back to the engine rather than publish a figure that omits it.

The report SHALL state how many surfaces were asked, as well as what they came
to, so that a surface figure of zero can be read as *there are none* rather
than as *nobody asked*.

#### Scenario: The breakdown names which part
- **WHEN** a person copies the diagnostics report
- **THEN** it states what is the user's work, what is rebuildable and what is
  undo depth, each as a figure, beside the total

#### Scenario: A sculpting session is in the figure
- **WHEN** the document holds a mesh subtool that has been sculpted
- **THEN** the reported total includes what that session costs, and is larger
  than what the engine reports for the document alone by exactly that amount

#### Scenario: A document holding no surface still says so
- **WHEN** the document holds no sculpting session
- **THEN** the report states that zero surfaces were asked, and the figures are
  the ones the engine reports for the document alone
