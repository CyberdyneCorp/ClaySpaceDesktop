## 1. Reuse the buffers that already fit

- [x] 1.1 Split the growth policy out of `GpuMesh::upload` into `grow_to`, so the replace path and the reserve path decide the same way and only one of them states the device ceiling.
- [x] 1.2 Make `reserve` grow rather than replace, keeping a buffer the reservation already fits and leaving its recorded capacity at the larger figure.
- [x] 1.3 Route every mesh buffer through one constructor, so what is allocated can be counted.

## 2. Write ranges, not keys

- [x] 2.1 Add `patch_vertex_runs` and `patch_index_runs`, which sort the runs a caller hands them and merge the ones that abut into one mapped write.
- [x] 2.2 Split the settle's `patch` into a placing pass and a writing pass, so the merge can see every destination before it writes any of them.
- [x] 2.3 Keep the full rebuild on per-brick writes, as `batch-whole-surface-uploads` measured.

## 3. Let the device give the memory back

- [x] 3.1 Poll the device once after each frame's submit, without waiting.

## 4. Count what was only argued about

- [x] 4.1 Count buffer writes beside the bytes they carried, and mesh buffer allocations beside both.

## 5. Hold it

- [x] 5.1 `reserve_reuses_a_buffer_that_already_fits`: the same reservation twice allocates once, a smaller one allocates nothing, a larger one replaces only the buffer that ran out.
- [x] 5.2 `a_rebuilt_layout_of_the_same_size_allocates_nothing`, on a real document.
- [x] 5.3 `a_settle_merges_key_ranges_into_one_write`, with the merged bytes checked against a rendered reference rather than only counted.
- [x] 5.4 `spans_with_a_gap_between_them_are_written_separately`, because a merge across an untouched key would overwrite geometry nobody asked it to touch.

## 6. Left for later

- [ ] 6.1 A scripted long-session harness that records the footprint floor at intervals and asserts it does not ratchet, which the acceptance criteria ask for and this repository has nowhere to put yet.
- [ ] 6.2 The reported-`in_use`-against-footprint ratio, which belongs with the memory reporting issue rather than here.
- [ ] 6.3 The 192 `Validation Error` lines from the audited session, which may or may not share this cause.
