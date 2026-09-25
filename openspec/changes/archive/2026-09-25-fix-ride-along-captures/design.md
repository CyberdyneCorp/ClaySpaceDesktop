# Design

## Context

Commands run on the interface thread between window events. The capture path retained primitives from the last presented frame and cleared the renderer's scene viewport for every offscreen target. MCP sessions share one catalogue instance.

## Goals / Non-Goals

**Goals:** Make attached captures correspond to applied commands, preserve whole-window scene framing, isolate remembered frames, and offer a temporary camera view.

**Non-Goals:** Rework the renderer's offscreen target allocation or the settle/upload path.

## Decisions

- Draw a fresh application frame immediately before capturing. This updates geometry, layout and egui primitives through the same path as the window. Rebuilding only egui would risk mismatching scene state.
- Use the active MCP session ID to key remembered images. The protocol passes it to the catalogue while retaining the existing unscoped trait method for in-process callers and tests.
- Keep camera overrides in the capture request and apply them to a local camera copy. This gives a set-and-restore capture without disturbing the live camera or requiring shared mutable camera slots.

## Risks / Trade-offs

- A capture draws an additional window frame, increasing its cost. Captures are explicit and already include an offscreen render; this favors correct evidence.
- Windowed image comparisons can vary slightly by GPU. The regression compares the panel change against an immediate follow-up frame with a wide margin.

## Migration Plan

No stored data changes. Existing clients can omit the camera argument. Deploy with the app and MCP server in the same build; rollback reverts the code and spec.
