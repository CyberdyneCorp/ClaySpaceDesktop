## 1. One registry instead of two lists

- [x] 1.1 `App::refusal_channels`: every channel a refusal arrives on, in the order the answer belongs to the command, written once.
- [x] 1.2 `App::remark_channels`: the same for the channels carrying something that did happen.
- [x] 1.3 Read the count before a command and the words after from that one list, so the two can no longer disagree position by position.
- [x] 1.4 Keep the width a constant the registry must match, so a channel added without widening it does not build.

## 2. The panels that were never read

- [x] 2.1 The cage, the curve, the boolean and the rig — the four the last change named and left.
- [x] 2.2 The cut and the reference panel, which refuse on the spot for the same reason and were missing for the same reason.
- [x] 2.3 The retopology, UV, conform and bake panels, for the refusals they answer at the start; a completion that fails is still not carried, and says so in the proposal.

## 3. The screen, not only the door

- [x] 3.1 A refused cage, curve or rig reaches the options bar's one "why that did not happen" line. The cage had no line at all.
- [x] 3.2 The rig's `eprintln!` in `after_armature_edit` goes: it is drawn and counted now, and a sentence in a terminal nobody has open is not a second surface.

## 4. Hold it

- [x] 4.1 `every_notice_channel_is_read`: read `struct App` for the ViewModels the shell holds, read each for the notice channels it owns, fail naming any the registry does not read.
- [x] 4.2 `a_refused_cage_reaches_the_options_bar`: removing the cage, the curve or the rig from the options bar's sources fails.
- [x] 4.3 `a_refusal_on_the_last_channel_is_still_the_command_s_answer`: every channel in the list is read, not the first few.
- [x] 4.4 `a_refused_boolean_run_is_an_error` and `a_refused_cage_command_is_an_error`, against the running application.
