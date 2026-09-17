# Design

Allocate fresh slots in the same key order, skip empty geometry, and append each brick at its slot bases. Fill vertex gaps with zero vertices and index tails with the owning vertex base (degenerate triangles). Compute bounds only from live vertices. Upload the resulting two arrays into reserved buffers and retain the layout for ordinary per-key patches.

Keep device budget checks. Prepare into a temporary slot map before reserving GPU buffers; if staging cannot place every brick, refuse the layout and preserve the existing mesh and slot map. A staging allocation costs extra temporary memory and vertex gap bytes; quantify both against reduced queue calls. Do not change incremental writes. Validate exact indexed triangle attributes, padding, empty keys, and subsequent slot reuse. Exercise real GPU rendering and sculpting tests; compare fresh baseline/candidate applications with the same engine revision under a quiet CPU guard.
