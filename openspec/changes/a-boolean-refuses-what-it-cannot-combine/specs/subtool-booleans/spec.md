## ADDED Requirements

### Requirement: An operand is refused when it is chosen
Where what a subtool *is* makes it unable to be an operand — it is no longer in
the document, it is empty, it is a subdivision hierarchy — or the same subtool
is chosen as both base and tool, the application SHALL refuse the choice when
it is made, with a reason naming the subtool, and SHALL leave the pair it
already held in place. A boolean that is run anyway SHALL be refused for the
same reason, in the same words.

Protection against editing SHALL be checked when the boolean runs rather than
when it is chosen, because the sculptor may lift it in between.

#### Scenario: A hierarchy is refused as it is picked
- **WHEN** the sculptor picks a subdivision hierarchy subtool as an operand
- **THEN** the choice is refused naming the hierarchy, the panel keeps the pair
  it held, and nothing is priced or sampled for it

#### Scenario: The same subtool twice is refused as it is picked
- **WHEN** the sculptor picks as the tool the subtool already chosen as the base
- **THEN** the choice is refused saying a boolean needs two different subtools,
  and the pair already chosen stays ready to run

#### Scenario: A choice that is taken clears the last refusal
- **WHEN** a refused choice is followed by one that is taken
- **THEN** the panel no longer shows the earlier refusal

### Requirement: A subtool with strokes and no form is refused by what it lacks
Where an operand holds items but no form of its own — strokes that offset a
surface, such as relief or incise, on a subtool with nothing beneath them to
offset — the application SHALL refuse the boolean before sampling it, naming
the subtool and saying that it has no form of its own, and SHALL leave the
scene unchanged. It SHALL NOT pass on the engine's sampling error in its place.

#### Scenario: A relief-only subtool is refused by name
- **WHEN** a field subtool sculpted only with relief strokes is one operand of a
  boolean
- **THEN** the boolean is refused naming that subtool and saying it has no form
  of its own, and no subtool is created

#### Scenario: Two sculpted field subtools make a result
- **WHEN** two overlapping field subtools, each sculpted with strokes that add
  material, are combined
- **THEN** a new subtool holds the result and is the active subtool
