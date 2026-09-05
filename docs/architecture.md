# Architecture

How the layers fit together, and why they are arranged this way. The
specification in `openspec/` says *what* the application must do; this says how
it is built and which decisions were forced rather than chosen.

## The engine underneath

ClayCore is a headless C++20 library with a stable C ABI — 610 entry points at
the pin this builds against, covering document and layer authoring, the stroke
engine, voxel grids and their sculpting verbs, fixed-topology mesh sculpting,
subdivision hierarchies and their pass stacks, mask fields, the brick cache,
one chunked transport shared by three surface kinds, a memory ledger, a
maintenance queue, picking, meshing, evaluation and file I/O. Three of its
properties shape everything above.

**Backends are runtime-registered and parity-gated.** CPU is compiled in
unconditionally and *defines correctness*; Metal, Vulkan, CUDA and OpenCL
register only where available and are held to 1e-4 relative on distances
against the CPU scalar reference. So this application never writes a backend
abstraction — it writes a *policy* over one that already exists.

**There is one implementation of every distance function and operator, and it
is the engine's.** The viewport renders meshes the engine produced rather than
evaluating the field itself. This is not a limitation worked around; it is the
protection against a specific bug ClayCore documents having already shipped
once, where a hand-written Metal preview used a smooth-minimum of support `k`
where the engine used `4k`, making every blend four times narrower than the
real field.

Two routes keep that promise, and the difference matters because only one of
them is closed to us. Compiling ClayCore's kernels into our own shader is not
available: the dialect targets MSL, CUDA C, OpenCL C and C++, and not WGSL.
Uploading what the engine already computed *is* available in any shading
language, and is the route ClayCore recommends for a host with no need for the
field between samples — it was proposed from this repository as ClayCore #43
and has been complete since ABI 0.25.0. So the rule above is about *whose
arithmetic decides where the surface is*, not about which pixels we are allowed
to draw. Anything the engine hands over already evaluated — a brick lattice, a
mesh, a lattice displacement — is ours to draw, and a live brush preview is
drawn that way rather than being withheld until the gesture ends.

**The C ABI states its threading contract.** A document is safe to read from
several threads at once; calls on one mutable handle are the host's to
serialise; batched evaluation is free-threaded against a const document. The
safe wrapper expresses this as `Send + !Sync` with a snapshot reader, so a
concurrent mutation is a borrow-check error rather than a race.

## The layers

```mermaid
graph TD
    APP["clayspace-app"]
    MCP["clayspace-mcp"]
    VIEW["clayspace-view"]
    VM["clayspace-vm"]
    MODEL["clayspace-model"]
    ENGINE["clayspace-engine"]
    SAFE["claycore"]
    SYS["claycore-sys"]

    APP --> VIEW
    APP --> MCP
    MCP --> VM
    MCP --> MODEL
    APP --> ENGINE
    APP --> VM
    VIEW --> VM
    VIEW --> MODEL
    VM --> MODEL
    ENGINE --> MODEL
    ENGINE --> SAFE
    SAFE --> SYS

    style MODEL fill:#2E3238,stroke:#C9C4BD,color:#C9C4BD
    style APP fill:#D9744A,stroke:#D9744A,color:#23262B
```

| Crate | Holds | Must not reach |
|---|---|---|
| `claycore-sys` | Generated FFI. No hand-written declarations | — |
| `claycore` | Safe wrapper: ownership, errors, threading | — |
| `clayspace-model` | The domain: tools, interfaces, types | ClayCore |
| `clayspace-engine` | ClayCore-backed implementations | — |
| `clayspace-vm` | ViewModels: observable state and commands | egui, wgpu, winit, ClayCore |
| `clayspace-view` | Widgets and the renderer | ClayCore, directly or transitively |
| `clayspace-mcp` | The agent-facing door: protocol, tool surface, gates | ClayCore, egui, wgpu, winit, `clayspace-view` |
| `clayspace-app` | Composition root, window, event loop | — |

`unsafe` exists in the two bridge crates and nowhere else. Every other crate
declares `#![forbid(unsafe_code)]`, and `tools/check_layering.py` fails if one
drops the declaration or if any forbidden dependency edge appears.

### Why the door sits beside the View and not under it

`clayspace-mcp` is a second reader of ViewModel state and a second emitter of
commands, and it is held to every constraint the View is held to, for the
View's reason: a tool surface that can only be exercised with a window and a
compiled C++ engine is a tool surface nobody tests. It has no `egui`, no
`wgpu`, no `winit` and no ClayCore, so a hundred and thirty command mappings,
the whole protocol and every gate are covered by tests that run in the
ViewModel suite's feedback loop.

The seam is a trait the composition root implements:

```rust
pub trait Session {
    fn apply(&mut self, command: Command) -> Result<Applied, Refusal>;
    fn read(&mut self, query: StateQuery) -> StateReport;
    fn capture(&mut self, request: CaptureRequest) -> Result<Frame, Refusal>;
    fn settle(&mut self, budget: Duration) -> Settled;
    fn measure(&mut self, command: Command) -> Result<Measured, Refusal>;
    fn consent(&mut self, ask: &Consent) -> ConsentOutcome;
    fn gesture_in_progress(&self) -> bool;
}
```

Every method runs **on the interface thread, between frames**, and that is not
a preference. `Observable` holds a `Cell` and the engine's safe wrapper is
`Send + !Sync`, so a connection thread holding a ViewModel or a document behind
a mutex is a borrow-check error rather than a design somebody rejected. A
parsed request becomes a job on a queue, the event loop drains it — bounded, so
a burst from an agent delays itself rather than starving the redraw — and the
answer is sent from *inside* the drain, which is what makes a tool's return
mean the change has happened.

Waking is why `EventLoop::with_user_event` replaced `EventLoop::new`. The loop
sleeps on `ControlFlow::Wait` deliberately; without a proxy to wake it, an
agent's command would sit in the queue until somebody moved the mouse.

**The tool surface cannot drift from the command vocabulary**, because one
function refuses to compile if it does:

```rust
fn home_of(command: &Command) -> Home  // exhaustive, no wildcard arm
```

A new `Command` variant does not build until somebody has given it a group and
an action name, or has said in `Home::NotOffered` why it is not offered. That
is the whole reason the mapping is written by hand rather than derived: a derive
would accept a variant nobody exposed, silently. The three commands that open,
shut and answer the door are themselves `NotOffered` — an agent that could
answer the permission it is being asked for would have made the gate a
formality.

**Any process running as this user can read the connection secret and drive the
session.** That is the stated blast radius. The door binds loopback only, the
secret is new every run and published `0600` in the session directory, the
`Origin` and `Host` headers are checked so a browser page cannot reach it, and
the operations that can destroy work — writing over a file, exporting, opening
a document, discarding unsaved work, quitting — need a consent the secret
cannot supply.

### Why the domain and the adapter are separate

The first arrangement put both in `clayspace-model`. The layering check failed
on its first run: `view → vm → model → claycore` reaches the engine
transitively, which the isolation rule forbids. There is no arrangement of the
remaining crates that fixes that while the domain and engine access share one,
so they were split.

The benefit is not only purity. The ViewModel tests build and run without
compiling the C++ engine, which is the difference between a fast feedback loop
and a slow one.

## What the safe wrapper adds

The C header states its contracts in prose. The wrapper turns them into things
the compiler enforces.

**Ownership.** A voxel grid or mask created standalone is owned and released on
drop; one lent by a document is a *different type*, lifetime-bound to it, with
no destroy operation at all. The engine documents destroying a borrowed handle
as an error; here it does not compile.

**Identity, where a handle cannot be lent.** The C ABI's masked entry points
take a document *and* one of that document's own masks, together. A wrapper
that hands the mask out and then asks for the document mutably cannot be
called — `&mut doc` and `&doc.mask` are the same borrow — so for a long while
the only reachable masks were standalone ones the host made itself, which the
document does not save. `MaskSource` names the mask's **layer** instead: the
resolution happens inside `claycore`, where the two pointers coexist for the
length of one C call and neither escapes. Where a *shared* borrow is enough,
`Document::layer_mask` lends a `MaskLease` and the caller holds it beside
another read of the same document; where two handles are wanted at once,
`Document::voxel_layer_masked` produces both from one borrow. The rule the
wrapper follows is that a borrowed handle never has to escape into a caller's
mutation path for that caller to use it.

**Errors.** Every `clay_result` becomes a `Result`, carrying the engine's
thread-local detail message read *at the point of failure* — before another
call can overwrite it.

**Buffers.** The size-query protocol is wrapped once rather than at each of the
dozens of call sites that use it. There are two of them — a byte-wise retry and
an array-wise one — and neither is offered to a caller as a choice: a wrapper
sizes from the engine's own count and hands back an owned buffer, so nothing
above ever has to tell a short buffer from a bad argument, which are two
different result codes that read alike at a call site.

**Sequences that must happen in an order.** The engine has several pairs where
the second half is the host's to remember: a gesture that must be closed, a
maintenance gate that must be reopened, a memory pin that must be released, a
deferred normal flush that must be handed *the same* undo record the stamps
went into. Each of those is a value whose `Drop` does the second half, and each
of the pairs it guards is unreachable except through it. Two of them are worth
naming because their shape is not obvious. A hierarchy's sculptor borrows the
surface for its own lifetime, which is the header's "the surface must outlive
the sculptor" written as a lifetime rather than as a sentence. And a chunk
acknowledgement is a *type* that only a completed copy can produce, carrying
the revision that copy actually read — so acknowledging a chunk at a revision
nobody read is not something a host can express.

One entry point is emphatically not a size-query call, and treating it as one
applied every stroke twice:

> This is NOT a size-query call: it applies the stroke exactly once, however it
> is called.

The node buffer is sized with `clay_stroke_resolve`, which is pure and
documented for exactly this.

## The sculpting path

```mermaid
sequenceDiagram
    participant V as View
    participant M as SculptViewModel
    participant D as ClayDocument
    participant C as Brick cache
    participant G as SurfaceGeometry

    V->>M: BeginStroke and ContinueStroke
    Note over M: samples accumulate
    V->>M: EndStroke
    M->>D: apply_stroke, whole gesture
    D->>D: apply once, one undo entry
    D->>C: mark dirty by node
    C-->>D: drained keys
    D-->>M: changed plus dirty count
    M-->>V: pending re-mesh
    V->>G: sync
    G->>C: mesh the dirty subset
    C-->>G: mesh plus per-key ranges
    G->>G: dilate, own triangles per key
    G-->>V: upload
```

Four properties are load-bearing:

**The whole gesture arrives at once.** The stroke engine then decides stamp
spacing from arc length rather than from how many samples the device delivered,
and the stroke undoes as one step.

**Dirty is marked by node, not by layer.** A layer's extent is the union of
everything in it; for content spread apart that spans more bricks than any
cache can hold, and the engine refuses such a region. Marking by node bounds
the work to the edit's influence.

**The dirty set comes from the cache's drain.** Diffing surface bricks before
and after finds nothing new after the initial fill, so it falls back to
everything.

**The dirty set is dilated by face neighbours.** A key meshed alone regenerates
the triangles on its boundary while its neighbour still holds the previous
version of the same seam, which shows as a thin crack tracing the edit.

### Getting inside the budget

The specification allows 50 ms median and 100 ms at the 95th percentile from
input to visible. Reaching it took three corrections and one admission:

| Change | Keys per dab | Median |
|---|---|---|
| First version: diff surface bricks | 1043 | 267 ms |
| Take the dirty set from the cache drain | 27 | 26 ms |
| Dilate by face neighbours, to close seams | 81 | 66 ms |
| A realistic brush radius | 32 | **31 ms** |

The last row is the admission. `BrushSettings` defaulted to a radius of `0.38`
because the design's tool bar reads *"Tamanho 38 px"*. Pixels are a screen
measure that maps through the zoom; taken as world units that is a brush
covering a third of a unit-sized model. A detail brush at `0.08` is what the
number means at a normal framing.

### What the gate measures

`just bench` measures one figure group per operation a sculptor can invoke —
every brush on every representation it has a verb for, the layer operations,
rigging and curves, placing and dragging an object, the eight conversions,
consolidation, export, pre-bake repair, mask gating, undo and redo — beside the
five the specification puts a budget on. A subdivision hierarchy has a group of
its own, `multires`, covering the crossing that builds one, the level that
deepens it, a stamp at the sculpt level against the same stamp into a pass, a
composition change, a reorder, a merge and a bake, the save that carries the
sculpt, and a cache release with the dab that pays for it. It builds its own
cage rather than taking a reference member, because a new member changes
`conditions.scenes` and every committed baseline would stop comparing the day
it landed.

Two groups price a seam rather than an operation. `normals` runs one resolved
stroke with the deferred normal flush and the same stroke without it, which is
the only way that pair exists — the application defers unconditionally and has
no switch to turn off — and reports the ratio. `maintenance` prices the moment
between two strokes: the budgeted drain a gesture's end performs, against the
same call with nothing queued. The coverage is derived rather than listed: the brush loop is
`Representation::ALL` against `ToolKind::for_representation`, which is the
table the shelf itself presents from, so a tool added to the shelf is a tool
measured.

Three things make the record trustworthy rather than merely present:

- **A reference suite, revisioned per member.** One scene per representation
  the suite holds a member for — the field, the grid and the carried mesh —
  plus the ten-times variant for locality and a deliberately damaged grid for
  the repairs. A hierarchy has no member on purpose, for the reason above.
  Each names its own revision in the baseline's `conditions`, and a comparison
  against a baseline recorded on a different revision is refused and says which
  member changed. `reference_suite.rs` checks each member still builds the size
  its revision claims.
- **A figure that stops being measured fails the gate.** A measurement that
  quietly returns early looks exactly like one that did not regress, which is
  the thing a performance gate exists to catch. So a measurement says *why* it
  could not run — no headless GPU, a tool with no gesture this harness can
  synthesise — and a baseline figure that is neither measured nor accounted for
  is reported as missing and fails.
- **Figures are reported, not asserted.** Only the specification's five carry
  budgets. Everything else is a tracked quantity compared against the recorded
  baseline; a new figure is not a new promise.
- **A figure carries the spread it was reduced from.** A repeatable measurement
  takes twelve samples and a one-shot three, and the file records the sample
  count, the minimum, the median, the 95th percentile and the maximum beside
  the one number the figure reports. Without it a baseline can say a figure was
  19.04 ms and nothing at all about whether 21 ms is a regression or a Tuesday —
  which is the weakness ClayCore's own device gate names in its release notes.
  A comparison marks a change that lands inside the range the baseline's own
  samples covered, and marks it rather than excusing it: a within-run range is
  the smaller half of the noise, since the variance between runs is larger and
  no single process can sample it. The section is additive, so a baseline
  recorded before it existed still compares and simply cannot say how noisy it
  was.

The `conditions` also name the engine's **revision** — the vendored submodule's
`git describe`, stamped into the binary by `claycore-sys` — and not only its
version. Two builds can both say 0.78.0 and differ by a commit. A comparison
across two engine pins is permitted, since that is the measurement an upgrade
most needs, but it is announced above the table so that no percentage folds an
engine change in silently.

The whole suite is long enough to be worth filtering: `just bench-only brush`
measures one group. A filtered run cannot record a baseline, since a baseline
recorded from a subset reports every omitted figure as missing on the next
comparison.

**A figure carries the machine it was measured on.** The one-minute load is
sampled before the warm-up — after that the load is mostly the benchmark, which
says nothing about who else is competing — reported per core, and written into
the baseline's `conditions`. Above half a runnable thread per core the run
refuses to record a baseline unless `--allow-busy` is passed: a noisy
comparison is wrong once, a noisy baseline is wrong for every run that comes
after it. Both directions are caveated when a comparison goes red, because a
baseline recorded on a loaded box makes every honest run afterwards look like a
regression.

The thresholds are measured rather than chosen. On the reference machine a
concurrent test suite and database — about 0.2 per core — left the move-brush
figure inside 2% across three runs, while an unrelated process at roughly 0.6
per core moved a single measurement by 25%, several times the gate's tolerance.

16-cell bricks were also tried, and are worse: a third as many keys but eight
times the cells each, so a dilated set meshes more overall — 64 ms against 39.

### Two costs that were paid per model rather than per edit

Both are gone, and both had been sitting behind a correct-looking incremental
path. A dab dirties about 27 bricks; before these, it re-meshed 200 and
rewrote the whole GPU buffer.

**The dilation ring.** The dirty bricks used to be dilated by one before
meshing, because a subset mesh omitted triangles straddling its boundary and
left seams. ClayCore 0.28.0 fixed that (#66) — a subset now returns every
triangle with a corner in a requested brick — and the ring stayed on out of
habit. Removing it changed nothing a rebuild comparison can see and cut the
keys per dab from 200 to 27.

**Whole-buffer upload.** Assembly used to be concatenation, on the grounds
that the engine welds vertices across brick seams so a key's range cannot move
independently. That is true of the engine's output and not of ours: the
per-key split gives every key its own local vertex table, so a key's geometry
is self-contained and its span can be placed anywhere. Each key now keeps a
span in both buffers (`clayspace-app/src/slots.rs`) and a dab writes only the
spans it touched.

Spans carry a quarter of headroom so a brick re-meshing slightly larger stays
put; one that outgrows its span is re-homed at the end and the abandoned
indices are filled with degenerate triangles, since the surface is a single
draw call over a single range. Holes accumulate, so the whole surface is laid
out again once a fifth of the drawn range is holes.

Index spans are rounded to a multiple of three. The rasteriser groups indices
into triangles by position from the start of the draw range and knows nothing
about spans, so a span ending mid-triangle re-cuts every triangle after it —
which renders as speckle over the entire model rather than damage anywhere
nameable.

A median dab, at 286k triangles:

| stage | before | after |
| --- | --- | --- |
| keys re-meshed | 200 | 27 |
| engine: apply stroke + refill | 0.7 ms | 0.6 ms |
| engine: brick cache mesh | 5.8 ms | 1.8 ms |
| ours: copy into vertex layout | 0.1 ms | 0.1 ms |
| ours: split into per-key geometry | 2.2 ms | 0.5 ms |
| ours: write to the GPU | 3.1 ms | 0.2 ms |
| **total** | **12.0 ms** | **3.1 ms** |

The upload is now the smallest term and no longer scales with the model, which
is the property that matters: it is the edit that is paid for, not the sculpt.
`dab_profile.rs` fails if it ever dominates again.

## MVVM, mechanically

A View function is a pure function of ViewModel state that emits commands. It
cannot mutate, call the Model, or perform I/O — and cannot reach the engine to
try.

```mermaid
graph LR
    STATE["ViewModel state"] --> VIEWFN["View function"]
    VIEWFN --> CMD["Command"]
    CMD --> DISPATCH["dispatch"]
    DISPATCH --> MODELCALL["Model"]
    MODELCALL --> STATE

    style CMD fill:#D9744A,stroke:#D9744A,color:#23262B
```

Commands are values rather than closures, so a menu item, a keyboard shortcut
and a panel button that mean the same thing emit the *same* command and cannot
drift apart. `Command::touches_document` decides in one place that view
changes never enter the undo history.

Observable state carries a revision. Reading never marks anything dirty, and
setting a control to the value it already holds is not a change — which matters
because an immediate-mode interface does both constantly.

Work that outlasts a frame goes to a job runner that never blocks the interface
thread and discards a result whose generation is behind the current one. A
stale export writing itself over a newer document is the failure that exists to
prevent.

## Testing

| Kind | Where | What it checks |
|---|---|---|
| Bridge | `claycore/tests` | Authoring, meshing, picking, save and reload against every registered backend |
| Domain | `clayspace-model` | Tool availability, brush clamping, protection states |
| ViewModel | `clayspace-vm/tests` | The interface rules, against a double, with no engine |
| Session | `clayspace-engine/tests` | The same rules against a real document |
| Latency | `clayspace-app/tests` | Dab cost against the budget |
| Performance | `clayspace-app/src/bin/bench` | Every operation a sculptor can invoke, against a recorded baseline |
| Visual | `clayspace-app/tests` | Real frames, written as PNGs |

### A fixture that cannot be built is a failure, not a skip

The session tests used to open with `let Some(mut document) = document() else {
return; };`, and the helper behind it swallowed every error with `.ok()?`. A
regression in `ClayDocument::new` or `with_starting_form` therefore turned whole
files green while asserting nothing — measured: breaking `with_starting_form`
left `curve.rs` reporting 11 passed. They now build with `.expect`, so the same
break fails all eleven and names the call that could not be made.

The distinction is what the failure would mean. `BackendPolicy::discover` cannot
fail for want of a candidate and a CPU document always builds, so a refusal
there is a bug and must be loud. A machine with no accelerated backend, or no
window server, is a fact about the machine: those tests still return early, and
`window_smoke` turns its skip into a panic when `CLAYSPACE_REQUIRE_WINDOW` is
set.

The visual tests are the unusual ones. They render a real frame on a real
device and write it to `target/visual/`, with assertions deliberately coarse —
"something was drawn", "these two differ", "this is dimmer than that" — because
a pixel-exact golden would fail on every driver and say nothing about whether
the picture is right.

### What the captures caught

Four bugs that the assertions passed straight over:

**Overlays rendered fully transparent.** The overlay shader took its alpha from
the material uniform's alpha channel, which carries the vertex-colour flag for
the surface pipeline and means nothing for a line.

**Overlays then rendered several times too bright**, competing with the
silhouette they are meant to sit behind. The design's hex palette was written
straight into an sRGB target as if it were linear.

**A crack along every brick boundary.** Per-key geometry stored only a key's
own vertex range and dropped triangles reaching outside it — but the engine
welds across seams, so a great many do. The capture showed a grid of holes; no
key count or timing would have named it.

**A stroke that deposited four specks.** Its stamps sat inside a radius-1 ball
and were swallowed by it.

A fifth was caught by running the app rather than the tests: the window aborted
on its first presented frame because the surface and the device came from two
different `wgpu::Instance`s. The offscreen tests never build a surface, so they
structurally could not have found it. `window_smoke` now requires three
presented frames and has been verified against the regression.

### The scaffolding reads depth without owning any

The manipulator is drawn after the occlusion composite, into the resolved
target, and binds no depth attachment — that is what keeps occlusion off it.
Which left it with no way to know what stands in front of it, so a rotate ring
was drawn at one strength all the way round and gave no sense of which half was
nearer.

It **samples** depth rather than testing against it. Attaching the framebuffer's
depth would mean matching its sample count, so either resolving depth into a new
texture or moving the draw back inside the scene pass — and moving it back is
the one thing the pass order exists to prevent. Sampling the multisampled depth
directly forks the shader on whether the device multisamples, which is the exact
fork the reduced-depth buffer was introduced to remove.

So it reads `Framebuffer::reduced_depth`: already single-sampled, already
`R32Float`, already `TEXTURE_BINDING`, and already holding this frame's depth
for the occlusion kernel. Half resolution, which costs a display pixel of slop
in where faint begins — a low-frequency decision about which half of a ring is
behind a head, and the same argument the kernel makes for running there.

Two things fell out of it rather than being arranged, and both are worth knowing:

**The reduction had to leave the occlusion gate.** It was the first pass inside
`occlude`, which returns early when occlusion is off. Read from where it sat, a
manipulator would have been dimmed only while occlusion happened to be enabled —
a widget whose appearance depends on the occlusion setting, which is precisely
what `occlusion_does_not_darken_the_manipulator` forbids. Putting that gate back
fails that test, which is how the requirement was checked rather than assumed.

**A ghosted surface dims nothing, for free.** The ghost pipelines write no depth
— "what lets the far half of the cage read through the form", in their own words
— so while a cage is up the depth buffer is empty and the scaffolding is drawn
exactly as it always was. The rule stays the single uniform one, with no
exception list for the cage that could rot.

What did change is the pass-order invariant's *scope*. A faint pixel is forty
percent widget over sixty percent form, and that form is legitimately shaded, so
such a pixel does darken with occlusion. The invariant is now stated over the
pixels the manipulator covers **opaquely** — read off the frame by comparing
against the same widget drawn with nothing behind it — where 0 of 5382 pixels
darken against 12% of the form.

### What the captures hid

The shell captures run egui several passes deep, because a menu does not exist
until the frame after the button that opens it was clicked. A glyph reaches
egui's font atlas in the pass that first *lays it out*, arriving as a
`textures_delta` on that pass's output — and the harness applied the deltas of
only the first and the last pass. So any accented character appearing **only**
inside a menu arrived in a discarded delta and drew as a blank: every menu
capture on this project was quietly missing its accents, and "Mostrar só esta"
had been reading as "Mostrar s esta" in the images used to eyeball the design.
Nothing failed, because nothing asserts on glyphs. Every pass's deltas are
applied now.

The menu was translucent in those images too, and for a neighbouring reason:
egui fades a popup in over a twelfth of a second, and each harness pass advanced
its clock by a sixtieth — so a capture two passes after the click caught the menu
at a little over half opacity, with the layer rows behind it reading straight
through the fill. The settling passes now declare a quarter of a second as their
own duration, which leaves every animation finished.

Both have regression tests, and both tests were checked by putting the bug back:
the fade one measures the fill where the menu overhangs the panel it opened from,
against the closed capture at the same pixel, so it can tell a transparent fill
from an opaque one; the glyph one captures the same menu twice, once with its
labels laid out in the first pass — the delta that was never dropped, in every
text style, since a glyph is rasterised per size — and requires the two pictures
to be identical. The accent is 46 pixels.

The lesson generalises past fonts. These captures exist to be looked at, so a
harness that renders something *other* than what the application renders is
worse than a missing test: it answers the question it was asked, wrongly, and
the answer looks like a picture.

## Decisions recorded elsewhere

`openspec/changes/add-clayspace-desktop/design.md` carries the full decision
record, including the alternatives considered. The ones most likely to be
revisited:

- **Meshing rather than a brick-volume raymarch** for the viewport. The volume
  path needs no kernel math in the shader and removes meshing from the
  interaction path; it is the first thing to try if meshing dominates.
- **No GPU device injection.** ClayCore 0.26.0 offers it, but the device path
  bypasses the brick cache, so taking it means reimplementing generations,
  staleness, quantization and the memory budget. A bounded copy is cheaper for
  now.
- **No soft-body dynamics.** The design's *Dinâmica* panel shows gravity,
  rigidity and damping; ClayCore has no solver and none is planned.
