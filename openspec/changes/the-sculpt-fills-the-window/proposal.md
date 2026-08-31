# The sculpt fills the window, and the stroke's numbers are on one bar

## Why

The redesign guide states its target as a rule — the viewport should hold most
of a sculptor's attention — and nothing in the application let them find out.
Every panel was on screen at all times. The guide's own answer, focus mode, was
its last unbuilt PR, and it had been blocked on the arrangement of the regions
not being expressible; that was fixed in
`the-regions-move-and-are-remembered`, and this is what it unblocked.

Three smaller gaps came with it:

- **The stroke's settings were spread across three regions.** Intensity, size
  and flow are on the options bar; the smoothing was in a right-panel section;
  symmetry was the sole content of a left-panel section. All three shape the
  stroke being made.
- **The shelf had no favourites**, which the store can now hold.
- **The viewport quality profile reset every launch**, likewise.

## What Changes

- **Focus mode**, on `Tab` and in the Janela menu. It clears the tool rail, the
  options bar, the representation bar, both inspectors, the shelf and the status
  area, and leaves the menu bar, the view presets and the sculpt.
- **A floating brush readout** while focus is on: the brush's own ball and mark,
  its name, the representation the stroke will land on, and its size, intensity
  and flow.
- **The smoothing and the symmetry axes move to the options bar.** Moved, not
  copied — the sections they came from lose them, and the left region's
  sculpt-settings section held nothing else and is gone.
- **Favourites**, starred from a brush's own menu and kept in the store, with a
  `★` filter beside the representation filters.
- **The viewport profile is remembered.**

## Decisions worth stating

**Focus is a presentation override, not a layout.** It hides the regions
*without* touching the sizes and collapse states a sculptor chose, so leaving it
puts everything back as it was. That is why it is a bool beside the layout
rather than three more collapse flags inside it, and why it is not stored: an
application that opened with its panels gone would look broken.

**The menu bar stays.** A mode a sculptor cannot find their way out of is worse
than no mode, and `Tab` is not discoverable from an empty window — so the menu
that offers focus mode is also the one still on screen while it is on.

**The brush readout is what makes focus usable rather than blind.** The options
bar carries the size and the intensity; hiding it without replacing them would
be focus in name only.

**The falloff shape did not move.** It was moved to the bar, and looking at the
result showed the bar had overflowed: the colour swatch, which had been visible,
was scrolled out of view. Four edge profiles need two hundred pixels between
them, an edge is chosen occasionally, and a smoothing is dialled while drawing —
so the smoothing went and the edge stayed. The sliders narrowed from 150 to 128
to make room for the fourth.

**An engaged symmetry axis wears the soft accent.** `chip` lifts an engaged
control to the raised surface, which is a three-and-a-half-per-cent step and
reads as "hovered" as much as "on". Fine for a view preset; not for symmetry,
where a mirrored stroke nobody expected is the most expensive surprise on the
bar.

**A favourite is starred from the brush's own menu**, which is the gesture a
layer row already uses for what is not its primary click. A brush met while
browsing another representation can be starred too: a shortlist is for finding a
brush again, and which layer it applies to is a separate question the swatch
already answers by refusing the click.

**Both new preferences carry a stable key.** `ToolKind::key` and
`ViewportProfile::key`, for the reason `Shape::key` already gives: a label is
interface text and would read differently in another language, and a position in
`ALL` is presentation order that reordering would silently reinterpret.

## Out of scope, and why

- **X-ray occlusion for the manipulator.** Unchanged and recorded in
  `transform-widget-presentation`: the pass that draws it binds no depth
  attachment and runs after the multisample resolve.
- **The orientation cube.** Still three coloured axis rods rather than a
  labelled cube. A distinct piece of work in the overlay geometry.
- **A viewport HUD naming the representation.** The representation bar sits
  directly above the viewport and the brush readout names it in focus mode; a
  third place would be repetition.
- **Gizmo World/Local, Sculpt Pivot, and a Mirror toggle.** Audited: there is no
  local-space transform in the domain, nothing resembling a sculpt pivot, and
  the concept's "Mirror" is the symmetry that already exists — a second control
  for it would be a duplicate.
- **Brush previews rendered from real brush behaviour**, the custom title bar,
  and rail labels.
