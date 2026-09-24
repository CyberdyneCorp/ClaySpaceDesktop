# Design

ClayCore's `first_visible_flip_bound` in `scene/commands.cpp` states that the first visible SDF layer initializes the field. A composed layer directly above it changes over its own influence bound when promoted or demoted. The helper is internal to ClayCore and cannot be called across the C ABI.

The desktop records the first visible SDF layer before a visibility, remove, or reorder command and compares it with the first afterward. If they differ, it marks the surviving old and new first layers when their composition is not a hard union. The same predicate as ClayCore's `layer_composition_is_hard_union` is read from `clay_document_layer_composition`. The cache's `mark_dirty_layer` supplies the engine's influence bound, including fold support. Ordinary commands still mark their own regions, and all marks are drained together where the existing path already does so.

Adding an empty SDF layer cannot change an existing first visible layer. A newly added first layer has no material until an edit, and that edit marks its own region. Conversion paths that add populated layers already refill the new layer.
