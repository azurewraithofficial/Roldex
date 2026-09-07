# Installing Roldex

Roldex is designed to be installed once, then launched from any Roblox/Rojo project folder.

## Windows — recommended

Open **PowerShell** and run:

```powershell
Set-ExecutionPolicy -Scope Process Bypass -Force
$installer = "$env:TEMP\install-roldex.ps1"
Invoke-WebRequest -UseBasicParsing "https://raw.githubusercontent.com/azurewraithofficial/Roldex/main/scripts/install.ps1" -OutFile $installer
& $installer
```

The installer will:

- detect Windows x64 vs ARM64
- prefer the latest prebuilt Roldex release
- verify the release SHA-256 checksum when available
- validate that `roldex.exe` starts
- add Roldex to your user PATH and current PowerShell session
- install `RoldexStudio.plugin.lua` into your local Roblox Studio Plugins directory
- install `RoldexStudioRuntime.plugin.lua` for visual captures, device simulation, virtual-input tests, and advanced playtest automation
- validate both downloaded Studio plugin files before accepting them
- fall back to `cargo install` if a compatible release is not available and Rust/Cargo is installed

Restart Roblox Studio after Roldex is installed or updated so Studio loads both plugin files.

## Configure the AI key

Roldex currently defaults to OpenRouter. In PowerShell, set the key for the current terminal:

```powershell
$env:OPENROUTER_API_KEY="YOUR_OPENROUTER_KEY"
```

To save it for future terminals:

```powershell
[Environment]::SetEnvironmentVariable("OPENROUTER_API_KEY", "YOUR_OPENROUTER_KEY", "User")
```

Open a **new PowerShell window** after saving a user environment variable.

Image/voice generation through the optional media provider uses `POLLINATIONS_API_KEY` when configured:

```powershell
[Environment]::SetEnvironmentVariable("POLLINATIONS_API_KEY", "YOUR_MEDIA_KEY", "User")
```

Provider quotas or charges can still apply. Roldex itself does not require a Roldex subscription. Live web originality/name research may also depend on the configured provider's web-search availability/credits.

## Run Roldex

Open PowerShell inside the Roblox/Rojo project folder and run:

```powershell
roldex
```

Roldex starts the localhost Studio bridge on `127.0.0.1:38247` by default. Keep the CLI open while using Roldex Studio.

When Studio is connected, Roldex can use the visible plugin to build/edit real Instances and the silent runtime plugin to capture the viewport, simulate supported input, switch device simulations, run playtests, and return evidence to the agent for verification/repair.

### Full computer access

If the task genuinely needs files or tools outside the project directory, launch:

```powershell
roldex --full-access
```

Full access means Roldex may read/write paths that the current Windows account itself can access and may run local development tools. Roldex is instructed to touch only files needed for the requested task, inspect before changing unfamiliar files, and verify/repair its changes before finishing.

## Check installation

Inside Roldex, run:

```text
/doctor
```

It reports the OS/architecture, project detection, AI/media keys, Git availability, Studio bridge state, and local Studio plugin state.

You can also verify the executable directly:

```powershell
roldex --version
```

## Reinstall or update

Run the same installer commands again. The installer replaces the CLI and both Studio plugin files with the newest available versions.

Force a source install with Cargo:

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\install.ps1 -ForceCargo
```

Skip both Studio plugin files if needed:

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\install.ps1 -SkipPlugin
```

## Install from source manually

If Rust is already installed:

```powershell
cargo install --git https://github.com/azurewraithofficial/Roldex roldex-cli --force
```

To build the repository instead:

```powershell
git clone https://github.com/azurewraithofficial/Roldex.git
cd Roldex
cargo build --release -p roldex-cli
```

The executable is then under:

```text
target\release\roldex.exe
```

## Studio permissions

Both Roldex Studio plugins communicate only with the local Roldex bridge. Roblox Studio may prompt the first time the plugins attempt localhost HTTP access. Allow localhost access so they can communicate with:

```text
http://127.0.0.1:38247
```

The visual-testing runtime may also request Studio screenshot permission the first time Roldex visually inspects the viewport. That permission is needed only for Studio viewport capture/vision QA.

If Studio is already open while Roldex is installed or repaired, restart Studio so it loads the new plugin source.
