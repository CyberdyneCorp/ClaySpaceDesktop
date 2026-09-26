# Write only what a settle changed, and say what it spent

Issue #175. A released stroke cost a fixed 150–220 ms of application-side
work at any size, every dab and every undo re-uploaded about 1.75 MB — the
whole layer — and the breakdown meant to explain it reported zero for the
upload.

## Why it happened

- `settle_after_edit` compacted duplicate triangles and then forced a full
  relayout, so every release reallocated the GPU buffers and wrote every key,
  however few the compaction had changed.
- `refresh_mask` marked every key touched on any `mask_revision` bump, even
  with no mask to draw, so `mask/apply clear` with no mask uploaded 1.78 MB of
  zeroes over zeroes.
- Each agent capture built a new offscreen target (texture, depth buffer and
  readback buffer) and discarded it.
- `SettleCost::upload_time` was hard-coded to zero on both settle routes, split
  and duplicate pruning were not measured at all, and the empty route carried
  the previous rebuild's engine time (one line read `motor 227 > total 101`).

## What changes

- Release compaction returns the keys it changed (lost a triangle, had
  vertices renumbered, or were discarded) and patches only those spans. A
  discarded key's span is returned to the layout as a hole. The full mapped
  relayout remains the fallback when the layout cannot take a patch.
- `refresh_mask` uploads only keys whose weights changed, and with no mask it
  clears without sampling; `ClayDocument::has_mask` answers the question
  without gathering every vertex position.
- Captures reuse one cached `OffscreenTarget`, replaced only when size or
  format changes (`CaptureTargets`).
- `SettleCost` gains `split_time` and `prune_time`; `upload_time` is measured
  on every route; `rebuild_at` zeroes stage timings first so a route reports
  only its own work. The console line reports each stage.

## Not in this change

- Pruning only the touched keys and their neighbours instead of hashing the
  whole store on every release.
- Re-sampling only the mask cells a mask change reached, and not triggering the
  refresh from unrelated revisions.
- A settle benchmark group over 50k / 300k / 1M-triangle fixtures asserting
  the fixed-overhead bound.
