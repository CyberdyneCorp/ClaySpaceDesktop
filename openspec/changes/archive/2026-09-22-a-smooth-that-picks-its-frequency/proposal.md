# A smooth that picks its frequency

The shelf has told a sculptor, for as long as the hierarchy tier has existed,
that a smooth on one picks a frequency: the form, the detail alone, or the form
with the detail carried through unchanged. No stroke ever picked one.

Every smooth on a hierarchy went out as a stamp carrying `MeshBrush::Smooth`,
which is `SmoothMode::Geometry` — a plain Laplacian over the evaluated
positions. That is exactly what a mesh does, and what it does over pores is
remove them. The mode the note describes as the useful one, the form corrected
*under* the detail with the detail put back unchanged, was never requested; it
is also the one operation this representation can perform and a flat mesh
cannot, because a mesh has one surface and nothing stored beneath it.

The engine and the wrapper were both ready. `clay_multires_sculpt_layer_stroke_smooth`
is in the pinned header, `SculptLayerStroke::smooth(mode, …)` binds it, and the
wrapper's own suite already compares all three modes and hashes their results.
The application simply did not call it: `stamp_into_a_pass` opened the layered
stroke transaction and then only ever stamped. The capability table named the
right entry point all along, which is what made the gap invisible — the table
said `clay_multires_sculpt_layer_stroke_smooth` and the code went somewhere
else.

## What this changes

- **A smooth on a hierarchy goes through the layered stroke transaction**, which
  is where the mode lives, rather than through a stamp with a smoothing brush in
  it. This is true whether or not a pass is active: with one, the write domain is
  the pass; without, it is the form under them.
- **The sculptor chooses**, on the smooth tool and on a hierarchy alone: the
  form, the detail only, or the form with the detail. The other three
  representations store one surface and so have one smooth, and a three-way
  control over them would decide nothing.
- **The default is the form with the detail** — what the tool's note has always
  promised, and what an artist correcting anatomy under pores is asking for.
- **The plain Laplacian stays reachable.** "I want the pores gone" is an
  ordinary thing to want and it is the only mode that does it.
- **`state` reports the chosen mode** in its tool section, on a hierarchy and
  nowhere else, and an agent sets it through `hierarchy.smooth_mode`.

## What this deliberately does not do

**No engine work, and no new FFI.** Every call this needs has been bound since
the wrapper's multires module was written; the change is the application
reaching them.

**It does not change what any other verb does on a hierarchy.** Draw, Grab,
Inflate and the rest still stamp, into the pass or into the form exactly as
before. Only the smooth has more than one thing it could mean.

**It does not make a pass stroke resolved.** The transaction offers stamps and
no resolver, so a smooth through it is stamped sample by sample for the same
reason a pass stroke already is — the samples arrive about one dab apart, so
what is lost is the jitter and the taper rather than the coverage.

## Capabilities

### Modified Capabilities
- `sculpting-tools`: the smooth on a hierarchy gains the choice its note has
  always described, with a stated default and a stated write domain.

## Impact

**Code**: `clayspace-model` (the three frequencies as a domain type, and the
model trait's session setting beside the combine operation), `clayspace-engine`
(`document.rs`: the smooth routed through `SculptLayerStroke::smooth`),
`clayspace-vm` (the choice and whether it is offered), `clayspace-view` (the
control on the options bar, in three locales), `clayspace-mcp` (the mode in the
tool section of `state`, and the action that sets it).

**Risk**: low and bounded to one verb on one representation. The regression
that matters is measured rather than argued — the detail deposited in a pass is
subtracted against an otherwise identical hierarchy before and after the
stroke, which fails on the old code at 0.0135 becoming 0.000027 and holds under
the new one.
