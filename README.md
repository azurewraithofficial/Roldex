# Roldex

Roldex is a lightweight, open-source Roblox Studio and Luau development agent for the terminal, with a companion Roblox Studio plugin.

It is deliberately focused on **Roblox game development**, not general-purpose programming. Roldex can inspect and edit project files, audit Luau, review Git changes, analyze screenshots, retrieve current official Roblox documentation, and accept live context from Roblox Studio.

> Status: v0.1 foundation / early Roldex Studio integration.

## Why Roldex

- Roblox/Luau-first system prompt and tooling
- online AI inference, so large models do not need to live on your PC
- free-provider-first OpenRouter configuration
- bounded project inspection instead of dumping an entire repository into every prompt
- exact-match patches for smaller and safer edits
- workspace path and symlink-escape protection
- deterministic Roblox/Luau security and modernization checks
- current Roblox Creator Hub documentation retrieval
- workspace screenshot/image vision
- Git diff, unstage and guarded single-file restore tools
- live Roblox Studio selection and active-script context
- explicit Studio-side code apply instead of silently replacing scripts

## Install

### Windows

From a cloned copy of this repository, run PowerShell:

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\install.ps1
```

The installer first tries the latest prebuilt Windows release. If no compatible release exists yet, it falls back to `cargo install` when Rust/Cargo is installed. It also installs the Roldex Studio plugin unless you pass `-SkipPlugin`.

### macOS / Linux

```bash
chmod +x ./scripts/install.sh
./scripts/install.sh
```

The installer prefers a matching release binary and falls back to Cargo. On macOS it also installs the Studio plugin by default.

### Cargo directly

If Rust is already installed:

```bash
cargo install --git https://github.com/azurewraithofficial/Roldex roldex-cli --force
```

### Build from source

```bash
cargo build --release -p roldex-cli
```

Cross-platform release binaries are produced by `.github/workflows/release.yml` for Windows x86-64, Linux x86-64, Intel macOS, and Apple Silicon macOS. A `v*` Git tag publishes those build artifacts as a GitHub release.

## Configure AI

Roldex defaults to the OpenRouter-compatible `openrouter/free` route. Create an OpenRouter API key and expose it to Roldex.

Windows PowerShell:

```powershell
$env:OPENROUTER_API_KEY="your-key"
```

macOS/Linux:

```bash
export OPENROUTER_API_KEY="your-key"
```

Provider availability and rate limits can still apply. You can change the provider, endpoint, model, permission mode, and UI settings in `roldex.toml`; start from `roldex.config.example.toml`.

## Run

Open a terminal in your Roblox/Rojo project directory and run:

```bash
roldex
```

By default Roldex also starts its Roblox Studio bridge at:

```text
http://127.0.0.1:38247
```

The bridge binds to loopback only. Use `--studio-port <port>` to choose another port or `--no-studio-bridge` to disable it.

## CLI commands

```text
/help                         Show commands
/status                       Show detected Roblox/Rojo project details
/tree                         Show a bounded project tree
/read <path>                  Read a UTF-8 workspace file
/image <path> :: <prompt>     Analyze a workspace PNG/JPEG/WebP/GIF
/quit                         Exit Roldex
```

Normal chat can use structured agent tools to search the project, read relevant files, run the Luau audit, patch or create files, inspect Git changes, safely unstage one file, restore explicitly requested unstaged edits, and retrieve current official Roblox documentation.

Image input is workspace-scoped and size-limited. The file is encoded for the provider request; Roldex does not require a local vision model.

## Roldex Studio plugin

The companion plugin lives in `plugins/roldex-studio/RoldexStudio.plugin.lua`.

When the CLI is running, the plugin can:

- show a docked Roldex panel inside Roblox Studio
- report the current Studio selection
- include the active Script, LocalScript, or ModuleScript editor source
- send that live Studio context to the same Roldex agent
- display the answer inside Studio
- explicitly apply the first returned Luau code block to the active script through `ScriptEditorService`

Roldex Studio does **not** automatically apply model output. Review the answer and click the Apply button yourself when you want the first code block written to the active editor.

See `plugins/roldex-studio/README.md` for plugin installation and bridge details.

## Smarter Roblox context

Roldex combines several forms of context rather than relying on model memory alone:

1. project detection and bounded project-tree inspection
2. literal text search and relevant-file reads
3. deterministic Luau/Roblox audit findings
4. Git status/diff state
5. live Studio selection and active editor source when the plugin is connected
6. current official Roblox Creator Hub documentation retrieved on demand
7. screenshot/image vision when you provide an image path

The official-documentation tools search Roblox's agent-friendly Creator Hub documentation index and then retrieve only bounded `/docs/...` Markdown pages from `create.roblox.com`. The prompt explicitly keeps Roblox Engine APIs separate from Open Cloud APIs.

## Current Luau audit baseline

The deterministic analyzer currently checks for:

- legacy global scheduler calls such as `wait`, `spawn`, and `delay`
- client-side DataStoreService usage
- deprecated BodyMover usage
- risky server `RemoteFunction:InvokeClient()` usage
- possible non-yielding `while true do` loops
- server scripts placed in replicated locations
- heuristic remote handlers that perform sensitive operations without obvious validation markers

Findings include a rule ID, severity, file, line, message, and remediation. Heuristic findings are leads to inspect, not proof of a vulnerability.

## Safety boundaries

- normal file operations stay inside the configured workspace
- canonical-path checks block symlink/path escapes
- large reads/writes/searches/images are bounded
- Git file paths use literal pathspecs
- repository-wide destructive Git reset workflows are intentionally excluded
- destructive single-file Git restore requires explicit discard intent
- the Studio bridge listens on `127.0.0.1` only and requires the Roldex Studio bridge header
- Studio code replacement requires an explicit plugin Apply action

## Repository layout

```text
apps/roldex-cli/               Interactive CLI and localhost Studio bridge
crates/roldex-core/            Agent runtime, provider, tools, analysis and docs retrieval
plugins/roldex-studio/         Roblox Studio companion plugin
scripts/                       Install helpers
.github/workflows/             CI and cross-platform release builds
docs/                          Roadmap and project documentation
```

## Next upgrades

The main remaining v0.1 runtime item is streaming model output. The next intelligence work is deeper RemoteEvent/DataStore analysis, Rojo-aware placement, richer debugging, and more Studio-native editing/creation operations.

Roldex is not affiliated with Roblox Corporation or OpenAI.
