# A refill that gives the frame back

Several commands rebuilt a region of the field synchronously on the interface thread and did not return until the whole of it was rebuilt. The worst case in the audit never returned at all: cancelling a radius-5 tube left the main thread inside `retire_curve_node → drain_dirty → BrickCache::submit` for **over thirty minutes** — the log carries `curva 1,829,156 ms` — at 3.35 GB, and the application did not recover. A subtool boolean took **72.8 and 73.3 s** and produced no layer; copying a hidden subtool took about **66 s** and produced nothing; undo of a radius-4 insert took **576,240 ms**, then 175 s, then 75 s.

## Why it happened

Three separate faults, each of which is enough on its own, all meeting in `drain_dirty`.

- **A sweep was retired by refilling the subtool it was laid on.** `retire_curve_node` asked for the layer's whole extent *and* for the box the layer vacated, which on a worked subtool are the same thing twice: everything the sculptor has ever put in it. A radius-5 tube made that region 2.3M bricks of a 6.7M budget.
- **A bake borrowed the scene's visibility and was charged for it.** Sampling one subtool alone means hiding the others, because `clay_item_volume_from_document` samples the whole document's field and a hidden layer contributes nothing to it. Every flag written marked its layer whole and drained it — so a boolean over a 45-layer document paid roughly 180 whole-layer refills for a field that is the same field on both sides of the bracket. A save under a solo paid the same toll.
- **The drain itself was unbounded.** `drain_dirty` took from the cache until the cache was empty, on the caller's thread, which is the interface thread. However small the region, the work happened between one frame and the next; however large, the window stopped until it was finished.

## What changes

- **A refill has a budget.** `RefillBudget` says what one drain may spend: the whole of it, a duration, or a number of bricks. The cache keeps whatever a bounded drain did not take, so nothing is dropped — `refill_is_pending` says there is more and `pump_refill` spends another budget on it. The application sets half a frame and pumps at the top of every frame, asking for the next one while there is work; a document built headless keeps the whole drain, because a caller with nothing waiting on it needs the exact answer before it returns.
- **The budget sizes the batches as well as stopping between them.** A budget that only stopped between batches of 512 bricks would be that budget plus a whole batch, and a whole batch on a slow backend is most of a frame. Each batch is priced on what the batches before it in the same drain actually cost, with a floor so a drain that has run down to a handful of bricks does not spend a frame's share on one device submission.
- **Retiring a sweep dirties the sweep.** The engine's own bound for the node, asked for before the node is removed, which is everything the tube has ever reached and nothing else. Measured on a three-point tube: the whole layer and the vacated box together, against the node's bound.
- **A borrowed visibility pattern costs nothing.** A bake writes a pattern, samples the document's field through it, and writes the sculptor's pattern back before returning. The fold the cache holds is the same fold on both sides, brick for brick, so nothing is marked and nothing drains. A restore that fails is the one exit that still owes a refill, and it pays it.

## What this deliberately does not do

**It does not stop a bake from hiding layers.** The engine has no per-layer sampler: `clay_item_volume_from_document` samples the document, and hiding the rest of the scene is how one subtool is sampled alone. What is removed here is the *cost* of the borrowing and not the borrowing itself — no refill, and the eyes come back exactly where they were. A sampler that takes a layer would remove it properly, and that is an engine change.

**It does not move the refill onto the worker pool.** `BrickCache::submit` is called from the thread that drains, and the pool it feeds is the engine's own. Stopping the drain is what brings a command under a frame; running it elsewhere would bring the whole refill under a frame, and that is a larger change with a different risk. The budget is what the slow undos needed and is what they now get, since `refill_what_a_step_reached` drains through the same loop.

**It does not add a progress report.** Anything the pump has not finished is named by `outstanding_work`, which is what an agent asks, and the surface fills in over the frames after the command. A figure in the interface is worth having and is not what stopped the window.

## Capabilities

### Modified Capabilities
- `performance-budgets`: a refill is bounded by a budget and continued across frames; a visibility write is bounded by it like everything else; retiring a sweep is bounded by the sweep; a visibility pattern borrowed for the length of one operation re-evaluates nothing.

## Impact

**Code**: `clayspace-engine` (`document.rs`: `RefillBudget`, `drain_dirty`, `refill_batch`, `pump_refill`, `settle_refill`, `retire_curve_node`, `with_borrowed_visibility`, `write_layer_visible`), `clayspace-app` (`main.rs`: the budget is set where a frame starts existing, and pumped at the top of each one).

**What a sculptor sees that is new**: a large refill arrives as a surface filling in over a few frames rather than as a window that has stopped. The document is correct throughout — the field and the drawn surface disagree only where the pump has not reached yet, and the pump keeps asking for frames until it has.

**Risk**: a host that sets a budget and does not pump would leave a surface silently stale. That is why the default is the whole drain and why the application is the only thing that sets a budget; `refill_is_pending` and `outstanding_work` are what make an unpumped refill visible rather than invisible.
