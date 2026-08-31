## MODIFIED Requirements

### Requirement: The active representation is visible without inspection
The application SHALL show the active layer's representation above the
viewport, as one of three cards standing together — so that a sculptor sees not
only what the active layer is but what the alternatives are — and in the layer
stack. Neither SHALL require opening a panel. The three SHALL be
distinguishable from each other by more than colour alone: each SHALL carry an
icon of a distinct shape *and* its name.

The active card SHALL be distinguished by surface tone and an accent rail, in
the same grammar the active layer row uses, so that the state survives the hue
being removed.

Every word the bar draws SHALL come from the interface's own table. It SHALL
NOT draw a representation's engine label, which reads the same in every
language.

#### Scenario: The representation is on screen
- **WHEN** a layer is active
- **THEN** its representation is lit among the three above the viewport, and named beside the layer in the stack

#### Scenario: The alternatives are on screen too
- **WHEN** a layer is active
- **THEN** the other two representations are shown beside it, named and explained, rather than being invisible until a panel is opened

#### Scenario: Switching layers changes what is lit
- **WHEN** the user makes a layer of a different representation active
- **THEN** the lit card changes to match, and so does the tag in the stack

#### Scenario: The bar reads in the interface's language
- **WHEN** the interface is set to any supported locale
- **THEN** every name and phrase in the bar is in that language

## ADDED Requirements

### Requirement: Naming a representation never converts a layer
Selecting, clicking or otherwise addressing a representation card SHALL NOT
change the active layer's representation. Crossing between representations
costs work, is not always reversible, and SHALL remain an explicit operation
confirmed where its cost is stated.

The application SHALL offer, beside the cards, exactly the crossings the domain
declares from the active representation — derived from the declared set rather
than listed, so a crossing that is added is offered and one that is removed
stops being. Invoking one SHALL aim the conversion panel at that crossing and
open it, and SHALL NOT perform the conversion.

#### Scenario: A card is inert
- **WHEN** the user clicks a representation the active layer is not
- **THEN** no conversion runs and the layer is unchanged

#### Scenario: A crossing opens the panel that states its cost
- **WHEN** the user invokes a crossing from the bar
- **THEN** the conversion is aimed at that crossing and the panel is shown, and the conversion has not run

#### Scenario: An already-open panel is not closed by aiming it
- **WHEN** the conversion panel is open and the user invokes a crossing
- **THEN** the panel stays open, aimed at the newly chosen crossing

#### Scenario: Only the crossings that exist are offered
- **WHEN** the active representation has no crossing to some other representation
- **THEN** no button offering it is drawn

### Requirement: The bar sheds its parts in a stated order
Where the window is too narrow for the bar to show everything, it SHALL give up
its explanatory phrases first, its heading second, and its crossings never. A
phrase given up SHALL remain available on hover.

A card SHALL always carry both an icon and a name. The bar SHALL scroll rather
than reduce a card to an icon alone.

#### Scenario: The phrases go first
- **WHEN** the region holding the bar is narrowed
- **THEN** the cards drop their phrases before anything else is lost, and the phrases appear on hover

#### Scenario: The crossings survive
- **WHEN** the bar cannot show everything
- **THEN** the crossings are still drawn

#### Scenario: A card never becomes an icon alone
- **WHEN** the bar has less room than even its shortest arrangement needs
- **THEN** it scrolls, and every card still shows an icon and a name
