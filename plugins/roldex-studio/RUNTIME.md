# Roldex Studio Runtime

`RoldexStudioRuntime.plugin.lua` is the silent testing companion to the visible `RoldexStudio.plugin.lua`.

The visible plugin owns chat/context and real-time object/script building. The runtime plugin owns bounded Studio-only automation used to verify those builds:

- viewport PNG capture through Studio capture APIs
- device simulation for responsive UI checks
- virtual keyboard/mouse/pointer/text input for the experience under test
- run/play/multiplayer scenario testing
- Studio Output collection
- limited runtime API reflection for engine-class inspection

Both plugins communicate only with the local Roldex bridge at `127.0.0.1`/`localhost` and send the `X-Roldex-Bridge: studio` header.

Roldex routes build/edit commands and test-runtime commands through separate queues so a visual-test feature failure cannot consume or corrupt normal Studio build commands.

Viewport captures are bounded, returned to the CLI, saved under `.roldex/captures/`, and can then be inspected by the vision model as part of Roldex's verify/repair loop.
