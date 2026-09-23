## 1. The decoder refuses what it cannot honour

- [x] 1.1 `Args::accept_only`: refuse a key the action's row does not declare, listing the ones it does; `build` checks it against the row before decoding, with the call's envelope let through.
- [x] 1.2 `Args::whole`, `whole_or`, `optional_whole` and `count`: one checked conversion for every count, size, key and index, replacing each `as u32`, `as u64` and `as i32`.
- [x] 1.3 `Args::number` and the list readers: refuse a value that is not finite once narrowed to `f32`.
- [x] 1.4 `optional_number` and `vec2_or` in place of the NaN sentinels and the thrown-away offset refusal.
- [x] 1.5 Refuse an unknown retopology method and an unknown bake map rather than defaulting.
- [x] 1.6 `brush set_azimuth`: degrees to radians, as the panel converts.

## 2. A clamp is reported

- [x] 2.1 `Args::note_clamp` and `clamped`: record `{argument, asked, used}` where the two differ.
- [x] 2.2 The brush's numbers through `BrushSettings::sanitized`; each settings block through its own `sanitized`; opacity and grid blur through their constructors.
- [x] 2.3 The group call's answer carries `clamped` when anything was.

## 3. Hold it

- [x] 3.1 `unknown_keys_are_rejected`: every row refuses an undeclared key and lists its own.
- [x] 3.2 `every_argument_the_builder_reads_is_declared`: under every combination of a row's choices, the decoder reads only declared names, and reads every declared one somewhere.
- [x] 3.3 `a_value_that_cannot_be_honoured_is_refused_wherever_it_is_read`: for every numeric argument, an infinite, fractional or negative value is refused in every variant that reads it.
- [x] 3.4 `clamped_values_are_reported` and `a_value_already_in_range_reports_no_clamp`.
- [x] 3.5 `a_clamped_value_is_reported_in_the_answer` and `an_unknown_key_changes_nothing`, through the catalogue.
- [x] 3.6 `docs/features.md` states the argument contract.
