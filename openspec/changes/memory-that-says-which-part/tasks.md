# Tasks

## 1. The figures

- [x] 1.1 Add `MemoryDiagnostics` to `clayspace-model` beside `MeshDiagnostics`
      — plain numbers, no engine types, as the rest of that module is — and
      carry the three roll-ups, the total, the surface count and the surface
      bytes
- [x] 1.2 Render them in `to_report`, breakdown before total: the total is the
      part a reader already has an intuition for and the split is the part that
      decides anything

## 2. Fill the host's ledger

- [x] 2.1 `ClayDocument::surface_ledger` — ask every held sculptor for its
      ledger and merge, accumulating onto the first rather than onto a default,
      because merging carries the shorter category count
- [x] 2.2 `ClayDocument::memory` — `memory_with_surfaces`, never the plain
      roll-up
- [x] 2.3 `memory_diagnostics` for the report, reading the engine's roll-ups
      back rather than summing them here

## 3. Show it

- [x] 3.1 A **Memória** section in the diagnostics window, in the three
      locales, with the surfaces row last
- [x] 3.2 Fill `Diagnostics::memory` at the composition root, where the
      renderer and the stalls are already filled

## 4. Hold it

- [x] 4.1 `the_three_roll_ups_account_for_the_whole_the_engine_reports` — the
      roll-ups plus `transient` are the total, on a bare document and on one
      holding a session
- [x] 4.2 `a_mesh_session_reaches_the_report_and_the_plain_roll_up_misses_it` —
      the same document asked twice, and the plain report is short by exactly
      what the ledger named
- [x] 4.3 `a_document_holding_no_surface_reports_what_the_engine_alone_would` —
      an empty ledger is neutral, so the fold is not a second code path
- [x] 4.4 The surfaces row is asserted at zero as well, in the model's own
      tests and in the pasted report the shell captures
