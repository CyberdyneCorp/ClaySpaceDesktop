## ADDED Requirements

### Requirement: The assembled surface's radial scale is reachable
The wrapper SHALL bind the engine's magnify of a layer's **assembled** surface,
with the centre, the signed strength, the region's radius and the easing the
engine's descriptor takes, and SHALL bind the preview that answers which nodes
it would warp without touching the document.

It is the radial counterpart to the assembled drag and exists for the same
reason: the per-item magnify deformer takes its centre in one item's local
frame, so on a form blended from several items it scales that item's field and
leaves the rest, with no error to show for it.

The strength SHALL be passed through signed and unscaled. It is a **total from
the start of a gesture** rather than an increment on the last frame — the
engine replaces its own last frame at a centre and radius it already holds —
and a wrapper that accumulated or rescaled it would break that idempotence.

The descriptor SHALL carry the radius and the easing and nothing else: a radial
scale has no direction to gate a half-space on, and no gesture identity to fold
on beyond the region itself.

#### Scenario: A blended form scales as one surface
- **WHEN** a magnify is applied at the join of two smooth-unioned items
- **THEN** both items take a warp and the surface moves on both sides of the
  blend

#### Scenario: The sign is the verb
- **WHEN** the same region is magnified at a positive strength and at a
  negative one
- **THEN** the surface swells away from the centre in the first case and
  gathers toward it in the second

#### Scenario: The frames of one gesture do not stack
- **WHEN** a gesture sends a growing total at one centre and radius over
  several frames
- **THEN** the field and the deformer chain are what a single call at the final
  total leaves

#### Scenario: Resolving is pure
- **WHEN** the preview is asked which nodes a magnify would warp
- **THEN** it names them and the document is unchanged, and the gesture
  afterwards warps the items it named

#### Scenario: What is not a gesture is refused
- **WHEN** a magnify is asked for at a strength of zero, which scales by one,
  or at a radius of zero, which is not a region
- **THEN** the call returns an error rather than recording a deformer that does
  nothing
