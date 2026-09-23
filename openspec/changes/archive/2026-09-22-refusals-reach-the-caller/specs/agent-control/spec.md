## ADDED Requirements

### Requirement: A refused command is answered as a refusal, never as a success
A command the application did not carry out SHALL be answered to the client as
an error carrying the reason, and SHALL NOT be answered with an applied result.
In particular a command that changed nothing SHALL NOT report that it touched
the document.

This SHALL hold for every command the application can refuse, including the
ones the composition root runs itself rather than dispatching to a ViewModel —
a repair, a crossing, a rebuild, a pass of the active layer's stack, a
hierarchy's levels.

Writing the reason only to the process's error stream SHALL NOT count as
answering it. A stream nobody is reading is not a surface the client can reach,
and a refusal whose only record is one SHALL be treated as a refusal that was
lost.

#### Scenario: An operation refused for the representation is an error
- **WHEN** a client asks for a repair on a layer the operation does not apply
  to
- **THEN** it is refused with the sentence naming the representation, and no
  applied result is returned

#### Scenario: A crossing priced past its budget is an error
- **WHEN** a client asks for a conversion whose cell size prices the result
  past the memory budget
- **THEN** it is refused with the sentence naming the cost and the budget, and
  the document is unchanged

#### Scenario: A refused command did not touch the document
- **WHEN** any refused command is compared against the document before it
- **THEN** the document is unchanged, and nothing in the answer says it was
  touched
