## ADDED Requirements

### Requirement: A capability row carries what the call is, not only its name
Each column of the capability table SHALL be a typed binding carrying the
engine entry point, the intent the tool means by that call, the engine family
the call belongs to, and the fidelity with which it keeps the promise the
tool's label makes. No capability information SHALL be left in prose alone.

An entry point is a name. Whether a row is the representation's natural verb,
a strength the representation alone has, a useful stand-in, or several verbs
composed is what a sculptor is actually asking when they ask what a tool does
here — and stated as prose it can drive nothing and be held to nothing.

The fidelity of a binding SHALL match what the engine has been measured to do.
Where the engine documents one of two rows as the faithful implementation of an
intent and the other as its approximation, the table SHALL say which is which.

A binding MAY be a recipe: several engine verbs in a fixed order standing in
for one the engine does not have. A recipe SHALL be marked as one, so that a
composed tool is describable rather than absent.

The shelf, the availability refusal, the tool notes and the diagnostics report
SHALL all read this one table, and no other crate SHALL decide anything per
tool and representation.

#### Scenario: A field's Standard and its Inflate are ordered as measured
- **WHEN** the rows for Padrão and Inflar on an SDF layer are read
- **THEN** both name the relief operation, Inflar's binding is the native one
  and Padrão's is an approximation, and Padrão is the row carrying the caveat

#### Scenario: A composed tool is described rather than omitted
- **WHEN** a tool reaches a representation through several verbs rather than one
- **THEN** its row states them as a recipe, and the shelf, the refusal and the
  report describe it as one

#### Scenario: A second capability table is added elsewhere
- **WHEN** a View, a ViewModel, the engine adapter or the agent-facing crate
  decides something per tool and representation
- **THEN** the layering check fails, naming the file

### Requirement: The typed row's claims are checked
Every claim a typed binding makes SHALL be held by a test.

A tool SHALL mean one thing wherever it is offered: every binding of one tool
SHALL declare the same intent, because the shelf presents them as one button
with one tooltip and a column borrowed from a neighbouring verb is how that
button comes to mean two things.

A binding SHALL be filed under the family whose calls it names, where the
engine spells that family as a prefix.

A caveat SHALL NOT hang off a binding that claims to do exactly what its
label says. The caveat and the fidelity are two halves of one fact, and a
caveat on a faithful row is a sentence about nothing.

Two tools offered on one representation SHALL NOT have bindings identical in
every part, because then nothing distinguishes them but their labels. Where
that is nonetheless the truth, the pair SHALL be recorded with its reason, and
a recorded pair that has since come apart SHALL fail so the record is removed.

#### Scenario: A column is borrowed from a neighbouring verb
- **WHEN** one of a tool's bindings is changed to a call meaning something else
- **THEN** a test fails naming the tool and the two intents it would carry

#### Scenario: A caveat outlives the difference it described
- **WHEN** a binding's fidelity is corrected to the plain reading of its label
  while its caveat is left in place
- **THEN** a test fails naming the tool, the representation and the binding

#### Scenario: Two shelf entries collapse onto one binding
- **WHEN** two tools offered on one representation come to name the same call
  with the same intent, family and fidelity
- **THEN** a test fails unless the pair is recorded as one verb under two words,
  with the reason it still stands
