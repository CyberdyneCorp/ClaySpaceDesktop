## 1. Upload

- [x] 1.1 `compact_release_geometry` returns the keys it changed; `settle_after_edit` patches only those, falling back to the mapped relayout.
- [x] 1.2 A key discarded by compaction gives its span back to the layout, blanked.
- [x] 1.3 `refresh_mask` uploads only keys whose weights moved; `ClayDocument::has_mask`.
- [x] 1.4 `CaptureTargets` keeps one offscreen target for agent captures.

## 2. Accounting

- [x] 2.1 `SettleCost::split_time`, `prune_time` and a measured `upload_time` on every route; `SettleCost::parts`.
- [x] 2.2 `rebuild_at` zeroes stage timings so the empty route reports its own.
- [x] 2.3 The `re-malha final` console line reports split and prune.

## 3. Verification

- [x] 3.1 `settle_uploads.rs`: `a_dab_uploads_only_its_keys`, `an_empty_mask_refresh_uploads_nothing`, `captures_reuse_their_target`, `the_settle_breakdown_sums_to_the_total`, `an_empty_settle_does_not_inherit_the_last_engine_time`.
