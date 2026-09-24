# Design

The existing test already builds the same starting form, applies 24 fixed dabs, syncs after each one, and times pointer-up. Once the gesture is complete, a fresh `SurfaceGeometry` rebuilds that exact document. This control uses the same engine, mesher and GPU upload path on the same runner while doing fixed, much larger work. It is independent of whether the incremental geometry retained its own key buffers.

On a local release build, the worst segment is 1.38–1.45 ms and the control rebuild is 32–39 ms across CPU-only and Metal runs: about 4% of the control. One quarter allows roughly six times the observed fraction yet catches a segment that approaches a full re-mesh. Pointer-up is about 0.76–0.79 ms, or 2–2.4% of the control; one tenth catches a renewed whole-scene shading pass.

The test reports both raw timings and the reference timing on every run. It keeps the debug build as measurement-only because compiler profile overhead dominates there. The independent reference is measured after the gesture so it describes the exact fixed scene the test produced.

`sculpt_latency` carries the same shared-runner exposure with fixed 50/100 ms release assertions. It also applies a fixed set of 24 dabs, so a fresh full rebuild after them supplies an independent control. On a local Metal release build, median and p95 are 1.2/1.5 ms against a 34.7 ms rebuild, or about 3.5/4.3%. The gate allows median at most one fifth and p95 at most one third of the control. It still reports raw milliseconds for the separately specified reference-machine budget.
