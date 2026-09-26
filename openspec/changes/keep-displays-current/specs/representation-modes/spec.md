## MODIFIED Requirements

### Requirement: The bar sheds its parts in a stated order
Where the window is too narrow for the bar to show everything, it SHALL give up
its explanatory phrases first, its heading second, and then fold its crossings
into one control that opens the conversion panel, where the same crossings are
offered. A phrase given up SHALL remain available on hover, and the folded
control SHALL name the crossings on hover. The crossings SHALL never be lost:
they are drawn as a row, or reachable through the folded control.

A card SHALL always carry both an icon and a name. The bar SHALL scroll rather
than reduce a card to an icon alone.

#### Scenario: The phrases go first
- **WHEN** the region holding the bar is narrowed
- **THEN** the cards drop their phrases before anything else is lost, and the phrases appear on hover

#### Scenario: The crossings survive
- **WHEN** the bar cannot show everything
- **THEN** the crossings are still drawn, or folded into one control that opens
  the panel offering them

#### Scenario: Nothing runs past the bar at 1280
- **WHEN** the window is 1280 wide, in any shipped language and on a layer of
  any representation
- **THEN** every card and the crossings, drawn or folded, stand inside the bar
  without scrolling

#### Scenario: A card never becomes an icon alone
- **WHEN** the bar has less room than even its shortest arrangement needs
- **THEN** it scrolls, and every card still shows an icon and a name
