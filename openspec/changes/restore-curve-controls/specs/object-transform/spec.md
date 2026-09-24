## ADDED Requirements

### Requirement: A selected curve exposes an operational transform target
When a curve has selected control points, choosing move, turn or scale SHALL draw the manipulator at their centroid. A manipulator drag SHALL transform the selected points and the swept form. Each frame SHALL resolve from the points at gesture start so pointer sampling rate does not change the result. The gesture SHALL be one undoable edit. With no selected curve points, choosing the curve target SHALL be refused visibly.

#### Scenario: Selected controls move with the manipulator
- **WHEN** the sculptor selects curve controls and drags the move handle
- **THEN** those controls and the visible tube move, and the manipulator follows their centroid

#### Scenario: A turn or scale spans several frames
- **WHEN** the sculptor turns or scales selected controls through multiple pointer samples
- **THEN** the final positions depend on the gesture start and final pointer position, and one undo restores them

#### Scenario: No controls are selected
- **WHEN** the curve target is requested with no selected control points
- **THEN** the target is refused with a visible reason
