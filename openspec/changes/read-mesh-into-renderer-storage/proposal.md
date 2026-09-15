# Read mesh into renderer storage

## Why
Issue #531 remains over the brush latency budget. Mesh readback allocates interleaved bytes, initializes them, fills them from the engine, then allocates and decodes a second vertex vector.

## What Changes
Initialize the final renderer vertex vector and safely expose its Pod storage to the existing engine copy API. Preserve default white color, zero mask, exact attributes, index order, empty meshes and errors.

## Impact
Desktop geometry only. Reuse the existing bytemuck version for safe casts. No engine pin, public engine API or file format changes.
