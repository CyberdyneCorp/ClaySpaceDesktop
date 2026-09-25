## Decisions

`Applied.outcome` distinguishes `applied` from `nothing_to_do`. A no-op remains a successful tool call because repeating it is harmless; the reason is carried in `notices`.

The application checks the target state before dispatch, so no-op commands do not enter a model or mutate history. The diagnostics command writes directly to the system clipboard when called through MCP and reports a failure if the clipboard rejects it.

Reduced LOD uses a complete mip set or falls back to full resolution. A partial mip set cannot be drawn alone without holes, and mixed LOD meshing would require a seam stitching path.

Mesh taper and twist already pass the active mask to the engine. The new regression test protects this contract. The cage is a separate control point operation and does not use the mask.
