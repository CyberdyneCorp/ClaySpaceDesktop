## MODIFIED Requirements

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

Every gesture SHALL be given a name no other gesture in the process has used,
and **both** Move doors — the live transaction and the held drag — SHALL send
that name with every grab they write. The engine folds a grab into the one
leading an item's chain only where both carry the same name.

Without the name the fold decides by centre and radius compared bit for bit,
which cannot tell a drag continuing from a second drag pressed at the same point
at the same size — and the fold *replaces*, because a drag re-sends its whole
displacement. So an unnamed second press at one anchor did not cost an extra
grab; it lost the first pull. The name is not saved: a reopened document's grabs
are unnamed and never match a new gesture.

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

#### Scenario: A second drag from the same press keeps the first
- **WHEN** a Move drag is made, released, and a second is made from the same
  press point at the same brush size
- **THEN** the item carries one grab more than after the first drag, and the
  surface has moved further than the first drag left it

#### Scenario: Both doors name the gesture
- **WHEN** the same pair of drags is made once through the live transaction and
  once through the held drag
- **THEN** neither loses the first pull
