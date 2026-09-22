## 1. A meter that is read on a clock

- [x] 1.1 Hold the status area's memory figures and the moment they were read, and take a fresh reading only once the last has aged out.
- [x] 1.2 Decide from the frame's own clock rather than reading one, so the memo can be driven through an hour of frames in a test without sleeping through them.
- [x] 1.3 Keep the last figures when a reading fails, and still spend the interval, so a cache that has stopped answering is not asked once a frame.
- [x] 1.4 Take a fresh reading at once when the document is replaced, beside the other staleness that swap forgets.
- [x] 1.5 Read the agent's state report through the same meter, so it agrees with the status area and cannot restore the per-call walk.

## 2. Hold it

- [x] 2.1 `crates/clayspace-app/src/main.rs`: two frames with nothing between them walk the cache once; a second of frames is one reading; the figure is read again once it has aged out; a replaced document is read at once; a failed reading keeps the last figure and costs one call rather than one per frame.
