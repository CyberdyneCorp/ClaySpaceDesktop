# Offer dispatched agent workflows

## Why

The command router already accepts cut, retopology, UV, conform and bake
actions, but their groups and actions are absent from the agent catalogue.
Clients cannot discover or call them through the published tool schemas.
The catalogue also omits six brush controls and curve point insertion.

## What changes

- Declare the five missing groups and their 16 dispatchable actions in the
  catalogue, with arguments and exercised examples.
- Declare the six brush controls and curve point insertion; connect the latter
  to its existing model command.
- Check catalogue rows, command routing and command homes against each other
  in both directions, so a route cannot silently disappear from discovery.
- Update the documented tool list and count from 24 to 29.

## Scope

The existing model commands and their behavior remain the source of truth.
Starting a texture bake remains behind the application's destination file
panel, so the bake group offers settings and cancellation only.

## Impact

The agent-facing tool list and `describe` gain the missing groups and actions.
Previously accepted unlisted routes gain the same argument declaration and
validation as all other catalogue actions.
