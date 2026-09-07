# Roldex

Roldex is a lightweight, open-source Roblox Studio development agent for the terminal.

It is intentionally focused on **Roblox Studio and Luau**, rather than general-purpose programming. The long-term goal is a Codex-style workflow for inspecting projects, editing files, debugging Luau, searching Roblox documentation, understanding screenshots, working with Git, and connecting directly to Roblox Studio through a dedicated plugin.

> Status: early v0.1 development.

## Design goals

- Roblox/Luau first
- lightweight RAM and disk usage
- CLI-first workflow
- online AI inference so large models do not need to live on the user's PC
- useful progress updates instead of hidden reasoning dumps
- safe workspace-scoped file operations
- provider abstraction with a free-provider-first default
- future Roldex Studio plugin for Explorer, selection, maps, UI and animation

## Current foundation

The Rust workspace currently contains:

- `roldex-cli` — interactive terminal application
- `roldex-core` — configuration, provider, Roblox prompt, project detection and agent runtime
- default OpenRouter-compatible provider configuration using `openrouter/free`
- Roblox/Rojo/Luau project detection
- workspace permission modes
- structured model tool calls from normal chat
- bounded project tree inspection, text search and file reads
- safe exact-match patches plus create/replace/delete file operations
- Git status and token-efficient Git diff inspection
- workspace path and symlink-escape protection
- deterministic Luau/Roblox audit findings with severity, rule IDs, file/line locations and remediation

The current audit baseline detects legacy scheduler calls, client-side DataStore usage, deprecated BodyMovers, risky `InvokeClient()` usage, possible non-yielding loops, server scripts in replicated locations, and heuristic remote-validation risks.

## Build

Install a current stable Rust toolchain, then:

```bash
cargo build
```

Run Roldex in a Roblox project directory:

```bash
cargo run -p roldex-cli -- --root .
```

For AI chat, create an OpenRouter API key and set it in your environment:

Windows PowerShell:

```powershell
$env:OPENROUTER_API_KEY="your-key"
```

Then start Roldex. The default model route is `openrouter/free`, so Roldex itself does not require a paid model. Provider limits and availability can still apply.

## Commands

Inside the interactive CLI:

```text
/help              Show commands
/status            Show detected project information
/tree              Show a lightweight project tree
/read <path>       Read a UTF-8 file inside the workspace
/quit              Exit Roldex
```

Normal text is sent to the configured AI with the Roblox-specialist system prompt and current project context. The agent can inspect structure, search code, analyze Luau, read relevant files, patch or write files, and review Git changes through structured tools.

## Configuration

Copy `roldex.config.example.toml` to `roldex.toml` in a project when you want custom settings.

## Planned

Next major pieces include Git restore/undo, image-path vision input, Roblox documentation retrieval, streaming output, deeper Luau analysis, richer security audits, and the Roldex Studio plugin.

Roldex is not affiliated with Roblox Corporation or OpenAI.
