# The shelf can be browsed, without offering a click that does nothing

## Why

The shelf shows the brushes the active layer can be sculpted with, and nothing
else. That is the right default and this change does not touch it. What it
cannot do is answer the question the representation bar above it now raises: a
sculptor who can see that Voxels and Mesh exist, and what crossing to one would
cost, still has no way to find out what they would *get* — which brushes that
representation has — short of converting and looking.

## What Changes

- **A filter column at the shelf's leading edge**: `Disponíveis`, then one
  entry per representation. The first is the sculpt workflow, unchanged and
  selected by default.
- **Browsing lists another representation's vocabulary**, with every brush the
  active layer has no verb for drawn dim and refusing to be picked, with the
  reason on hover.
- **The chosen row wears the same mark the active layer does** — an accent rail
  over a raised surface — so the shelf and the stack answer "which of these is
  selected?" in one grammar.
- **The filter is interface state.** It lives in egui's own memory beside the
  section folds, emits no command, and is forgotten when the application
  closes.

## The requirement this changes, and why

`make-representations-first-class` states:

> A tool that has no verb on the active representation SHALL NOT be offered.
>
> #### Scenario: A tool with no verb here is absent
> - **WHEN** a representation has no verb for a tool
> - **THEN** the tool is not shown in the shelf for that layer

The rule was defending against a shelf that is mostly grey — with three
representations carrying substantially different vocabularies, one undivided
list would be disabled entries whatever the layer, all saying the same thing.
That defence is intact: the shelf *offers* exactly what it offered before, and
what it offers is still derived from the one declared table.

What the rule did not distinguish is **offering** a tool from **showing** one
in answer to a direct question. Choosing `Mesh` on a field layer is a sculptor
asking what a mesh layer has. Answering with a list they cannot select from is
not a shelf full of dead controls; it is the answer to the question they asked,
and it is reachable only because they asked it.

So the requirement is narrowed rather than dropped: absent from the shelf's
*offer*, showable while explicitly browsing, and never selectable. A brush that
could be clicked to no effect is the failure the original rule and this one
both refuse.

## Out of scope, and why

- **Favourites.** The guide asks for a `★` filter persisted as a user
  preference. There is no preference store to persist it in: `layout.rs` — the
  panel sizes and collapse state the design specifies — is exported from
  `clayspace-view` and used by nothing, and the regions are drawn at fixed
  widths. A favourites list that forgot itself every launch is a promise the
  application would break each time it opened, so the star waits for the store.
  Worth its own change, alongside wiring `layout.rs` to something.
- **Previews rendered from the real brush behaviour.** The guide files this
  under "eventually". Each brush already carries a distinct drawn mark and a
  test asserts no two are alike; rendering thirteen canonical sphere strokes
  per shelf is a cost that should be measured before it is paid.
- **Preview texture caching.** Nothing to cache yet: the swatch is painted as
  shapes, not built as a texture. The material previews, which *are* textures,
  are already cached on first use.
