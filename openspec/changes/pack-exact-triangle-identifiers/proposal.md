# Pack exact triangle identifiers

## Why
Issue #531 still exceeds 16 ms across reported brush actions. Several ordinary releases spend approximately 17–23 ms in compaction. Exact triangle pruning hashes three machine-sized interned vertex IDs and allocates/copies a new index vector for every brick.

## What changes
Keep complete vertex-bit interning and machine-sized vertex IDs. When the total vertex count guarantees that IDs fit 32 bits, pack each sorted ID triple into a lossless 96-bit value held in u128. Retain the original wide-key path beyond that bound. Compact surviving indices in place while preserving owner order, winding and all surviving vertex bits.

## Impact
Private geometry processing, regression coverage and validation documentation. No engine pin, file format, visible geometry or public API changes. Prototype evidence is promising but requires production/application verification; the full 16 ms goal remains active.
