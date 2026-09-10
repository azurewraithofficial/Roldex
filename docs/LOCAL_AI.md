# Roldex Local AI

Roldex can use a locally hosted OpenAI-compatible model server instead of OpenRouter. The intended runtime is `llama.cpp` (`llama-server`), but any compatible local server may be used.

Local mode is designed for users who want to avoid provider request quotas and per-request API charges. It does not make compute unlimited: model size, RAM, VRAM, CPU/GPU speed, storage, and power remain physical constraints.

## Architecture

```text
Roldex CLI
   |
   | OpenAI-compatible chat-completions API
   v
127.0.0.1:8080
   |
   v
llama-server
   |
   v
local GGUF coding model

Roldex CLI <-> localhost Studio bridge <-> Roblox Studio plugins
```

The same Roldex Roblox system prompt, filesystem tools, shell tools, Git tools, Studio bridge, test loop, screenshots, progress updates, and repair logic remain in use. Local mode changes the inference provider; it does not create a separate reduced-capability chatbot.

## GitHub intentionally does not contain model weights

Do not commit `.gguf` model files or local inference binaries to the Roldex repository. They can be many gigabytes and should live on a user-selected drive.

A future installation can use, for example:

```text
E:\RoldexAI\
├── llama.cpp\
│   └── llama-server.exe
├── models\
│   └── coding-model.gguf
└── cache\
```

The drive letter is not significant. An internal SSD, external SSD, external HDD, or sufficiently fast USB storage can all be used for storage. Runtime speed still depends mainly on the computer's RAM/VRAM/CPU/GPU once the model is loaded.

## Start an existing local server

If `llama-server` is already running on port 8080:

```powershell
roldex --local-ai
```

Defaults:

```text
endpoint: http://127.0.0.1:8080/v1/chat/completions
model:    local-model
API key:  not required
```

Custom endpoint/model:

```powershell
roldex --local-ai `
  --local-endpoint "http://127.0.0.1:9000/v1/chat/completions" `
  --local-model "roldex-local"
```

## Start llama.cpp and Roldex together on Windows

Roldex includes `scripts/start-local-ai.ps1`.

Example:

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\start-local-ai.ps1 `
  -LlamaServerPath "E:\RoldexAI\llama.cpp\llama-server.exe" `
  -ModelPath "E:\RoldexAI\models\coding-model.gguf" `
  -LaunchRoldex `
  -FullAccess
```

The script:

1. validates the model and server paths;
2. binds the model server to `127.0.0.1` only;
3. enables llama.cpp Jinja chat templates for tool/function calling;
4. starts the server with a configurable context size and GPU-layer setting;
5. waits for the public llama.cpp `/health` endpoint;
6. configures Roldex local mode for the current PowerShell process;
7. optionally launches Roldex.

Useful parameters:

```text
-ModelPath        required GGUF model path
-LlamaServerPath  llama-server executable path
-ContextSize      default 32768
-Port             default 8080
-GpuLayers        default auto
-ModelAlias       default roldex-local
-LaunchRoldex     launch Roldex after the server is healthy
-FullAccess       pass --full-access to Roldex
```

## Environment configuration

Local mode can also be configured without CLI flags:

```powershell
$env:ROLDEX_AI_MODE="local"
$env:ROLDEX_AI_ENDPOINT="http://127.0.0.1:8080/v1/chat/completions"
$env:ROLDEX_MODEL="roldex-local"
roldex
```

Supported provider overrides:

```text
ROLDEX_AI_MODE
ROLDEX_AI_PROVIDER
ROLDEX_AI_ENDPOINT
ROLDEX_MODEL
ROLDEX_API_KEY_ENV
ROLDEX_AI_TIMEOUT_SECONDS
ROLDEX_OPENROUTER_SORT
```

## Candidate open models

Roldex is intentionally model-agnostic. Coding/agent models with reliable tool calling are preferred.

Candidates to evaluate include:

- Qwen coding models / Qwen3-Coder family
- OpenAI gpt-oss-20b
- other GGUF models with strong coding and tool/function-calling behavior

Do not select a model by benchmark score alone. Roldex should evaluate each candidate on Roblox-specific tasks: Luau correctness, Roblox API knowledge, tool-call reliability, multi-step repair, Studio command planning, security rules, and latency on the target PC.

## Roblox specialization

The local base model is only one layer. Roldex provides the Roblox specialization through:

- Roblox/Luau system instructions;
- live Studio selection/editor context;
- filesystem/project inspection;
- Studio mutation/query tools;
- deterministic Luau/security analysis;
- test and repair loops;
- screenshot/visual QA when the selected inference stack supports images;
- project-specific `ROLDEX.md`, `.roldex/INSTRUCTIONS.md`, or `AGENTS.md` instructions.

Future specialization can add Roblox-focused RAG and a LoRA/fine-tuned adapter without changing the user-facing CLI/Studio workflow.

## Web and vision capabilities

A local text-only model does not automatically gain live web search or image understanding. Roldex must not pretend otherwise.

The architecture should treat these as optional capabilities:

- local coding/text inference: no provider quota;
- local vision model: optional, when configured;
- live web research: optional external network capability;
- cloud provider: optional fallback, never required for local text inference.

## Security

- The recommended local server binds to `127.0.0.1`, not the public network.
- Local mode does not require `OPENROUTER_API_KEY`.
- Full filesystem access remains opt-in through `--full-access`.
- Roblox Studio continues to communicate with Roldex through its separate loopback bridge on port 38247 by default.

## Upstream projects evaluated

Roldex's local architecture is informed by established open-source projects rather than copying proprietary model services:

- `ggml-org/llama.cpp` — local GGUF inference and OpenAI-compatible server, MIT license.
- `openai/codex` — open-source terminal coding agent, Apache-2.0 license.
- `QwenLM/qwen-code` — open-source terminal coding agent, Apache-2.0 license.
- `openai/gpt-oss` — open-weight model/reference code, Apache-2.0 license.

Roldex keeps its own Roblox-specific agent/runtime code. If substantial upstream source is vendored in the future, the applicable upstream copyright/license notices must be preserved.
