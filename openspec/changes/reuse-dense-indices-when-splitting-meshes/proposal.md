# Reuse dense vertex indices during mesh splitting

## Why

ClayCore#531 remains above its 16 ms brush-action target. Live profiling measures about 11–12 ms splitting a full engine mesh into per-brick geometry. Each brick creates a hash map to translate global vertex indices into local indices, although valid global indices already address a dense vertex array.

## What Changes

Investigate replacing per-brick hashing with one temporary dense remap reused across the split. Preserve first-encounter local vertex order, complete vertex bits, triangle order and empty-entry behavior. Keep the existing permissive handling of invalid global indices through a fallback. Verify correctness and measure speed and scratch memory before adoption.

## Impact

Only transient application geometry construction changes. Rendering, field evaluation, brick ownership, wire formats and engine pins retain their contracts. This increment does not establish the 16 ms target by itself.
