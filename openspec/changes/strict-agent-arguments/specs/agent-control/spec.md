## ADDED Requirements

### Requirement: An argument the application cannot honour is refused, and a clamp is reported
A group tool SHALL refuse, with a message naming the argument, any argument it
cannot honour as given, and SHALL change nothing when it does. In particular:

- a key the action does not declare SHALL be refused, and the refusal SHALL
  list the keys the action does take. The call's own envelope — the action's
  name and a capture riding along with the answer — is not an argument of the
  action and SHALL NOT be refused as one;
- a whole number that does not fit the field it sets — a negative count, size,
  index or key, or one too large to hold — SHALL be refused, and SHALL NOT be
  converted by wrapping or truncation;
- a fraction where a whole number is expected SHALL be refused;
- a number that is not finite at the precision the application holds it in
  SHALL be refused, for every numeric argument and every element of a list of
  numbers;
- a value that is not one of an argument's named choices SHALL be refused, and
  SHALL NOT fall back to a default.

A selection that an argument can clear SHALL be cleared by leaving the argument
out, not by a sentinel value.

Where the application brings a value into range rather than refusing it — as
its own panels do — the command SHALL apply, and the answer SHALL report each
such argument with the value asked for and the value used. The rule used for
the report SHALL be the one the application applies, not a restatement of it.

#### Scenario: A misspelt argument is refused
- **WHEN** a client calls `brush` `set_size` with `sizee` rather than `size`
- **THEN** it is refused naming `sizee` and listing `size`, and the brush is
  unchanged

#### Scenario: A negative count is refused rather than wrapped
- **WHEN** a client calls `layer` `set_remesh` with a resolution of -1
- **THEN** it is refused saying the resolution cannot be negative, and the
  remesh settings are unchanged

#### Scenario: A number too large for the application is refused
- **WHEN** a client calls `exchange` `set_import` with a scale of `1e308`
- **THEN** it is refused as not a finite number, and the import settings are
  unchanged

#### Scenario: A clamped value is reported with the value used
- **WHEN** a client calls `view` `set_surface_opacity` with an opacity of 5
- **THEN** the command applies, and the answer reports `opacity` asked as 5 and
  used as 1

#### Scenario: A value in range reports nothing
- **WHEN** a client calls any action with values inside the ranges the
  application applies
- **THEN** the answer reports no clamp
