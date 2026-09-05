## MODIFIED Requirements

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
