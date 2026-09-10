# Installing Roldex

Roldex is designed to be installed once, then launched from any Roblox/Rojo project folder. The CLI supports both cloud inference and an optional local OpenAI-compatible model server.

## Windows — one-line development install

Open **PowerShell** and paste:

```powershell
Set-ExecutionPolicy -Scope Process Bypass -Force; $env:ROLDEX_REF='roldex-intelligence-studio-vision'; irm 'https://raw.githubusercontent.com/azurewraithofficial/Roldex/roldex-intelligence-studio-vision/scripts/install.ps1' | iex
```

That installs the CLI and both Roblox Studio plugins. It does **not** clone the Roldex repository permanently to the PC.

Once the development branch is merged/released, the stable installer will use `main` instead.

### What the installer does

1. Detects Windows x64 vs ARM64.
2. Prefers a verified prebuilt `roldex.exe`.
3. Verifies the executable checksum when a matching prebuilt is available.
4. Installs it under `%LOCALAPPDATA%\Roldex\bin`.
5. Adds Roldex to the user PATH/current PowerShell session.
6. Installs `RoldexStudio.plugin.lua`.
7. Installs `RoldexStudioRuntime.plugin.lua`.
8. Validates both Studio plugin files.
9. Falls back to a temporary source build when necessary.

Restart Roblox Studio after Roldex is installed or updated so Studio reloads both plugins.

## Cloud AI mode

Cloud mode is the compatibility default.

For OpenRouter in the current PowerShell session:

```powershell
$env:OPENROUTER_API_KEY="YOUR_OPENROUTER_KEY"
roldex
```

To save a key for later PowerShell windows:

```powershell
[Environment]::SetEnvironmentVariable("OPENROUTER_API_KEY", "YOUR_OPENROUTER_KEY", "User")
```

Do not share API keys publicly. Cloud provider quotas, rate limits, and charges can still apply.

## Local AI mode — no OpenRouter key

Roldex can connect to a local OpenAI-compatible server. The intended runtime is `llama.cpp`/`llama-server`.

This removes OpenRouter request/day limits from text/coding inference performed locally. It does **not** remove hardware limits: the model still needs enough RAM/VRAM/CPU/GPU resources.

Large model files are intentionally not bundled in GitHub or in the normal Roldex installer.

### Install only the local runtime

You can prepare `llama.cpp` before choosing/downloading a model:

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\install-local-runtime.ps1
```

The runtime can also live on another drive:

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\install-local-runtime.ps1 `
  -InstallDir "E:\RoldexAI\llama.cpp" `
  -Backend cpu
```

Supported backends are `cpu`, `vulkan`, `cuda12`, and `cuda13`. The installer dynamically discovers a current Windows `llama.cpp` runtime from its GitHub releases, verifies the GitHub-provided SHA256 digest when available, handles the matching CUDA companion runtime for CUDA builds, and sets `ROLDEX_LLAMA_SERVER`. It **does not download any model weights**.

### If a local server is already running

Default endpoint:

```text
http://127.0.0.1:8080/v1/chat/completions
```

Launch:

```powershell
roldex --local-ai
```

Custom server/model:

```powershell
roldex --local-ai `
  --local-endpoint "http://127.0.0.1:9000/v1/chat/completions" `
  --local-model "roldex-local"
```

### Start llama.cpp from any drive

When you have a `llama-server.exe` and a GGUF model, the repository includes `scripts/start-local-ai.ps1`.

Example using an external drive:

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\start-local-ai.ps1 `
  -LlamaServerPath "E:\RoldexAI\llama.cpp\llama-server.exe" `
  -ModelPath "E:\RoldexAI\models\coding-model.gguf" `
  -LaunchRoldex `
  -FullAccess
```

The drive letter can be changed freely. The script validates the paths, safely handles paths containing spaces, starts a loopback-only model server, enables the llama.cpp Jinja/tool-call path, waits for `/health`, configures Roldex local mode, and optionally launches Roldex.

### Validate the chosen model

Before trusting a model with Roldex tools/Studio:

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\test-local-ai.ps1
```

This checks server health, normal chat, response timing, and an actual OpenAI-style function/tool call. A model that can chat but cannot reliably call tools is not suitable as Roldex's main agent model.

Full details: **[docs/LOCAL_AI.md](docs/LOCAL_AI.md)**.

## Example local config

Copy `roldex.local.example.toml` to `roldex.toml` in a project if you prefer file-based configuration:

```powershell
Copy-Item .\roldex.local.example.toml .\roldex.toml
```

`roldex.toml` is ignored by Git so machine-specific paths/settings are not accidentally committed.

## Environment overrides

Roldex recognizes:

```text
ROLDEX_AI_MODE
ROLDEX_AI_PROVIDER
ROLDEX_AI_ENDPOINT
ROLDEX_MODEL
ROLDEX_API_KEY_ENV
ROLDEX_AI_TIMEOUT_SECONDS
ROLDEX_OPENROUTER_SORT
ROLDEX_LLAMA_SERVER
```

For local mode:

```powershell
$env:ROLDEX_AI_MODE="local"
$env:ROLDEX_AI_ENDPOINT="http://127.0.0.1:8080/v1/chat/completions"
$env:ROLDEX_MODEL="roldex-local"
roldex
```

## Run Roldex

From a Roblox/Rojo project folder:

```powershell
roldex
```

For tasks that genuinely require access outside the project directory:

```powershell
roldex --full-access
```

For local inference:

```powershell
roldex --local-ai
```

Keep Roldex running while Roblox Studio is open. The Studio bridge uses:

```text
http://127.0.0.1:38247
```

## Repair the Studio plugins

Close Roblox Studio, then run:

```powershell
roldex --repair-studio-plugin
```

Restart Studio afterward.

Roldex also synchronizes the bundled plugin copies automatically on Windows startup when they are missing or stale.

## Check installation

Verify the executable:

```powershell
roldex --version
```

Then start Roldex and run:

```text
/doctor
```

`/doctor` checks the project, AI/provider setup, Git availability, Studio bridge, and Studio plugin installation. In local mode it shows the configured endpoint/model and `AI key: not required`.

## No Git clone / low-disk fallback

When a prebuilt executable is unavailable, the source fallback can use a temporary repository ZIP/build directory and delete it afterward. The source checkout/build cache does not need to remain on the PC.

## Troubleshooting

### `roldex` is not recognized

Try:

```powershell
& "$env:LOCALAPPDATA\Roldex\bin\roldex.exe" --version
```

If that works, close and reopen PowerShell so Windows reloads the user PATH.

### Local AI says it cannot reach the server

The local server must be running before Roldex can generate a response. Check:

```powershell
irm http://127.0.0.1:8080/health
```

If that fails, check the `llama-server` window for model, RAM/VRAM, or startup errors.

### Studio plugin does not appear/connect

1. Close Roblox Studio completely.
2. Run `roldex --repair-studio-plugin`.
3. Reopen Studio.
4. Start Roldex in the project folder.
5. Allow Studio localhost access if prompted.

Installed plugin files on Windows:

```text
%LOCALAPPDATA%\Roblox\Plugins\RoldexStudio.plugin.lua
%LOCALAPPDATA%\Roblox\Plugins\RoldexStudioRuntime.plugin.lua
```

### Cargo/source build errors

A verified prebuilt Roldex release avoids needing Rust/MSVC. If a source fallback is required, install Rust/Cargo and Microsoft C++ Build Tools/Desktop development with C++ as needed.

## Studio permissions

The Roldex Studio plugins communicate with the local Roldex bridge. Roblox Studio may ask for permission to access:

```text
http://127.0.0.1:38247
```

Allow localhost access.

Visual testing can also request Studio screenshot permission. If Studio was open while plugin files were replaced, restart Studio before testing the new plugin code.
