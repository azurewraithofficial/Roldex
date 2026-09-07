# Roldex

Roldex is a lightweight, open-source Roblox Studio and Luau development agent for the terminal, with a companion Roblox Studio plugin.

It is deliberately focused on **Roblox game development**, not general-purpose programming. The goal is an autonomous Roblox workflow: describe the finished result, then let Roldex research when needed, inspect the project/place, build visibly in Studio, write Luau, test, visually inspect, repair failures, verify, and only then report completion.

> Status: active v0.1 development. The architecture is usable, but Roldex is still pre-release software and Studio/runtime coverage continues to expand.

## Highlights

- Roblox/Luau-first agent intelligence
- normal-language interface; slash commands are optional shortcuts
- online AI inference so large models do not need to live on the PC
- OpenRouter-compatible provider routing
- bounded project inventory/search instead of dumping the entire repository into each prompt
- current official Roblox Creator Hub documentation retrieval
- automatic greenfield game/name originality research when using OpenRouter web grounding
- deterministic Roblox/Luau security and modernization audit
- natural local-image understanding and autonomous screenshot QA
- Git status/diff plus guarded file-level undo helpers
- Windows full-access mode for task-relevant local files/tools
- live Roblox Studio selection and active-editor context
- real-Instance-first Studio building with visible incremental actions
- undoable Studio mutation batches
- automated play/run/multiplayer testing
- virtual keyboard/mouse/pointer input for experience-level test flows when available
- Studio device simulation for responsive UI testing
- Studio viewport PNG capture -> local file -> AI vision review -> repair loop

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

The installer prefers a matching prebuilt release, validates it, updates PATH, and installs the Roldex Studio plugin. If no compatible release exists yet, it can fall back to Cargo.

## Configure AI

Roldex defaults to an OpenRouter-compatible configuration using `openrouter/free` for the model route.

Windows PowerShell for the current session:

```powershell
$env:OPENROUTER_API_KEY="your-key"
```

Provider availability, quotas, web-search charges, and rate limits can apply. In particular, automatic live web originality research uses provider web-search infrastructure and is not guaranteed to be zero-cost just because the selected language model route is free.

Optional image/voice generation currently uses `POLLINATIONS_API_KEY` when configured.

## Run

Open PowerShell/Terminal in the Roblox/Rojo project directory:

```text
roldex
```

For a task that genuinely needs files/tools outside the active project:

```text
roldex --full-access
```

Full access is opt-in. Roldex is instructed to inspect and touch only computer files needed for the requested Roblox task.

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

## Greenfield originality preflight

For requests that look like a new game/concept/name, Roldex can automatically perform live research before implementation. It searches for:

- exact and similar proposed names
- experiences with closely related core mechanics
- similar progression loops/player fantasy
- concept/visual-premise overlap
- useful genre references and market gaps

The result is used to avoid accidental near-duplicates and to create deliberate differentiators. Existing games are reference evidence only; Roldex is instructed not to copy their distinctive assets, code, branding, layouts, UI or other protected expression.

If live research is unavailable, Roldex continues with reasonable defaults but must not claim the name/concept is proven unique.

## Roldex Studio

The companion plugin is `plugins/roldex-studio/RoldexStudio.plugin.lua`.

When connected, Roldex can operate on the live Studio data model instead of only editing source files. Current direct Studio operations include:

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

### Real-time building

`studio_batch` executes actions sequentially with a small configurable delay. Roldex therefore prefers:

```text
Workspace
└─ Lobby
   ├─ Floor
   ├─ Walls
   ├─ PortalArea
   ├─ Decorations
   └─ SpawnArea
```

appearing as actual editable Instances while you watch, rather than creating a throwaway script whose only purpose is to generate the map later.

Scripts are still used where runtime logic or genuinely procedural content belongs.

## Visual QA

Roldex Studio can use Studio screenshot APIs to capture the current viewport as PNG. The bridge saves the capture under:

```text
.roldex/captures/
```

Roldex can then send that file through its vision model and review visible issues such as:

- map composition and scale
- visual hierarchy/readability
- lighting balance
- misplaced geometry
- UI clipping/overlap
- responsive layout problems
- whether an expected visible animation/interaction state appeared

The agent can repair the problem, recapture, and review again. Screenshot capture may require a one-time Studio permission prompt. Roldex does not claim visual verification when capture/vision fails.

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

This lets Roldex exercise generic player interactions, UI flows, movement, in-game Tools/abilities, and animation-driven systems instead of only checking that their scripts/Instances exist.

Virtual input is restricted to the experience and may be unavailable in some Studio contexts. If Roblox rejects an automated interaction, Roldex reports that limitation and uses another verification path instead of pretending the test succeeded.

## Roblox intelligence sources

Roldex combines:

1. project/Rojo detection
2. compact project inventory
3. project tree + literal text search
4. relevant file reads and exact patches
5. deterministic Luau audit findings
6. Git state
7. live Studio selection/editor/Instance context
8. current official Roblox documentation
9. live web research for current market/name/ecosystem questions
10. local screenshots and Studio viewport captures
11. runtime logs, simulated inputs and device tests

Project-specific conventions can be supplied in:

```text
ROLDEX.md
.roldex/INSTRUCTIONS.md
AGENTS.md
```

## Full computer access and self-repair

Workspace mode remains the safe default. With `--full-access`, Roldex can access absolute paths that the current OS account itself can access and can run local development programs with bounded time/output.

This exists so Roldex can diagnose supporting-tool failures such as:

- broken Roldex Studio plugin files
- Cargo/build failures
- Git/tooling problems
- Rojo/configuration problems
- task-relevant files outside the project

The agent is instructed to inspect unfamiliar files before changing them, avoid unrelated personal files, try repairing recoverable failures itself, and verify after mutation.

On Windows, Roldex can reinstall its embedded Studio plugin when the local plugin is missing/corrupt. Studio may need a restart to load repaired plugin source.

## Current Luau audit baseline

The deterministic analyzer checks for issues including:

- legacy global scheduler calls (`wait`, `spawn`, `delay`)
- client-side DataStoreService usage
- deprecated BodyMover usage
- risky server `RemoteFunction:InvokeClient()` usage
- possible non-yielding loops
- server scripts in replicated locations
- heuristic remote handlers near sensitive operations without obvious validation

Heuristic findings are leads to inspect, not automatic proof of a vulnerability.

## Security and capability boundaries

- Studio bridge is loopback-only and requires the Roldex bridge header
- ordinary bridge requests remain tightly size-limited; screenshot action results have a separate bounded limit
- workspace mode blocks path/symlink escapes
- full computer mutation requires explicit full-access mode
- Git paths use literal pathspecs
- repository-wide destructive Git reset workflows are intentionally excluded
- Studio mutation batches are recorded in Undo history
- visual/runtime claims require successful evidence rather than model assumption

Roldex cannot bypass Roblox account permissions, moderation, security prompts, plugin security restrictions, publishing permissions or OS permissions. Some human Studio actions intentionally require a user/account decision. When that is the true blocker, Roldex should surface it rather than fake success.

## Repository layout

```text
apps/roldex-cli/               CLI + localhost Studio bridge
crates/roldex-core/            Agent runtime, intelligence, tools, docs, media and Studio broker
plugins/roldex-studio/         Roblox Studio companion plugin
scripts/                       Install/static-check helpers
.github/workflows/             CI and release builds
docs/                          Architecture and roadmap
INSTALL.md                     Installation guide
```

Roldex is not affiliated with Roblox Corporation or OpenAI.
