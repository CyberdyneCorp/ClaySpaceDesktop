## ADDED Requirements

### Requirement: The workspace is layered Model, ViewModel, View
The application SHALL be organized into crates with a strict dependency direction: `clayspace-model` (domain and engine access) ← `clayspace-vm` (ViewModels) ← `clayspace-view` (interface and rendering) ← `clayspace-app` (composition root). No crate SHALL depend on a crate later in that order.

#### Scenario: Dependency direction holds
- **WHEN** the workspace dependency graph is inspected
- **THEN** no edge runs from a Model crate to a ViewModel crate, or from a ViewModel crate to a View crate

### Requirement: The View layer cannot reach the engine
`clayspace-view` SHALL NOT depend on `claycore` or `claycore-sys`, directly or transitively. No ClayCore type, handle, enum, or error SHALL appear in the View layer's API or implementation.

#### Scenario: Engine dependency in the View fails CI
- **WHEN** a dependency on `claycore` or `claycore-sys` is added to `clayspace-view`
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

#### Scenario: One place to observe every mutation
- **WHEN** the command path is instrumented in a debug build
- **THEN** every document and application state change appears in that instrumentation

#### Scenario: Commands are independent of their source
- **WHEN** the same command is dispatched from a menu item, a keyboard shortcut, and a panel button
- **THEN** the resulting state change is identical in all three cases

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
