## 1. The substitute

- [x] 1.1 `ToolKind::intent` and `ToolKind::substitute_on`, read off the
      binding table (#217)
- [x] 1.2 `every_substitute_is_a_tool_the_layer_carries`,
      `a_substitute_means_the_same_act_where_the_layer_has_one`,
      `the_fallback_is_the_same_tool_on_every_representation`

## 2. The settings

- [x] 2.1 `BrushSettings::default_for`, and the ViewModel's slots start from it
- [x] 2.2 `every_default_brush_fits_its_representation`;
      `settings_are_kept_per_tool_and_representation`,
      `no_representation_inherits_a_brush_size_from_another`
- [x] 2.3 `the_stamp_radius_used_is_the_target_layers_setting`, against a real
      document, reading the radius the engine was handed

## 3. The report

- [x] 3.1 The ViewModel holds the chosen tool while a stand-in is in hand;
      `an_unsupported_tool_falls_back_deterministically`,
      `a_substitution_is_reported`,
      `the_chosen_tool_returns_with_a_layer_that_carries_it`
- [x] 3.2 The agent door names both tools in the answer
      (`a_substitution_is_named_to_an_agent`) and `state.tool.stands_in_for`
      (`a_substituted_tool_names_the_tool_it_stands_in_for`)
- [x] 3.3 `docs/features.md`: the rule, the table of substitutes, the defaults
