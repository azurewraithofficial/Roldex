# Roldex Intelligence Pipeline

Roldex is designed to make Roblox development decisions from evidence rather than only model memory.

## Greenfield preflight

When a prompt proposes a new game/experience concept or game name, Roldex should run live current research before major implementation:

1. search the exact proposed name and close variants
2. search for experiences with strongly similar mechanics/core loops/player fantasy
3. identify name-collision and concept-overlap risk
4. identify meaningful differentiators
5. proceed with a distinct implementation rather than copying another experience

The preflight is advisory evidence. It must never be used to copy another game's protected branding, map layout, assets, code, characters, UI, writing, thumbnails, or other distinctive expression.

## Project intelligence

For existing projects Roldex combines:

- project kind and tree detection
- project-specific `ROLDEX.md`, `.roldex/INSTRUCTIONS.md`, or `AGENTS.md`
- compact Roblox architecture inventory
- literal symbol/text search
- relevant file reads
- Luau/security analysis
- Git state/diffs
- current Roblox Creator Hub documentation
- live Studio selection/editor state

## Studio intelligence

Roldex prefers real Instances for place-building. It can build incrementally while the user watches, then query resulting objects/properties.

The visible Studio plugin handles normal place mutations. A separate silent runtime plugin handles testing capabilities such as viewport capture, device simulation and bounded virtual input.

## Visual intelligence

For appearance-sensitive work the intended verification loop is:

1. build/edit in Studio
2. position/use a meaningful view or gameplay state
3. capture a bounded Studio viewport PNG
4. save capture under `.roldex/captures/`
5. inspect it with the vision model
6. distinguish visible evidence from inference
7. repair concrete visual issues
8. capture/review again when the repair materially changes presentation

Vision is evidence for composition, clipping, scale, lighting, readability and visible interaction states. It does not prove hidden server state, security, collision or performance.

## Runtime testing

Roldex can use bounded run/play/multiplayer tests, Studio Output and virtual input scenarios to exercise representative player actions. Examples include opening an in-game menu, movement controls, activating a Tool, triggering an animation, interacting with a prompt, or checking a responsive UI state.

For significant player-facing work, verification should combine the relevant structural, runtime and visual evidence instead of relying on only one of them.

## Repair loop

Tool failures are fed back to the agent. Roldex should inspect the failure, repair the relevant code/plugin/configuration when possible, retry, and continue until the requested end state is verified or a genuinely external blocker remains.
