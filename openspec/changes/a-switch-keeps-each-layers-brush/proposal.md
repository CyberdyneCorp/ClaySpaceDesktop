# A switch keeps each layer's brush, and a substitute is the table's

## Why

#162 put the scene first in the dispatch order, so the sculpting ViewModel
reads the new active layer when a switch reaches it. That was the ordering
half. This is the settings and substitution half (#217), and it is what the
plan asked for beside it:

- retain the current tool if the new representation supports it;
- otherwise select a sensible supported substitute;
- report the substitution;
- preserve settings per tool *and* representation.

Three of those were only partly true:

- **The substitute was a hand-written choice.** A tool with no verb on the new
  representation was replaced by whatever the shelf listed first. Raspar
  moved to a field became Padrão, a deposit, although the field has Planar —
  the same act. Now that each binding carries its `SemanticIntent` (#205) the
  table can answer the question directly.
- **The substitution was a marker.** The status line said "tool changed", and
  the agent door carried the bare marker `tool-substituted` as a remark: an
  agent learned that *something* had been swapped, not what for what, and
  `state` had no way to say that the tool it reported was given rather than
  chosen.
- **A slot nothing had been set in held one global default.** Settings were
  already per (tool, representation), but what an unset slot held was the
  same `BrushSettings::default()` everywhere, with no statement of why that
  number suits each representation.

The issue's premise that a grid is measured in different units from a field is
not quite what the code does: since `grid-brush-radius` a brush size is a
radius in document units on every representation, and the grid's footprint is
converted from it. What *was* wrong is a value carried from one
representation's slot into another's, which #162 stopped; this change makes
the default each slot starts from a documented, per-representation value so
that nothing is left to carry.

## What Changes

- `ToolKind::substitute_on(representation)` reads the substitute off the
  capability table: the tool itself where it is offered; else a tool with the
  same intent that is the representation's native verb for it; else one with
  the same intent at any fidelity; else Padrão, which every representation
  lists first. A candidate driven by a different gesture (Trim against a
  stroke) or one that refuses the layer until a pass is selected (a
  hierarchy's eraser) is never a substitute. Deterministic: tool and
  representation alone decide it.
- The sculpting ViewModel remembers the tool that was *chosen* while a
  stand-in is in hand. A switch back to a layer that carries it returns it;
  choosing any tool ends the stand-in.
- `BrushSettings::default_for(representation)` is the documented default a
  slot starts from. The four agree today (0.18), and each arm states why.
- Over the agent door the switching command's answer names both tools by
  their keys, and `state.tool.stands_in_for` carries the chosen tool's key for
  as long as a stand-in is in hand.

## What this deliberately does not do

- **It is not written into `typed-capability-bindings`.** The issue suggests
  the rule lives there because the substitute is derived from the binding
  table. That change's own proposal says behaviour on the shelf is unchanged,
  and it has landed; this one changes behaviour, so it is its own change and
  cites the table rather than amending a finished record.
- **No new `ToolNote`.** The issue suggests one where a substitution is
  systematic. Every substitution is now read off the table and listed in
  `docs/features.md`, and the status line already names the event; a note per
  (tool, representation) pair that is *absent* would be a second vocabulary
  for what the table says. Left for the advanced-help view.
- **No change to the shelf.** The same tools reach the same representations
  through the same calls.

## Impact

- `representation-modes`: substitution from the table, and per-representation
  defaults.
- `agent-observation`: the tool section says when it is a stand-in.
- `crates/clayspace-model/src/tools.rs`, `crates/clayspace-vm/src/sculpt_vm.rs`,
  `crates/clayspace-mcp/src/{report,session}.rs`,
  `crates/clayspace-app/src/main.rs`; `docs/features.md`.
