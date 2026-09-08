## ADDED Requirements

### Requirement: CyberRemesher is vendored at a released tag and its version is verified
The engine SHALL enter as a git submodule at `vendor/CyberRemesherAndUV`, pinned
to a **released tag**, and the build SHALL refuse a submodule that is absent or
at a different revision than the superproject records — as `claycore-sys`
already does.

**The pin SHALL be the submodule commit, and `cyber_version` SHALL be treated as
informational.** Unlike ClayCore there is no separate ABI number: `SOVERSION` is
the project's major, which is 0, so `libcyber_capi.so.0` names v0.7.0 and v0.8.0
alike and a mismatched library loads without complaint — surfacing later as
behaviour rather than as a link error.

`cyber_version` SHALL still be checked against a constant the Rust side declares,
so that a submodule moved without the constant following fails a test rather than
running. Where the engine gains an ABI-level number, this application SHALL
assert on it as it already does on ClayCore's `EXPECTED_ABI`.

The first pin SHALL be **v0.8.0**, which is the release carrying the ZRemesher
track and the fix for a PLY header that could size an allocation from an
attacker-controlled count. v0.7.0 SHALL NOT be pinned: this application imports
meshes, and that defect is a denial of service in any build that reads an
untrusted file.

Only the quad methods the pinned build implements SHALL be offered. Offering one
it does not carry is a refusal a sculptor meets at the moment they ask for it.

#### Scenario: The submodule is at a revision the superproject does not record
- **WHEN** the workspace is built
- **THEN** the build fails naming both revisions, before CMake runs

#### Scenario: A quad method the pinned release does not carry is asked for
- **WHEN** the interface offers quad methods
- **THEN** only those the pinned build implements appear

### Requirement: The unsafe lives in two crates and nowhere else
`cyberremesh-sys` SHALL hold the generated bindings and `cyberremesh` the safe
wrapper, and both SHALL be added to the `unsafe` allowlist in
`tools/check_layering.py`. Every other crate SHALL continue to declare
`#![forbid(unsafe_code)]`.

The View, the ViewModels and the agent-facing crate SHALL be forbidden from
depending on either, by the same rules and for the same reasons that already
forbid them `claycore` and `claycore-sys`.

#### Scenario: A ViewModel reaches for the retopology engine
- **WHEN** the layering check runs
- **THEN** it fails naming the crate and the forbidden dependency

### Requirement: The sculpt handoff is written by the handoff writer
A sculpt SHALL leave through `clay_mesh_save_handoff` or its in-memory twin,
and SHALL NOT leave through `clay_mesh_save`.

The general writer declares a mesh's **quads** as its faces when it has them,
and CyberRemesher's reader rejects any arity but triangles — so the best export
this application can produce is exactly the file the pipeline refuses. The
handoff writer triangulates and computes normals, both of which their reader
requires.

The declared handoff version SHALL be carried from ClayCore's own
`CLAY_HANDOFF_VERSION_MAJOR`/`MINOR` rather than restated.

#### Scenario: A mesh with quads is handed off
- **WHEN** the sculpt is written for retopology
- **THEN** the file carries triangles and normals, and their reader accepts it

### Requirement: The field evaluator's inversions are converted, not passed through
Where ClayCore answers a `CyberFieldEvaluator` callback, the correspondence
SHALL be:

| their callback | ours |
|---|---|
| `distance(p)` | `clay_eval_points` |
| `gradient(p)` | `clay_eval_gradients`, already unit length |
| `occlusion(p, n, r)` | **`1.0 - clay_measure_points(CLAY_MEASURE_OCCLUSION)`** |
| curvature | **left to their default**, which derives it from `gradient()` |

Their `occlusion` is **openness**, where 1 is fully open; ours is occlusion,
where 1 is fully enclosed. Passing ours through unconverted bakes an inverted
ambient occlusion map that looks plausible and is wrong everywhere.

`CLAY_MEASURE_CURVATURE` is a saturated `[0,1]` masking value while theirs is
signed mean curvature in `1/length`. They are not the same quantity and SHALL
NOT be substituted for one another.

Both SHALL be held by a test rather than by a comment.

#### Scenario: Ambient occlusion is baked from the field
- **WHEN** a point that is fully enclosed is sampled
- **THEN** the value reaching their baker is the one meaning "closed", and a
  test fails if the conversion is removed

### Requirement: The build refuses without QuadCover, and the run checks anyway
The engine SHALL be configured with **`CYBER_REQUIRE_QUADCOVER=ON`**, which fails
the CMake configure when the dependency is missing rather than falling back. The
`cpu-headless` preset sets `CYBER_WITH_QUADCOVER=ON`, which *enables* the solver
but still permits a silent fallback; the require flag is what turns a missing
dependency into a build failure.

The application SHALL **also** read `cyber_version`'s solver string at startup
and refuse a library reporting `native` rather than `native+geogram`.

**These are two gates and neither is redundant, because they guard different
failures.** The configure flag catches *our build losing the dependency*. The
startup check catches *a different library being loaded* — which this engine's
own versioning makes possible, since `SOVERSION` is the project major, still 0,
so `libcyber_capi.so.0` names any 0.x release and a substituted one links
without complaint. A build-time gate cannot see that, and a startup gate cannot
prevent the build.

Without either, a missing solver does not fail: it routes to the portable
quadrangulator and produces genuinely different quads, visible only as "the
output got worse and nobody knows why".

#### Scenario: The QuadCover dependency is missing at configure time
- **WHEN** the workspace is built
- **THEN** CMake fails naming the dependency, rather than producing a fallback
  build

#### Scenario: A library built without the solver is loaded at run time
- **WHEN** the application starts
- **THEN** it refuses, naming which quadrangulator the loaded library reports

### Requirement: A mesh handle is valid for one operation
A `CyberMesh` SHALL NOT be held across a document edit or across another
retopology operation. The discipline SHALL be **build, use, drop**, per
operation.

The engine's element-id stability contract is that most retopology operations
reassign ids and `cyber_retopo_subdivide*` reassigns **all** of them. Anything
cached and keyed on a vertex or face id — a selection, a pin, a mapping back into
ClayCore — is stale the moment such a call returns, and stale with nothing to
announce it.

Any correspondence this application keeps between a retopologised mesh and the
sculpt it came from SHALL therefore be **positional and rebuilt**, never an id
that was kept.

#### Scenario: A retopology is followed by a subdivision
- **WHEN** the second operation returns
- **THEN** nothing downstream is reading an id recorded before it

### Requirement: An import through the retopology engine carries our own resource ceiling
The retopology engine has **no import ceiling of any kind** — no maximum file
size, no vertex cap, justified or otherwise. Its `max_vertices` parameters are
caller-supplied *output buffer sizes*, not limits. Its entire defence is the
structural bound: each element's declared count compared against the actual file
size, computed by division so the product cannot overflow while being checked.

That is a **hostility** bound and not a **resource** bound, and the two are not
interchangeable:

- a fixed vertex cap passes a bomb that declares just under it, where a
  file-size-derived bound does not — a bomb is by definition a small file making
  a large claim;
- a file-size-derived bound admits a legitimate enormous mesh, where a resource
  ceiling refuses it.

So any import routed through the retopology engine — its FBX path in particular,
which ClayCore does not read — SHALL carry a ceiling set on **this** side, and
that ceiling SHALL NOT be expressed as "tighter than the engine's own", because
the engine has none to be tighter than. `ImportSettings` is where it belongs,
beside the 8,000,000 that already bounds the ClayCore path.

A test asserting our ceiling sits below an engine default SHALL NOT be written
for this path; the number it would compare against does not exist.

#### Scenario: A mesh is imported through the retopology engine
- **WHEN** the file declares more vertices than this application allows
- **THEN** it is refused by our own ceiling, the engine having no opinion

#### Scenario: A hostile file declares a count just under a fixed cap
- **WHEN** the file is small and the claim is large
- **THEN** the engine's structural bound refuses it regardless of our ceiling,
  because the two guards answer different questions
