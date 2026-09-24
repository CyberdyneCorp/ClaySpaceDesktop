# Design

## Published route boundary

Reject an unknown `measure.group` before building a command, using the
`GROUPS` list already used by `tools/list` and its schema. This keeps the
measurement wrapper from becoming a second access path to unpublished verbs.

## Outstanding jobs

Read progress from the existing retopology, UV, conform and bake job runners.
Add each active progress entry to `App::outstanding_work`, the common source
for `wait`, `state.jobs` and capture metadata. Keep the job visible until the
application collects its result on a frame or during `wait`. Copy reported progress into the
runner's observable when it is polled, so the fraction is current rather than
remaining at the initial zero. Poll the same workers during `wait`, so a
finished job can be collected even when no redraw has happened.

## Descriptions

List all ten deliberately unoffered command variants and use each command's
existing `Home::NotOffered` reason. Correct the six summaries by inspecting
the ViewModel and engine paths that consume their arguments.
