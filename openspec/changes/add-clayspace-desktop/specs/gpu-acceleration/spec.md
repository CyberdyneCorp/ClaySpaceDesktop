## ADDED Requirements

### Requirement: Available backends are discovered at runtime
On startup the application SHALL query the engine's registered backends via `clay_list_backends` and SHALL build its selection from that answer alone. It SHALL NOT assume a backend is present because the platform usually offers one, nor because it was compiled in.

#### Scenario: Discovery reflects the machine, not the build
- **WHEN** the application starts on a Linux machine whose build included the CUDA preset but whose driver is unavailable at runtime
- **THEN** the CUDA backend is absent from the discovered list and is not selected

#### Scenario: Discovery cannot yield an empty list
- **WHEN** backend discovery runs on any supported machine
- **THEN** the CPU backend is present, because the engine compiles it in unconditionally, and selection always has a candidate

### Requirement: Backend preference follows the platform
The application SHALL rank discovered backends by a fixed per-platform preference and select the highest-ranked one available: on macOS `metal` then `cpu`; on Linux `cuda`, then `opencl`, then `cpu`.

#### Scenario: macOS prefers Metal
- **WHEN** the application starts on macOS with the Metal backend registered
- **THEN** `metal` is the active backend

#### Scenario: Linux without CUDA falls to the next tier
- **WHEN** the application starts on Linux with `opencl` and `cpu` registered and `cuda` absent
- **THEN** `opencl` is the active backend

#### Scenario: No GPU backend at all
- **WHEN** no GPU backend is registered on any supported platform
- **THEN** `cpu` is the active backend and the application starts normally with no error presented to the user

### Requirement: Unsupported operations fall back per call, not per session
Where a backend reports an operation unsupported — the engine documents OpenCL as providing neither raycast nor device meshing — the application SHALL fall back to the CPU backend for that operation only, SHALL keep the selected backend active for every operation it does support, and SHALL NOT present the fallback to the user as an error.

#### Scenario: Raycast falls back while evaluation does not
- **WHEN** the active backend is OpenCL and the application performs a raycast
- **THEN** the raycast is served by the CPU backend, subsequent point evaluation still uses OpenCL, and no error is surfaced

#### Scenario: Fallback is recorded once
- **WHEN** an operation falls back to CPU repeatedly during a session
- **THEN** the fallback is recorded in the diagnostics log once per operation kind, not once per call

### Requirement: Backend selection never changes results
Switching the active backend SHALL NOT change the content of a document, the geometry of an exported mesh, or the outcome of any edit. Backend choice SHALL affect only the time an operation takes.

#### Scenario: Same document, different backends, same export
- **WHEN** the same document is exported to a mesh once on a GPU backend and once on the CPU backend
- **THEN** the two exports agree within the engine's documented parity tolerance, and the saved `.clayspace` documents are byte-identical

#### Scenario: Switching mid-session preserves the document
- **WHEN** the user changes the active backend while a document is open
- **THEN** the document is unchanged, no edit is lost, and no re-authoring is required

### Requirement: The active backend is visible and overridable
The application SHALL display the active backend in the interface and SHALL allow the user to override the automatic selection with any discovered backend, including forcing CPU. The override SHALL persist across sessions until cleared.

#### Scenario: User forces the reference path
- **WHEN** the user selects CPU as an override on a machine where a GPU backend is available
- **THEN** every subsequent operation uses the CPU backend, and the interface shows CPU as active with the selection marked as a manual override

#### Scenario: A persisted override that is no longer available
- **WHEN** a session starts with a persisted override naming a backend absent from the discovered list
- **THEN** the application falls back to automatic selection, applies the platform preference, and informs the user that the stored override was unavailable

### Requirement: Acceleration state is reportable for diagnostics
The application SHALL provide a diagnostics view listing the discovered backends, the active backend, whether it was chosen automatically or overridden, the engine version, and every operation kind that has fallen back this session.

#### Scenario: A user reports a performance problem
- **WHEN** the user opens the diagnostics view
- **THEN** it names the discovered backends, the active one, its selection reason, and any operations running on a fallback path

### Requirement: Rendering acceleration is independent of engine acceleration
The WebGPU rendering device and the engine's evaluation backend SHALL be selected independently. A failure or absence in one SHALL NOT constrain the other.

#### Scenario: Software rendering with GPU evaluation
- **WHEN** WebGPU resolves to a software adapter while the engine has a GPU backend registered
- **THEN** the engine still uses its GPU backend, and the application reports the two independently
