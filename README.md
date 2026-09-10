# Roldex

Roldex is a lightweight, open-source Roblox Studio and Luau development agent for the terminal, with companion Roblox Studio plugins.

It is deliberately focused on **Roblox game development**, not general-purpose programming. The goal is an autonomous Roblox workflow: describe the finished result, then let Roldex inspect the project/place, build visibly in Studio, write Luau, test, visually inspect when supported, repair failures, verify, and only then report completion.

> Status: active v0.1 development. The architecture is usable, but Roldex is still pre-release software and Studio/runtime coverage continues to expand.

## AI modes

Roldex now supports two inference modes while keeping the same Roblox agent and Studio tools.

### Cloud mode

Cloud mode remains the compatibility default:

```text
Roldex -> OpenRouter -> remote model
```

It keeps local resource usage low, but the selected provider/model can have request quotas, rate limits, or charges.

### Local mode

Local mode connects Roldex to a keyless OpenAI-compatible server running on the same computer:

```text
Roldex -> 127.0.0.1:8080 -> llama-server -> local GGUF model
```

Local inference removes OpenRouter request/day limits from the text/coding path. It still has physical limits: model size, RAM, VRAM, CPU/GPU speed, storage, and power.

Start Roldex against an already-running local server:

```powershell
roldex --local-ai
```

Custom endpoint/model:

```powershell
roldex --local-ai `
  --local-endpoint "http://127.0.0.1:8080/v1/chat/completions" `
  --local-model "roldex-local"
```

Windows users can also use:

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\start-local-ai.ps1 `
  -LlamaServerPath "E:\RoldexAI\llama.cpp\llama-server.exe" `
  -ModelPath "E:\RoldexAI\models\coding-model.gguf" `
  -LaunchRoldex
```

The model/runtime can live on **any drive**, including an external USB drive. Roldex itself does not require the model to be stored on the Windows system drive.

See **[docs/LOCAL_AI.md](docs/LOCAL_AI.md)** for the full local architecture and setup contract.

## Highlights

- Roblox/Luau-first agent intelligence
- normal-language interface; slash commands are optional shortcuts
- cloud or keyless local AI inference
- streaming model responses and persistent progress updates between tool steps
- OpenAI-compatible provider layer
- bounded project inventory/search instead of dumping the entire repository into each prompt
- current official Roblox Creator Hub documentation retrieval
- automatic greenfield game/name originality research when compatible web grounding is available
- deterministic Roblox/Luau security and modernization audit
- natural local-image understanding when the selected model/provider supports vision
- Git status/diff plus guarded file-level undo helpers
- Windows full-access mode for task-relevant local files/tools
- live Roblox Studio selection and active-editor context
- real-Instance-first Studio building with visible incremental actions
- undoable Studio mutation batches
- automated play/run/multiplayer testing
- virtual keyboard/mouse/pointer input for experience-level test flows when available
- Studio device simulation for responsive UI testing
- Studio viewport PNG capture and visual repair loops when a vision-capable inference path is configured
- automatic Studio plugin synchronization/repair on Windows

## Install

See **[INSTALL.md](INSTALL.md)** for copy/paste installation commands and Windows setup.

From a cloned repository on Windows:

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\install.ps1
```

With Rust/Cargo already installed:

```powershell
cargo install --git https://github.com/azurewraithofficial/Roldex roldex-cli --force
```

The installer prefers a matching prebuilt release, validates it, updates PATH, and installs both Roldex Studio plugins. If no compatible release exists yet, it can fall back to Cargo.

## Configure cloud AI

Roldex's cloud default currently uses an OpenRouter-compatible configuration with `openrouter/free`.

Windows PowerShell for the current session:

```powershell
$env:OPENROUTER_API_KEY="your-key"
```

Provider availability, quotas, web-search charges, and rate limits can apply. Roldex cannot bypass a provider account quota.

Optional image/voice generation currently uses `POLLINATIONS_API_KEY` when configured.

## Configure local AI

Local mode needs **no OpenRouter key**.

An OpenAI-compatible server must be running first. Roldex's intended local runtime is `llama.cpp`/`llama-server`, although the provider layer is intentionally compatible with other local servers that implement the same chat-completions/tool-call protocol.

Default local endpoint:

```text
http://127.0.0.1:8080/v1/chat/completions
```

Environment-based configuration is also supported:

```powershell
$env:ROLDEX_AI_MODE="local"
$env:ROLDEX_AI_ENDPOINT="http://127.0.0.1:8080/v1/chat/completions"
$env:ROLDEX_MODEL="roldex-local"
roldex
```

An example config is included at **[roldex.local.example.toml](roldex.local.example.toml)**.

Large `.gguf` model files and local runtime binaries are intentionally not stored in the Roldex repository.

## Run

Open PowerShell/Terminal in the Roblox/Rojo project directory:

```text
roldex
```

For a task that genuinely needs files/tools outside the active project:

```text
roldex --full-access
```

For local inference:

```text
roldex --local-ai
```

The Studio bridge listens on loopback by default:

```text
http://127.0.0.1:38247
```

Keep the CLI open while using the Studio plugin.

## Natural-language workflow

Examples:

```text
Build a complete sci-fi lobby for this game and test it.

This is a new game concept called Neon Salvagers. Check if the name/concept is too close to existing Roblox games, improve it if needed, then build the first playable version.

Make this UI work well on desktop and phone, then visually verify both layouts.

Fix the current round system. Keep testing and repairing it until the runtime errors are gone.

Look at "screenshots/studio-error.png" and fix what is causing it.
```

Roldex does not require `/image`, `/web`, or other mode commands for normal requests. It infers intent and chooses tools itself.

During multi-step work, short user-visible progress messages are retained instead of disappearing when Roldex transitions into a tool call.

## Roldex Studio

The visible companion plugin is:

```text
plugins/roldex-studio/RoldexStudio.plugin.lua
```

The automated runtime/testing companion is:

```text
plugins/roldex-studio/RoldexStudioRuntime.plugin.lua
```

On Windows, Roldex synchronizes its bundled plugin copies at startup and can force repair them with:

```powershell
roldex --repair-studio-plugin
```

Restart Roblox Studio after plugin replacement so Studio reloads the source.

When connected, Roldex can operate on the live Studio data model instead of only editing source files. Current direct operations include:

- inspect selection, Instances, children, properties and attributes
- find objects by hierarchy/name/class
- create real Roblox Instances
- set properties and attributes
- clone, move/reparent and delete objects
- select created/changed objects so live work is visible
- add/remove CollectionService tags
- create/update Script, LocalScript and ModuleScript source using `ScriptEditorService`
- move Models/BaseParts by pivot/CFrame
- create/edit GUI hierarchies using normal Instances/properties
- create Parts, Models, Attachments, Constraints, effects, lights, sounds, remotes and other creatable Instances
- fill/clear Terrain primitives
- undo/redo through Studio history

## Real-time building

`studio_batch` executes actions sequentially with a small configurable delay. Roldex therefore prefers actual editable Instances appearing while you watch rather than generating a throwaway setup script for everything.

Scripts are still used where runtime logic or genuinely procedural content belongs.

## Visual QA

Roldex Studio can use Studio screenshot APIs to capture the current viewport as PNG under:

```text
.roldex/captures/
```

A vision-capable model/provider can then review visible issues such as map composition, scale, visual hierarchy, lighting, misplaced geometry, UI clipping, responsive layout, and expected interaction state.

A text-only local coding model does **not** automatically gain vision. Roldex must not claim visual verification unless an actual vision-capable inference path successfully inspected the image.

## Automated testing

The Studio testing layer supports:

- Run mode smoke tests
- Play mode player/client tests
- multiplayer tests with up to 8 simulated clients
- Output/error/warning collection
- bounded test timeouts
- optional screenshot capture during a test
- simulated experience-level keyboard input
- simulated mouse clicks/movement/position
- simulated pointer/scroll/pan/pinch input
- text-input simulation
- device/resolution/orientation emulation
- real chat-message test harnesses where supported

This lets Roldex exercise player interactions, UI flows, movement, Tools/abilities, chat customization, networking, and animation-driven systems instead of only checking that scripts/Instances exist.

## Roblox intelligence sources

Roldex combines:

1. project/Rojo detection
2. compact project inventory
3. project tree + literal text search
4. relevant file reads and exact patches
5. deterministic Luau audit findings
6. Git state
7. live Studio selection/editor/Instance context
8. official Roblox documentation tooling
9. optional live web research
10. local screenshots and Studio viewport captures
11. runtime logs, simulated inputs and device tests

Project-specific conventions can be supplied in:

```text
ROLDEX.md
.roldex/INSTRUCTIONS.md
AGENTS.md
```

## Local model specialization

Roldex does not need to train a model from scratch. The intended path is:

```text
open coding model
    + Roldex Roblox/Luau instructions
    + project/Studio context
    + Roblox tools
    + deterministic analyzers
    + Roblox-specific evaluation
    + optional RAG / adapter fine-tuning later
```

Candidate open models can be evaluated for Luau quality, Roblox API accuracy, function-calling reliability, multi-step repair behavior, latency, RAM/VRAM use, and Studio planning quality before Roldex recommends one as a default.

## Full computer access and self-repair

Workspace mode remains the safe default. With `--full-access`, Roldex can access absolute paths that the current OS account itself can access and can run local development programs with bounded time/output.

The agent is instructed to inspect unfamiliar files before changing them, avoid unrelated personal files, repair recoverable failures itself, and verify after mutation.

## Security and capability boundaries

- Studio bridge is loopback-only and requires the Roldex bridge header
- recommended local model server is also loopback-only
- workspace mode blocks path/symlink escapes
- full computer mutation requires explicit full-access mode
- Git paths use literal pathspecs
- repository-wide destructive Git reset workflows are intentionally excluded
- Studio mutation batches are recorded in Undo history
- visual/runtime claims require successful evidence rather than model assumption
- local mode removes provider quotas only from inference actually performed locally

Roldex cannot bypass Roblox account permissions, moderation, security prompts, plugin security restrictions, publishing permissions, OS permissions, model hardware requirements, or third-party provider quotas.

## Repository layout

```text
apps/roldex-cli/               CLI + localhost Studio bridge
crates/roldex-core/            Agent runtime, intelligence, tools, providers and Studio broker
plugins/roldex-studio/         Roblox Studio companion plugins
scripts/                       Install, validation and local-AI launcher helpers
.github/workflows/             CI and release/dev builds
docs/                          Architecture, local AI docs and roadmap
INSTALL.md                     Installation guide
THIRD_PARTY_NOTICES.md         Upstream project/license notes
```

## Upstream/open-source references

Roldex evaluated `ggml-org/llama.cpp`, `openai/codex`, `QwenLM/qwen-code`, and `openai/gpt-oss` while designing the local stack. Roldex currently keeps its own Roblox-specific agent code rather than vendoring those projects. See **[THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md)**.

Roldex is not affiliated with Roblox Corporation or OpenAI.
