## ADDED Requirements

### Requirement: A substituted tool is reported as one
When a layer switch replaces the active tool, the answer to the command that
made the switch SHALL name the tool that was chosen, the tool now in hand and
the representation, by the keys an agent chooses tools with. For as long as the
substitute is in hand, the session state's tool section SHALL name the tool it
stands in for, and SHALL NOT when the tool in hand is the one chosen.

#### Scenario: The switch's answer says what changed
- **WHEN** an agent switches from a voxel layer holding Raspar to an SDF layer
- **THEN** the answer carries a remark naming Raspar's key, the SDF
  representation and the key of the tool standing in for it

#### Scenario: State tells a given tool from a chosen one
- **WHEN** an agent reads the tool section after such a switch
- **THEN** it reports the substitute as the tool and Raspar's key as the tool it
  stands in for, and after the agent chooses a tool it reports no substitution
