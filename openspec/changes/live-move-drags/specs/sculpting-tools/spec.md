## ADDED Requirements

### Requirement: A drag costs the field the gesture, not the segments
A drag with Move on a field SHALL cost the layer's field the same whatever
number of segments the gesture is delivered in. The same drag delivered more
finely SHALL NOT lengthen the layer's deformer chain, and SHALL NOT lower its
safe step scale, by more than measurement noise.

A whole drag SHALL be one history entry, however many segments drew it.

A segment SHALL carry the displacement measured from the gesture's **anchor**
rather than from the previous segment, so that a sequence of segments ends where
a single drag of the final displacement ends rather than at a composition of
them.

Where the engine cannot hold a drag open on a layer, the application SHALL fall
back to applying it per segment, which is correct but costs more.

#### Scenario: The same drag cut more finely costs the same
- **WHEN** the user makes one drag delivered in four segments, and the same drag
  delivered in twelve
- **THEN** the layer's safe step scale is the same after both

#### Scenario: A drag is one action to undo
- **WHEN** the user completes a drag and undoes once
- **THEN** the whole drag is taken back, however many segments drew it

#### Scenario: Segments do not compose into a longer pull
- **WHEN** a drag is delivered as a sequence of segments
- **THEN** the surface ends where a single drag of the final displacement puts
  it

### Requirement: A drag is shown while it is being made
The surface SHALL follow the pointer during a drag rather than appearing only
when the pointer is released.

While the gesture is open the document SHALL NOT carry any part of the drag: the
layer's field SHALL measure as it did before the gesture began, and the history
SHALL be unchanged. Where the application draws the preview by writing to the
layer and taking it back, it SHALL take it back within the same segment, so that
a segment leaves the history depth where it found it.

Abandoning a drag SHALL leave neither a mark on the document nor a preview on
the screen.

The committed drag SHALL land where the preview showed it.

#### Scenario: The surface follows the pointer
- **WHEN** the user drags with Move across the form
- **THEN** the surface the viewport draws changes before the drag ends

#### Scenario: The field is untouched while the pointer is down
- **WHEN** a drag is open and segments have been applied
- **THEN** the layer's safe step scale is what it was before the drag began, and
  the history is unchanged

#### Scenario: An abandoned drag leaves nothing behind
- **WHEN** the user abandons a drag in progress
- **THEN** the history is unchanged and the surface is drawn as it was before

#### Scenario: The drag lands where it was previewed
- **WHEN** a drag is committed
- **THEN** the surface stands where the preview showed it

### Requirement: A mirrored drag pulls each side once
A drag under a layer mirror SHALL pull each side by what one drag pulls, not by
what two do. Where the engine reflects a drag into every image the layer emits
of it, the application SHALL NOT reflect the gesture again.

#### Scenario: A mirrored live drag is not doubled
- **WHEN** the user drags with Move on a mirrored layer
- **THEN** the near side moves as far as it does on an unmirrored layer, and the
  far side moves with it
