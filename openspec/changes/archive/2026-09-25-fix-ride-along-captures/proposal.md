# Proposal

## Why

A command's attached screenshot can show the previous interface frame, while a whole-window screenshot places the scene outside the window's viewport rectangle. This makes visual verification disagree with the command result.

## What Changes

- Draw the command's frame before taking an attached capture and preserve the viewport rectangle in window captures.
- Scope remembered captures to each MCP session.
- Allow a capture to use a camera preset for that frame while preserving the live camera.

## Capabilities

### Modified Capabilities

- `agent-observation`: Captures show the command result with correct framing, caller-owned memories, and a temporary camera view.

## Impact

The application capture path, MCP protocol and catalogue, the agent observation spec, and the windowed regression harness change. The capture request accepts an optional camera preset.
