## 1. A channel for the operations the composition root runs

- [x] 1.1 Give the composition root one `Observable<Option<String>>` for the operations it runs itself, beside the five channels the options bar already draws and the door already counts.
- [x] 1.2 Add it to the options bar's one "why that did not happen" line, after the document's own refusal and ahead of every other explicit one — it is the answer to the thing that was just asked for.
- [x] 1.3 Add it to the channels compared either side of a command, first, so it is the refusal an agent is answered with where two channels carry one.
- [x] 1.4 One helper that states an outcome and hands back what it produced: announced on refusal, cleared on success, so no caller has to decide how a refusal is recorded.

## 2. Every operation through it

- [x] 2.1 `run_sculpt_layer_op`: a pass of the active layer's stack.
- [x] 2.2 `run_operation`: the repairs and the deformations.
- [x] 2.3 `run_conversion`: a crossing, which is priced and refused over its budget.
- [x] 2.4 `run_remesh`: a rebuild refused for an unusable resolution.
- [x] 2.5 `run_multires_level_op` and `run_multires_pass_op`: drop `.is_ok()` and state the outcome.

## 3. Hold it

- [x] 3.1 Pull the channel comparison out of the composition root into a function, so the list the door reads can be asserted on rather than reviewed.
- [x] 3.2 `a_refused_operation_reaches_the_options_bar`: removing the channel from the options bar's sources fails.
- [x] 3.3 `a_refused_operation_reaches_the_agent_door`: removing it from the compared channels fails.
- [x] 3.4 `a_sentence_left_over_from_the_last_command_is_not_this_one_s`: what makes a refusal this command's is that the channel was written between the two readings.
- [x] 3.5 `a_substituted_tool_is_a_remark_and_not_a_refusal`: a remark is carried beside the answer rather than as an error.
- [x] 3.6 `a_refused_level_op_is_not_dropped`: the ViewModel states the reason and counts a repeat, which is what makes the composition root's silent branch safe.
