## MODIFIED Requirements

### Requirement: Every fallible engine call becomes a Rust Result
The safe wrapper SHALL map every `clay_result` code to `Result<_, ClayError>`. On
failure it SHALL capture the engine's thread-local detail message via
`clay_last_error` at the point of failure, before any further engine call can
overwrite it, and SHALL carry that message in the error value.

**Every code the engine's header declares SHALL become a kind that names it**, and
no two kinds SHALL print the same sentence. A code carried as an opaque number is
a refusal a caller cannot branch on and a reader cannot understand, and the
failure is silent: the call still returns an error, it simply says nothing useful.
A code the header does not declare SHALL be carried verbatim rather than
flattened into a neighbouring kind.

This SHALL be held by a check against the pinned header itself rather than by
review. The wrapper SHALL fail its own tests when the engine declares a result
code the table does not name, and SHALL skip that check — rather than failing it —
where the vendored source is not present, since a packaged build has the generated
bindings and not the engine's source tree.

Codes that are not `clay_result` values SHALL NOT be folded into the same kind.
The engine has refusal enumerations of its own, returned beside a result rather
than in place of one, and a refusal that means "this hierarchy has no such pass"
is not the same statement as "this call was malformed".

#### Scenario: Detail message is captured at the failure site
- **WHEN** an engine call fails and the application makes further engine calls
  before inspecting the error
- **THEN** the error still reports the detail message belonging to the original
  failure

#### Scenario: No panic across the boundary
- **WHEN** any engine call returns a failure code
- **THEN** the wrapper returns an error value and does not panic, abort, or unwind
  through the C boundary

#### Scenario: Every declared code has a kind of its own
- **WHEN** each code the engine's result enumeration declares is mapped
- **THEN** each becomes a distinct kind with a sentence no other kind prints

#### Scenario: A code the wrapper does not know is carried, not flattened
- **WHEN** a result code arrives that the wrapper's table does not name
- **THEN** the error carries the code itself rather than reporting a neighbouring
  kind

#### Scenario: A code added upstream fails the wrapper's own tests
- **WHEN** the pinned engine's header declares a result code the wrapper does not
  name
- **THEN** the wrapper's tests fail, naming the code
