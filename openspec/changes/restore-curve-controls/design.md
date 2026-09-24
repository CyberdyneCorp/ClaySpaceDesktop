# Design

A curve's selected points have a centroid. The object transform interface exposes that centroid as a transform with identity rotation and scale. At gesture start the document stores the selected indices and positions. Each requested transform maps those original positions through scale, axis-angle rotation and translation about the centroid. This avoids frame-by-frame drift. The object ViewModel's existing gesture grouping makes the drag one undo step; the desktop chooses the curve target when a transform mode is requested while a curve is active.

A circle uses the stroke item's per-point radii. Other profiles store their sizes on the placed swept item, so changing radii requires replacing that item. Removal and insertion are grouped as one engine history entry. The curve tracks each placed node's profile so undo and redo can restore the control panel along with the engine node. Resync searches both nodes involved in a replacement.

Inactive curve edits return a model refusal. A curve target without selected controls is refused at the ViewModel boundary. Tests exercise the engine transform path and the ViewModel's command route, radius/profile updates, undo and redo, and inactive commands.
