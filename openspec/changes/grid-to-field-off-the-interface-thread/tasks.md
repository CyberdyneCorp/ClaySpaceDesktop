# Tasks

- [x] Read a grid's active level out as plain data and rebuild an owned grid
      from it, with a test that the snapshot converts to the same field as the
      document's own conversion, on another thread.
- [x] Split the grid-to-field crossing into read, convert and place, with the
      synchronous crossing running the same parts; engine tests for the result,
      the undo step, an in-place crossing, a grid changed or taken back while
      converting, and the refusals before anything starts.
- [x] Run the crossing as a background job in the application, report it as
      outstanding, refuse a second one, and drop it when the document is
      replaced.
- [x] Measure the crossing at 5k, 23k and 100k cells in the benchmark, split
      into interface and worker time, with budgets.
- [x] Pin grid mask extrude at 0.3, 0.1 and 0.05 with an engine test.
- [x] Document the behaviour and the budgets in `docs/features.md` and the
      agent catalogue.
