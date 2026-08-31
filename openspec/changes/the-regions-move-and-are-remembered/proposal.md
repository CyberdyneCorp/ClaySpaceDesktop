# The regions move, and are remembered

## Why

`app-shell` has carried this requirement since the application was specified:

> The user SHALL be able to resize and collapse each panel region and restore
> the default layout in one action. Layout SHALL persist across sessions.

with two scenarios under it. None of it was true. The regions were drawn at
fixed widths with `exact_width`, the Janela menu was declared and left empty —
`ui.menu_button(s.menu_window, |_| {})` — and `clayspace-view`'s `layout`
module, which carries the sizes, the minimums, the maximums, the collapse
state, a reset and a pair of serialisers written for "a line the composition
root can store", was exported to **no consumer at all**. Its own tests passed
throughout.

So a sculptor could neither drag a panel wider nor put one away, and the
design's arrangement was the only arrangement.

## What Changes

- **The left region, the right region and the shelf are resizable**, clamped to
  the bounds `layout` already declared.
- **Each can be put away and brought back from the Janela menu**, which now has
  contents, with a reset that returns every region to the design's own size.
- **The arrangement is stored** beside the recent documents and the locale, in
  the per-user directory `SessionStore` already owns, and read at start-up.

## Corrections to the record

Two things this change disproves, both of which I had written down as facts in
earlier changes on this branch:

- **"There is nowhere to persist a preference."** There is: `SessionStore`,
  which has been keeping the recent list, the chosen locale, the recovery
  marker and the remembered reference images since it was written. The layout
  goes in beside them, and the shelf's favourites and the viewport profile can
  follow the same route.
- **"The guide says the repository already has resizable, collapsible
  regions."** The guide's own audit says so and it was wrong. That is why its
  later phases — focus mode in particular, which is specified as not destroying
  a saved layout — read as though the groundwork existed.

## Decisions worth stating

**A reset has to reach egui as well as the stored line.** `default_width` is a
default: egui keeps each panel's width in its own memory and holds it against
the value passed on later frames. Resetting the stored sizes alone moved
nothing on screen. The remembered `PanelState` is dropped when a reset is asked
for, which is what makes the default apply again.

**The arrangement is written when it changes, not every frame.** egui reports a
region's width on every frame it draws; comparing against the stored size first
means the file is written for the handful of drags a sculptor made rather than
thousands of times a session.

**A toggle is read and removed rather than left in memory.** A value that stays
there is a toggle applied once per frame for as long as it sits, which is a
region that flickers instead of one that closes.

**Neither the resize nor the toggle is a command.** The arrangement of the
regions touches no document and enters no history — and `Panel` is a view type
that a command in the layer below could not carry. It goes the way the section
folds, the shelf's filter and the viewport profile go.

**The capture harness honours collapse but keeps exact widths.** It is a second
copy of the composition root's frame, and the lesson from the representation
bar is that the two must stay in step — but a capture is compared at a known
size, and a panel egui had remembered a drag on would make one capture
incomparable with the next.

## Out of scope

- **Focus mode**, which is now unblocked and is the guide's PR 7. It wants a
  temporary presentation override that hides regions *without* disturbing the
  stored arrangement, which is a distinct piece of state.
- **The shelf's favourites** and **persisting the viewport profile**, both of
  which can now follow the route this opens.
