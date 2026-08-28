## Purpose

Combining two subtools — union, subtract, intersect — into a new subtool
carrying the result, so a sculptor can cut one form with another and keep
working on what comes out.

## ADDED Requirements

### Requirement: Two subtools combine into a third
The application SHALL offer union, subtraction and intersection between two
subtools, and the result SHALL arrive as a new subtool: named for what made
it, selected on arrival, and carrying no special status afterwards — it can be
sculpted, transformed, deformed and used as an operand again.

The order of the two operands SHALL be stated in the interface, because
subtraction is not symmetric and "A minus B" is the whole of what the sculptor
is choosing.

#### Scenario: A cylinder cuts a sphere
- **WHEN** the sculptor subtracts a cylinder subtool from a sphere subtool
- **THEN** a new subtool holds the sphere with a cylindrical bore through it,
  and it is the active subtool

#### Scenario: The result is an ordinary subtool
- **WHEN** the result of a boolean is sculpted, moved and used as the operand
  of a second boolean
- **THEN** each of those works exactly as it does on a subtool that was never
  a result

#### Scenario: Which operand is subtracted from which is stated
- **WHEN** the sculptor sets up a subtraction between two subtools
- **THEN** the interface names which is being cut and which is doing the
  cutting, and swapping them changes the result accordingly

### Requirement: The operands survive unless the sculptor says otherwise
A boolean SHALL keep both operand subtools by default, hiding rather than
removing them, and SHALL offer consuming them as an explicit choice. Whichever
is chosen, the whole operation SHALL be a single undo step.

Keeping the operands is what makes the boolean recoverable: the engine
composes layers by hard union, so the result is baked and its operands cannot
be re-edited through it. A sculptor who can still reach the cylinder can move
it and run the boolean again; one whose cylinder was consumed cannot.

#### Scenario: Operands are kept and hidden
- **WHEN** a boolean completes with the default choice
- **THEN** both operands are still in the scene, hidden, and the result is
  what the viewport shows

#### Scenario: One undo takes back the whole boolean
- **WHEN** the sculptor undoes once after a boolean
- **THEN** the result subtool is gone and both operands are visible again,
  exactly as they were

#### Scenario: Consuming the operands is deliberate
- **WHEN** the sculptor chooses to consume the operands
- **THEN** the operands are removed and the interface has stated that this is
  what will happen before it runs

### Requirement: A boolean states its cost before it runs
Because the result is sampled onto a lattice rather than kept as an edit list,
the application SHALL present what that costs — the resolution the result will
have, and what sampling at that resolution does to surface accuracy, thin
features and sharp edges — before running the boolean, using the same
vocabulary as the conversion crossings the application already prices. It
SHALL NOT run a boolean the sculptor has not confirmed.

The sculptor SHALL be able to choose the resolution, and the default SHALL
follow the operands' own detail rather than a fixed constant.

#### Scenario: The cost is shown and the boolean waits
- **WHEN** the sculptor sets up a boolean
- **THEN** the estimated cost is shown and nothing is changed until it is
  confirmed

#### Scenario: A finer resolution is chosen
- **WHEN** the sculptor raises the resolution before confirming
- **THEN** the stated cost updates and the result is sampled at the chosen
  resolution

### Requirement: A boolean that cannot be run says why
Where an operand cannot take part — it is empty, it is protected against
editing, the two do not overlap in a way that makes the chosen operation
meaningful, or the pair would exceed the document's budget — the application
SHALL refuse with a reason naming the operand and the cause, and SHALL leave
the scene unchanged.

An intersection of two subtools that do not touch produces nothing. The
application SHALL say so rather than creating an empty subtool.

#### Scenario: Intersecting two forms that do not touch
- **WHEN** the sculptor intersects two subtools standing apart
- **THEN** the operation is refused with that as the stated reason, and no
  empty subtool is created

#### Scenario: A ghosted operand is refused by name
- **WHEN** one of the two chosen subtools is ghosted
- **THEN** the refusal names that subtool and why it cannot take part

### Requirement: Every representation can be an operand
A subtool SHALL be usable as an operand whatever it is made of — an SDF edit
list, a voxel grid or an imported mesh — with the crossing each one needs
performed as part of the operation rather than demanded of the sculptor
beforehand.

#### Scenario: A mesh is cut by a primitive
- **WHEN** an imported mesh subtool is subtracted from with a box subtool
- **THEN** the result is a subtool holding the cut mesh's form, and the
  sculptor was not asked to convert anything first

#### Scenario: A voxel subtool joins an SDF subtool
- **WHEN** a voxel subtool and an SDF subtool are unioned
- **THEN** the result holds both forms and the cost stated beforehand was the
  one for that pair
