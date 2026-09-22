# mvvm-architecture Specification

## Purpose
The layering the workspace is built on — Model, ViewModel, View — the single
command path every mutation flows through, and the edges that keep the engine
out of the View, the interface out of the ViewModel, and the wiring in one
composition root.

## Requirements

### Requirement: The workspace is layered Model, ViewModel, View
The application SHALL be organized into crates with a strict dependency direction: `clayspace-model` (the domain) ← `clayspace-vm` (ViewModels) ← `clayspace-view` (interface and rendering) ← `clayspace-app` (composition root). No crate SHALL depend on a crate later in that order.

Engine access SHALL live in a separate `clayspace-engine` crate that depends on the domain and on ClayCore, and on which only the composition root depends. The domain SHALL NOT depend on ClayCore.

This separation is what makes the View's isolation achievable: a single crate holding both the domain and engine access would put ClayCore in the transitive dependencies of every layer above it, and no arrangement of the remaining crates could satisfy the isolation requirement below.

A crate that offers the application to something other than a person — the
agent-facing server — SHALL sit beside the View rather than under it: it MAY
depend on the domain and on the ViewModels, and SHALL NOT depend on ClayCore,
on the interface and rendering crate, or on any windowing, drawing or input
library. It is a second reader of ViewModel state and a second emitter of
commands, and it is subject to every constraint the View is subject to for the
same reason — so that the tool surface is exercisable in a test with no window,
no display, no GPU, and no C++ engine built.

#### Scenario: Dependency direction holds
- **WHEN** the workspace dependency graph is inspected
- **THEN** no edge runs from a Model crate to a ViewModel crate, or from a ViewModel crate to a View crate

#### Scenario: The domain is free of the engine
- **WHEN** the domain crate is built
- **THEN** it compiles without ClayCore present, and the ViewModel tests run without the engine being built at all

#### Scenario: The agent-facing crate is held to the View's isolation
- **WHEN** a dependency on ClayCore, on the View crate, or on a windowing,
  drawing or input library is added to the agent-facing crate
- **THEN** the architecture check in CI fails, naming the forbidden edge

#### Scenario: The tool surface is testable headlessly
- **WHEN** the agent-facing crate's tests run in a headless environment
- **THEN** every tool can be dispatched and asserted on with no display server,
  no GPU and no engine build

### Requirement: The View layer cannot reach the engine
`clayspace-view` SHALL NOT depend on `claycore` or `claycore-sys`, directly or transitively. No ClayCore type, handle, enum, or error SHALL appear in the View layer's API or implementation.

#### Scenario: Engine dependency in the View fails CI
- **WHEN** a dependency on `claycore` or `claycore-sys` is added to `clayspace-view`, directly or through any intermediate crate
- **THEN** the architecture check in CI fails, naming the forbidden edge

#### Scenario: Engine data reaches the View as plain values
- **WHEN** the View displays polygon, vertex and triangle counts for the current document
- **THEN** it reads plain numeric fields from a ViewModel, not an engine handle or a mesh object

### Requirement: The ViewModel layer is free of interface and rendering dependencies
`clayspace-vm` SHALL NOT depend on `egui`, `wgpu`, `winit`, or any other windowing, drawing or input library. ViewModels SHALL be constructible and exercisable in a test with no window, no display and no GPU.

#### Scenario: ViewModels are testable headlessly
- **WHEN** the ViewModel test suite runs in a headless environment
- **THEN** every ViewModel can be constructed, driven through commands, and asserted on, with no display server or GPU present

### Requirement: A View is a pure function of ViewModel state that emits commands
Every View function SHALL take ViewModel state by shared reference and SHALL affect the application only by emitting commands. A View SHALL NOT mutate ViewModel state, call the Model, perform I/O, or hold state that outlives a frame beyond transient interaction state such as a drag in progress or a scroll offset.

#### Scenario: A click produces a command, not a mutation
- **WHEN** the user clicks a brush in the brush shelf
- **THEN** the View emits a select-brush command and mutates nothing itself

#### Scenario: View state is reconstructible
- **WHEN** the interface is rebuilt from ViewModel state after a restart with the same document and session state
- **THEN** the interface presents identically, because no durable state lived only in the View

### Requirement: All mutations flow through a single command path
Every change to application or document state SHALL be expressed as a command dispatched through one command path. The command path SHALL be the only place where Model mutations are initiated.

This holds for every source of commands, not for the interface alone. A source
that is not a View — an agent-facing server, a script, a test harness — SHALL
emit the same commands the interface emits and SHALL dispatch them through the
same path. No source SHALL hold a mutation route of its own, and no source
SHALL reach a Model interface directly.

A command SHALL therefore be indistinguishable by its effect from the same
command emitted anywhere else: the same history entry, the same observable
state change, the same conditions disabling it, and the same refusal where the
Model refuses it.

Commands from every source SHALL be applied on the interface thread in the
order they arrived, so that no source can interleave within another's command.

#### Scenario: One place to observe every mutation
- **WHEN** the command path is instrumented in a debug build
- **THEN** every document and application state change appears in that instrumentation, whatever emitted it

#### Scenario: Commands are independent of their source
- **WHEN** the same command is dispatched from a menu item, a keyboard shortcut, a panel button, and an agent's tool call
- **THEN** the resulting state change is identical in all four cases

#### Scenario: A non-interface source has no shortcut
- **WHEN** the agent-facing crate's dependencies and API are inspected
- **THEN** it can reach the application only by emitting commands and reading ViewModel state, exactly as a View can

### Requirement: Long-running work does not block the interface
Commands whose Model work can exceed one frame — meshing, baking, consolidation, import, export, save — SHALL execute off the interface thread. The ViewModel SHALL expose their progress and completion as observable state, and the interface SHALL remain responsive while they run.

#### Scenario: Export keeps the window responsive
- **WHEN** the user exports a high-resolution mesh
- **THEN** the interface continues to redraw, the viewport continues to respond to camera input, and progress is displayed

#### Scenario: A stale result is discarded
- **WHEN** an asynchronous result arrives for a document state that has since been superseded
- **THEN** the result is discarded and the newer state is not overwritten

### Requirement: ViewModel state changes are observable
Each ViewModel SHALL expose a change signal that lets the interface redraw only when its state has actually changed. Reading ViewModel state SHALL NOT itself cause a change notification.

#### Scenario: An idle application does not redraw continuously
- **WHEN** no input arrives, no command is dispatched, and no asynchronous work completes
- **THEN** no ViewModel reports a change and the interface does not schedule a redraw for ViewModel reasons

### Requirement: The composition root is the only place that wires layers together
`clayspace-app` SHALL construct the engine bridge, the Model, the ViewModels, the renderer and the window, and SHALL inject dependencies downward. No other crate SHALL construct a layer other than its own.

#### Scenario: A ViewModel receives its Model rather than creating one
- **WHEN** a ViewModel is constructed in a test
- **THEN** it accepts a Model interface as a parameter, allowing a test double to be supplied in place of the engine-backed implementation

### Requirement: An operation the composition root runs states its refusal where the interface reads it
A few operations are run by the composition root rather than dispatched to a
ViewModel, because each carries an answer back rather than a `Result<(), _>`.
Every one of them SHALL write its refusal to an observable channel the
interface draws and the agent door counts, on the same terms as a ViewModel's
own refusal: announced rather than set, so that the same impossible request
asked twice is two refusals and not one, while the words on screen do not
redraw for a repeat.

The refusal SHALL be cleared by the next operation that works, so the line
belongs to the last thing that was asked rather than to the last thing that
failed.

No operation's outcome SHALL be discarded — by `.is_ok()`, by an ignored
`Result`, or by a branch that neither acts nor states.

#### Scenario: A refused operation says why on screen
- **WHEN** an operation the composition root runs is refused
- **THEN** the reason appears on the interface's one "why that did not happen"
  line, in the words the operation was refused with

#### Scenario: The same refusal twice is two refusals
- **WHEN** the same impossible operation is asked for twice in a row
- **THEN** each attempt is counted as its own refusal, and the line on screen
  is not redrawn for the second

#### Scenario: A refusal does not outlive what it was about
- **WHEN** a refused operation is followed by one that works
- **THEN** the reason is taken down

### Requirement: Every notice channel a ViewModel owns is one the shell reads
A ViewModel that can refuse SHALL carry its refusal on an observable channel,
and the composition root SHALL read every such channel: both to draw it on the
interface's one "why that did not happen" line and to compare it either side of
a command for the agent door.

The channels SHALL be named in one list rather than in each reader. A reader
that samples a channel's count before a command and reads its words afterwards
SHALL take both from that one list, so a channel present in one reading and
absent from the other is not expressible.

Registering a channel SHALL NOT be a matter of review. Adding a ViewModel that
owns a notice channel without adding it to the list SHALL fail an automated
check that names the channel.

Writing a refusal to the process's error stream SHALL NOT stand in for either
reader.

#### Scenario: A panel's refusal reaches both readers
- **WHEN** a cage, a curve, a boolean or a rig command is refused
- **THEN** the reason appears on the interface's one "why that did not happen"
  line, and the command is answered to the client as a refusal carrying the
  same reason

#### Scenario: A ViewModel added without registering its channel fails a check
- **WHEN** a ViewModel the composition root holds owns a notice channel that no
  reader is named against
- **THEN** an automated check fails, naming the channel that is written and
  never read

#### Scenario: A refusal on any channel is the command's answer
- **WHEN** a command is refused on a channel other than the first
- **THEN** it is still answered as a refusal, with the sentence that channel
  was written

### Requirement: One command reaches every ViewModel against one document state
A command SHALL be dispatched to the ViewModels in an order that lets each of
them read a document the command has already reached. Where one ViewModel
changes document state that others read while handling the same command, that
ViewModel SHALL be dispatched to first.

The active layer is the case this exists for: the scene ViewModel is the only
one that moves it, and the sculpting, mask, cage and manipulator ViewModels all
read it while handling the same command. Dispatched after any of them, a
selection leaves each follower set up for the layer that was left, and the
*next* command is the first to see a consistent document — an error that is
always exactly one command behind and therefore reads as a defect in whatever
was done next.

Where the composition root moves the active layer without a command passing
through the ViewModels — a conversion, opening a document, starting a rig — it
SHALL tell the ViewModels that read the active layer to catch up, by name.

#### Scenario: A follower reads the layer the command selected
- **WHEN** a layer-selection command is dispatched
- **THEN** every ViewModel that reads the active layer while handling it reads
  the newly selected layer, not the previous one

#### Scenario: The order is a property of the composition root
- **WHEN** the composition root's dispatch is inspected
- **THEN** the ViewModel that moves the active layer is dispatched to before
  every ViewModel that reads it
