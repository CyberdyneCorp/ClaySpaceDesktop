# Borrow exact pruning vertex keys

## Why
Issue #531 remains above the 16 ms brush budget. Exact triangle pruning copies forty bytes per vertex into a temporary hash table although immutable vertex storage already exists for the entire pass.

## What Changes
Borrow vertex data for temporary interning keys. Preserve full bit equality, hash randomization, sorted brick ownership, triangle order and the packed/wide fallback.

## Impact
Only desktop geometry scratch storage changes. No API, file format, engine pin or brush semantics change.
