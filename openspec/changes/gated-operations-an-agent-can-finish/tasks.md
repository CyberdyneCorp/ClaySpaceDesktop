## 1. Paths instead of panels

- [x] 1.1 `SaveTo`, `ImportFrom` and `ExportTo` commands; the app splits the panel from the write so both routes share it.
- [x] 1.2 `document.save_as`, `exchange.run_import {path}` and `exchange.run_export {path}`; the panel commands are not offered.
- [x] 1.3 The gates cover the path commands and show the path in the ask.
- [x] 1.4 `export_accepts_a_path` and `an_exchange_without_a_path_is_refused_before_anything_is_asked`.

## 2. No native dialog on the door's path

- [x] 2.1 `refused_at_the_door`: a never-saved save, an open or quit over unsaved work, and a plain switch away from a dragged cage.
- [x] 2.2 `layer.select` takes `cage`; `SelectLayerSettlingCage` applies or discards before switching.
- [x] 2.3 `select_with_a_cage_takes_an_argument`, and the end-to-end cage test.

## 3. The consent wait fits the call

- [x] 3.1 `Bounds::consent_wait` held inside the call bound; default 8 s.
- [x] 3.2 `consent_fits_the_call_bound`.

## 4. Startup

- [x] 4.1 The recovery offer is a shell window answered with `AnswerRecovery`; no autosave over it, kept across a clean quit.
- [x] 4.2 The banner is printed after the door opens, with one agent line; `the_startup_banner_states_the_door_once`.

## 5. Persistence verified

- [x] 5.1 `document_round_trip`: masks, a mirrored stroke, a grid pass, a transform, a rig and a hierarchy saved and reopened, compared by digest.
- [x] 5.2 `a_document_saved_over_the_door_reopens_as_it_was` over the real application.
- [x] 5.3 README and `docs/features.md` state how gated operations complete for an agent.
