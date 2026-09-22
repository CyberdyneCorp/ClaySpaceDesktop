## 1. The three gesture sets, said once

- [x] 1.1 `Command::opens_a_gesture`, `closes_a_gesture` and `continues_a_gesture`, beside the vocabulary rather than at each caller.
- [x] 1.2 `Command::changes_the_document`, the wide question, over the seven commands that mark the document on the composition root's own path and so answer `false` to `touches_document`.
- [x] 1.3 The composition root's `AgentGesture` asks the vocabulary instead of carrying its own copy of the list.

## 2. The narrow exemption

- [x] 2.1 Split the composition root's one question in two: whether *anything* is open, and whether the one that is open is the agent's.
- [x] 2.2 The agent's hold is the flag **and** an open gesture, so a begin the ViewModel refused cannot wedge the door for the rest of the session.
- [x] 2.3 `Session::agent_gesture_in_progress`, beside `gesture_in_progress`, because the two carry different rules.
- [x] 2.4 The door refuses a changing command that does not continue the agent's own gesture, naming it.
- [x] 2.5 `measure` is refused for either gesture: a figure taken across an open stroke measures the stroke too.

## 3. A cage that refuses on every path

- [x] 3.1 `SculptModel::active_layer_is_caged`, forwarded by the shared document and answered from the held cage.
- [x] 3.2 `Unavailable::LayerCaged`, so the refusal says what to do about it.
- [x] 3.3 The sculpting ViewModel refuses a begin while a cage is up, before it collects anything.
- [x] 3.4 Raising a cage clears the whole-subtool manipulator's target.

## 4. Hold it

- [x] 4.1 `only_the_verbs_that_finish_a_gesture_continue_one`: the three sets do not overlap and a begin is in none of the two a holder may send.
- [x] 4.2 `an_agent_may_only_continue_its_own_gesture`: a table over the verb list, asserting which go through while the agent holds a gesture.
- [x] 4.3 `a_measurement_is_refused_during_the_agents_own_gesture`.
- [x] 4.4 `a_stroke_is_refused_while_a_cage_is_up`, against the ViewModel rather than through the pointer.
- [x] 4.5 `a_raised_cage_is_reported_to_the_sculpt_model`: the engine seam the ViewModel's guard stands on.
- [x] 4.6 `raising_a_cage_clears_the_object_target`, with a drag afterwards that reaches nothing.
