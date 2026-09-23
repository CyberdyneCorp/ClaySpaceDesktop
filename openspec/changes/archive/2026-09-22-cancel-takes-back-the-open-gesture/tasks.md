## 1. A line the gesture cannot reach past

- [x] 1.1 Read the model's history depth when a gesture opens, before anything the gesture does reaches the model, and hold it for as long as the gesture is open.
- [x] 1.2 Revert down to that depth rather than spending the entries the segments were counted as producing, so one record and twenty cost the cancel exactly what each of them wrote.
- [x] 1.3 Drop the line when the gesture closes, so a cancel arriving after the release has nothing of its own to take back.

## 2. A cancel with nothing open

- [x] 2.1 Stop before the model is told a gesture ended, so a cancel an agent repeats settles the document no further.
- [x] 2.2 Report that there was nothing to cancel rather than reporting the stroke that came before it.

## 3. Hold it

- [x] 3.1 `crates/clayspace-vm/tests/viewmodel.rs`: against a double that banks a gesture the way a mesh layer does — cancel reverts only the open gesture; a fresh layer survives its first gesture being cancelled; a cancel with nothing open changes nothing and says so; the field path still spends one entry per segment.
- [x] 3.2 `crates/clayspace-app/tests/stroke_cancel.rs`: against the engine — two mesh gestures committed, a third opened and cancelled, and the geometry digest, the subtool count and the history the interface reads are all where the second gesture left them.
