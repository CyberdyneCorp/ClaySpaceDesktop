# Tasks

## 1. Move the pin

- [x] 1.1 Point the submodule at v0.78.0 and move `EXPECTED_ABI`, which the
      `version_is_the_pinned_engine` test holds to the submodule. It moves by
      hand and is deliberately not derived from the linked engine, which would
      make the check assert that a number equals itself
- [x] 1.2 Build the whole workspace against the new engine before changing a line
      of it, so that what the upgrade forces is separable from what it enables —
      it forces nothing: 146 added entry points, none removed, no signature
      changed, no struct re-laid out, and the three descriptors that grew did so
      behind the `struct_size` this workspace writes from `size_of`
- [x] 1.3 Run the whole suite and triage every failure against the release notes
      rather than assuming staleness — one failure, a tripwire written to fail on
      exactly this

## 2. Decide what to write, since the formats moved

- [x] 2.1 Establish whether the upgrade notes' advice — write at minor 15 if you
      exchange documents with an older build — is reachable. It is not: the
      `minor` parameter is on the C++ `serialize_document`, not on
      `save_clayspace`, and not on `clay_document_save`, which takes a path and
      nothing else
- [x] 2.2 Name the minor this build writes as a constant carrying the reasoning,
      rather than leaving it implicit in whatever the engine does
- [x] 2.3 Check the constant against a file this build actually wrote, by reading
      the container header, rather than asserting it
- [x] 2.4 Ratchet it against the pinned engine's own headers, so the day upstream
      moves the minor again this build says so instead of silently following
- [x] 2.5 Carry the minor into the diagnostics report, so "it will not open on the
      other machine" has a figure to quote
- [x] 2.6 Establish what the brush preset format's move to version 2 costs here.
      Nothing: no preset entry point is bound and the session directory holds no
      brush. Adopt the field the version bump exists for instead

## 3. A hierarchy, and where its bytes live (upgrade item 5)

- [x] 3.1 Wrap the hierarchy, its sculpt layer stack, the surface-view transport,
      the memory tier and the maintenance queue in `claycore`, with every wrapper
      executed by a test and the ones nothing calls yet run in `abi_surface.rs`
- [x] 3.2 Name in each module doc the entry points deliberately not wrapped and
      why — ten take an adaptive-surface handle this workspace does not hold,
      three are telemetry nothing reads, two are the projection
- [x] 3.3 Settle the undo question by measurement before specifying anything on
      top of it. The stroke record does not cross the C ABI, `dirty_blocks` has no
      write-back to reconstruct a delta through, and the transaction's exact
      cancel has no resolved-stroke verb — so the history holds the surface's own
      bytes, bounded and trimmed from the old end
- [x] 3.4 Decide where the bytes go, against the object table's precedent, and
      break that precedent in exactly one place with the reason written down: a
      failed side-car write fails the save, because here the side-car is the work
- [x] 3.5 Take the side-car as one file rather than the directory the survey
      specified — `clay_multires_serialize` measures 1.39 ms, so a per-layer
      checksum skip saves a millisecond on a two-minute clock and costs a tree
      that Save-As must copy and every removal must prune
- [x] 3.6 Decide what a document whose side-car is missing opens as, rather than
      discovering it. It opens as the cage it demonstrably holds; a side-car that
      is present and damaged is named in diagnostics, and one that is absent
      cannot be, because nothing in a `.clayspace` distinguishes a document that
      never held a hierarchy
- [x] 3.7 Fill the memory ledger, which is the second half of the same item —
      measured, the plain roll-up omits 8,446,536 bytes of sculpting session on a
      document whose whole document half is 8,463,808

## 4. Between two strokes, and the normals (upgrade item 6)

- [x] 4.1 Drain the maintenance queue at every gesture end against a stated budget,
      and hold the stroke gate for the length of a gesture rather than for the
      length of a borrow — a gesture spans frames, so the gate cannot be a
      borrowing guard
- [x] 4.2 Give the queue something real to service: the ray tree was refitted at
      four sites and rebuilt at none, and the number saying it wants rebuilding was
      computed and thrown away
- [x] 4.3 Establish which of the engine's two deferral switches covers which verbs
      — the resolver clobbers the sculptor's flag for the length of its own call,
      so arming only the member flag would have been an inert change
- [x] 4.4 Make the flush structural rather than written at the end of each path
      that ends a stroke, by holding the record and the sculptor that owes it as
      one value whose disposal recomputes
- [x] 4.5 Test every exit, not the common one: committed, cancelled, tool changed,
      subtool changed, undone mid-drag, document dropped mid-drag, and the same
      under a cage preview. Verify the tests bite by removing the flush and
      watching five of nine fail
- [x] 4.6 Measure the seam rather than quoting it. Deferring cost about fourteen
      per cent more than it saved at every stamp spacing tried; record the numbers
      beside the case rather than the conclusion

## 5. The seed and the grain (upgrade items 7 and 2)

- [x] 5.1 Carry the class and the token together, not the class first. This
      repository passed no class at all, so carrying it alone would have shipped
      the defect the token exists to catch
- [x] 5.2 Capture the seed where the pick is made — it was being discarded at the
      moment it was picked — and read it back where the stroke is made
- [x] 5.3 Add the reach gate the upgrade note does not mention: the surface walk
      abandons a seed farther than the stamp's radius from its centre, so a valid
      seed out of reach loses the dab exactly as a stale one does
- [x] 5.4 Prove the regression by measurement rather than argument: with the token,
      one rejection and the dab lands; with the token struck off, no rejection and
      nothing moved at all
- [x] 5.5 Report the rejection count beside the number of held sculptors, because
      zero over none and zero over four are the same number and different facts
- [x] 5.6 Take `stamp_azimuth` as a brush control in degrees, wrapping rather than
      clamping, and drive it down the path every other brush parameter takes
- [x] 5.7 Test it on a striped stamp, never on a ring — a radially symmetric alpha
      passes at every angle — and at a quarter turn, never at zero

## 6. The items that cost nothing, checked rather than assumed

- [x] 6.1 Item 3 — automask on the adaptive sculptor. There is no adaptive surface
      here and no automask factor was ever set. Rather than leave that unwritten,
      surface all five factors and name the default explicitly at the one call site
      that names every field, so the absence is a decision
- [x] 6.2 Hold the two inert factors as a tripwire with controls beside them, so
      that an equality cannot pass because automasking stopped working altogether
- [x] 6.3 Item 4 — the dirty-chunk drain's error code. No drain loop here branched
      on it. Take the retry rather than offering it: size from the engine's own
      counts and hand back owned buffers, so a caller cannot conflate the two codes
      because it never sees a truncation

## 7. The rest of the release this pin makes reachable

- [x] 7.1 The excluding evaluators (#378): a live smooth composes the rest of the
      scene rather than refusing to open with a second visible field subtool.
      Specified by `a-preview-that-holds-the-whole-scene`
- [x] 7.2 The memory roll-ups: which part, not only how much. Specified by
      `memory-that-says-which-part`
- [x] 7.3 Per-axis layer scale and layer transform readback (#373). Specified by
      `a-subtool-stretches-per-axis`, which also names the format minor
- [x] 7.4 The fourth representation, end to end. Specified by
      `a-hierarchy-the-domain-can-describe`,
      `a-hierarchy-that-is-sculpted-and-saved` and
      `a-stack-of-passes-on-a-hierarchy`
- [x] 7.5 The between-strokes drain. Specified by `maintenance-between-strokes`

## 8. Tripwires, in both directions

- [x] 8.1 Turn the intersect bound around in place following `mask_gate.rs`: keep
      the history in the doc comment, name #319, and assert the finite box is the
      **layer's** and not the item's, which is the thing a future reader will get
      wrong
- [x] 8.2 Correct the four places that state the old behaviour as prose, and the
      documentation that quotes the drag figures as a standing fact
- [x] 8.3 Re-check every other tripwire two ways — the issue number against the
      release notes, and the entry points against the added list — and record why
      three of them needed more than a date stamp
- [x] 8.4 Write tripwires for the two limits the release states about itself, so
      the next release is measured against them rather than against a paragraph

## 9. Make the harness able to carry the answer

- [x] 9.1 Record the samples a figure was reduced from, as a section beside the
      figures so that every committed baseline still compares
- [x] 9.2 Show a figure with one genuine observation as having no spread, rather
      than as having a range of zero width
- [x] 9.3 Mark a change that lands inside the baseline's own spread rather than
      excusing it — within-run spread is the smaller half of the noise, and one
      process cannot sample the half that dominates
- [x] 9.4 Record which build of the engine, not only which version, and announce a
      cross-build comparison above the table rather than refusing it
- [x] 9.5 Add the cases for what this pin brought — the hierarchy, the deferred
      normal flush, the between-strokes drain — knowing they exist on the B side
      only and cannot be compared against the previous pin

## 10. Say what moved

- [ ] 10.1 Run the A/B: the pre-built v0.73.0 binary against this tree, several
      whole runs per pin on a quiet machine, medians read against each other, with
      the pre-flight that every reference member still builds the size it says.
      A filtered run measures a different shape and is not evidence
- [x] 10.2 Update the documentation that states the engine version, and the
      roadmap's account of what the engine gets wrong — two of its entries are
      closed by this pin and two new limits are held as tripwires
