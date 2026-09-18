## Why

ClayCore #531 still pays unnecessary release work. The host owes a full settlement after every SDF stroke, including mask-only edits and tools whose preview-to-document epoch change already triggered a full rebuild during synchronization. These paths need no second rebuild of the same surface. Ordinary full-gradient incremental edits also retain the exact rebuilt triangle set, with duplicate ownership copies that can be compacted without sampling or meshing the field again.

## What Changes

Track whether stored triangles combine separate partial meshing requests. Settle eligible synchronized document-gradient surfaces by exact duplicate compaction, retaining full rebuilds for preview shading and other ineligible states. Consume deferred debt without any work for a surface already produced by one request. Preserve the live-gesture guard, complete mask rendering, and retain explicit rebuild operations.

## Impact

SurfaceGeometry state and host deferred settlement only. No engine pin or field/shading approximation changes. Add native zero-upload release regression, exact repeated-release geometry tests, explicit fallback guards, and a distinct compaction telemetry route.
