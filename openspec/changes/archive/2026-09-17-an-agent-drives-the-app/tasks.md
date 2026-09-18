# Tasks

## 1. The crate and its place

- [x] 1.1 `crates/clayspace-mcp`, depending on `clayspace-model` and
      `clayspace-vm` and on nothing else of ours, with
      `#![forbid(unsafe_code)]`
- [x] 1.2 `serde` and `serde_json` here and nowhere else in the workspace
- [x] 1.3 `tools/check_layering.py` gains the crate and its forbidden edges —
      `claycore`, `claycore-sys`, `clayspace-view`, `clayspace-engine`, and any
      windowing, drawing or input library
- [x] 1.4 A test that the crate builds and its suite runs with no display, no
      GPU and no C++ engine built, because that is the property the whole
      arrangement exists for
- [x] 1.5 `deny.toml` and a regenerated `ATTRIBUTION.md`

## 2. The transport

- [x] 2.1 An HTTP/1.1 subset: request line, headers, `Content-Length` and
      chunked bodies, keep-alive, and nothing else
- [x] 2.2 `POST /mcp` answering `application/json`, `GET /mcp` holding a
      `text/event-stream`, `DELETE /mcp` ending a session
- [x] 2.3 JSON-RPC framing, and `initialize` reporting the protocol version
      this build implements as a named constant
- [x] 2.4 A listener bound to loopback, a thread per connection, and a refusal
      to bind anything else — a configuration asking for a reachable address is
      an error rather than an honoured request
- [x] 2.5 The port taken when the preferred one is busy, and published
- [x] 2.6 `SessionStore::agente.acesso` — port and a per-run secret, written
      `0600`, removed at exit
- [x] 2.7 Bearer authentication, compared in constant time, refusing without
      disclosing what is behind it
- [x] 2.8 `Origin` validation, and no CORS header ever emitted
- [x] 2.9 Conformance cases for the subset, beside it

## 3. The seam

- [x] 3.1 The `Session` trait: `apply`, `read`, `capture`, `settle`, `measure`,
      `consent`, `gesture_in_progress`
- [x] 3.2 A fake `Session` in the crate's tests, which is what makes the tool
      surface exercisable headlessly
- [x] 3.3 A job queue shared with the composition root, each job carrying its
      reply channel, with a deadline on the waiting side
- [x] 3.4 `EventLoop::with_user_event` and an `EventLoopProxy`, so a request
      arriving at a sleeping application wakes it
- [x] 3.5 The drain in `user_event` **and** in `about_to_wait`, so a job that
      raced the wake-up is not held until the next input
- [x] 3.6 A bound on jobs executed per frame, so a burst delays itself rather
      than starving the redraw
- [x] 3.7 The reply sent from inside the drain, which is what makes a changing
      tool's answer mean the change has happened
- [x] 3.8 The composition root's `Session` implementation

## 4. Reading, and the window that says it is listening

- [x] 4.1 `state`, `jobs` and `session` tools — document, scene tree, layers
      and their representations, selection, active tool and its settings, mask
      coverage, camera, history depth and what the next undo would undo
- [x] 4.2 The read path reads `Observable::get` and never marks anything
      changed, with a test that repeated reads schedule no redraw
- [x] 4.3 `AgentViewModel` in `clayspace-vm`: listening, connected, when an
      agent last acted, and any consent being asked for
- [x] 4.4 The status area draws it
- [x] 4.5 The menu bar's controls — stop, start, and show the address and
      secret a client would need
- [x] 4.6 The choice remembered in the session store, so a door shut by hand
      stays shut

## 5. Seeing

- [x] 5.1 `viewport.capture` through `OffscreenTarget`, same renderer, camera,
      shading, overlays and quality as the window
- [x] 5.2 A requested size, with the answer saying what was rendered
- [x] 5.3 The whole window, by running the `egui` pass into the same target
- [x] 5.4 PNG encoding and base64 on the connection thread, not the interface
      thread
- [x] 5.5 A base64 encoder of our own, against the published vectors
- [x] 5.6 A capture asked for alongside a command taken after that command's
      dirty region has been re-meshed
- [x] 5.7 Outstanding work named beside the image where the frame is not
      settled
- [x] 5.8 `settle` with a bound, naming what is still running when the bound is
      reached
- [x] 5.9 The render floor, measured through the same path the compared
      captures went through, and reported with any difference

## 6. Driving

- [x] 6.1 The catalogue: the domain groups, each an action and its arguments
- [x] 6.2 `action_of(&Command)` — one exhaustive match, no wildcard arm, so a
      new `Command` variant does not compile until it has a home
- [x] 6.3 `NotOffered(reason)` for the variants only a pointer can mean, so
      "why can't I call this" has an answer in the source
- [x] 6.4 JSON arguments to `Command`, per group
- [x] 6.5 `describe`, answered from the catalogue, including which actions are
      currently unavailable and why
- [x] 6.6 A malformed or unknown call refused with what was expected, changing
      nothing
- [x] 6.7 A refusal carrying both a stable code and the interface's own
      localized words
- [x] 6.8 Mid-gesture: a changing tool refused, saying a gesture is in
      progress; reading tools served
- [x] 6.9 Long work run the way it is run for a person — off the interface
      thread, progress observable

## 7. Consent

- [x] 7.1 The gate table, keyed on `Command`, written down rather than inferred
      from `touches_document()`
- [x] 7.2 `agente.consentimentos` in the session store, one kind per line
- [x] 7.3 The ask as `AgentViewModel` state; the shell draws it and emits the
      answer as a command, so the View stays a pure function of state
- [x] 7.4 The ask names the operation, the client and the path
- [x] 7.5 A bound on the ask, refusing rather than holding the connection
- [x] 7.6 A refusal that names the gate and what would lift it
- [x] 7.7 Consent that does not generalise past the operation asked for

## 8. Measuring

- [x] 8.1 `measure`: wall time, whether a frame stalled, backend, platform, and
      the marker saying this is a live-session figure
- [x] 8.2 The `FrameLog`, the memory ledger by part and the fallback list as
      readable state
- [x] 8.3 Nothing writes `benchmarks/*.json`, and a test that says so
- [x] 8.4 An agent request that stalls a frame appears in the `FrameLog` under
      its own operation name
- [x] 8.5 The diagnostics report's section — listening, address, connected, how
      many of this session's commands came from an agent
- [x] 8.6 The secret is not in the report, with a test

## 9. Tests

- [x] 9.1 A stroke through a tool and the same stroke by hand produce one
      history entry each, and one undo returns the document in both cases
- [x] 9.2 A tool cannot reach past a refusal the interface would give
- [x] 9.3 Every `Command` variant is reachable through some group, or is named
      as deliberately not offered
- [x] 9.4 Two clients calling at the same moment do not interleave
- [x] 9.5 An idle application answers without a person touching the window
- [x] 9.6 A request without the secret, with the wrong secret, and with a
      secret from a previous run are each refused
- [x] 9.7 A request declaring a web origin is refused whatever secret it
      carries
- [x] 9.8 A capture taken with a stroke carries the stroke, re-meshed
- [x] 9.9 A gated operation is refused without consent and proceeds with it
- [x] 9.10 An idle connection costs nothing measurable in frame timing, and a
      client that stops reading does not stall the window
- [x] 9.11 A visual capture of the status area listening, and of the consent
      ask, regenerated with the rest
- [x] 9.12 An end-to-end test that drives the real application over loopback,
      in the harness-free style `window_smoke` already uses because macOS wants
      the event loop on the process main thread

## 11. The armature reaches the command path

- [x] 11.1 Six commands for what only a pointer could say: select, add,
      insert, move, resize and reparent a ZSphere
- [x] 11.2 `ArmatureViewModel` verbs behind them, mirrored where the
      armature's symmetry is on, exactly as a drag is
- [x] 11.3 A move takes a point rather than a displacement — exact, where a
      drag only ends up close
- [x] 11.4 A sphere grown by asking is the same sphere as one grown by
      dragging, held against each other in a test
- [x] 11.5 An index that is not there is named rather than guessed, and does
      not cost the caller a selection they already had

## 10. The record

- [x] 10.1 `docs/architecture.md`: the layer, why it sits beside the View, and
      the seam
- [x] 10.2 `docs/features.md` and the README: the door, and how to point a
      client at it
- [x] 10.3 The blast radius stated where a reader will find it — any process
      running as this user can read the secret and drive the session
- [x] 10.4 `justfile` targets for running the application with the door open
      and for the crate's suite
