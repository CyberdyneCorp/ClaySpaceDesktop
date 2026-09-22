## ADDED Requirements

### Requirement: The report says which binding the tool in hand reaches
The diagnostics report SHALL carry the tool in hand, the representation of the
active layer, and the binding the two resolve to — its entry point, intent,
family and fidelity.

One word on the shelf stands for up to four different engine calls, and which
one ran depends on a layer the person writing the report is not thinking about.
"Padrão did something I did not expect" is therefore the same sentence for a
relief stroke on a field and a cell deposit on a grid, and without the binding
behind it the report cannot be acted on at all.

The report SHALL read the binding from the capability table rather than being
handed a rendered sentence, so there is no second place where a tool's binding
is described and no way for the line to say something the shelf does not.

Where the tool in hand has no binding on the active layer, the report SHALL say
so rather than omitting the line: the shelf does not offer such a tool, so
meeting one means something upstream of the shelf put it in hand.

#### Scenario: The same tool on two representations
- **WHEN** a report is taken with Padrão in hand on an SDF layer, and again on a
  voxel layer
- **THEN** the two reports name different entry points and different fidelities
  under the same tool name

#### Scenario: A tool the shelf would not offer
- **WHEN** a report is taken with a tool in hand that the active layer has no
  verb for
- **THEN** the line names the tool, the representation and the absence
