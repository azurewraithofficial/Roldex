# Installing Roldex

Roldex is designed to be installed once, then launched from any Roblox/Rojo project folder.

## Windows — one-line install

For the current development build, open **PowerShell** and paste this **single line**:

```powershell
Set-ExecutionPolicy -Scope Process Bypass -Force; $env:ROLDEX_REF='roldex-intelligence-studio-vision'; irm 'https://raw.githubusercontent.com/azurewraithofficial/Roldex/roldex-intelligence-studio-vision/scripts/install.ps1' | iex
```

That line installs the CLI and both Roblox Studio plugins. It does **not** clone the Roldex repository to your PC.

Once this development build is merged/released, the normal stable one-line installer will be:

```powershell
Set-ExecutionPolicy -Scope Process Bypass -Force; irm 'https://raw.githubusercontent.com/azurewraithofficial/Roldex/main/scripts/install.ps1' | iex
```

### What the installer does

1. Detects Windows x64 vs ARM64.
2. Tries to download the newest matching prebuilt `roldex.exe` release.
3. Downloads its `.sha256` file and verifies the executable.
4. Runs `roldex --version` to make sure the binary starts.
5. Installs it under `%LOCALAPPDATA%\Roldex\bin`.
6. Adds Roldex to your user PATH and to the current PowerShell session.
7. Installs `RoldexStudio.plugin.lua`.
8. Installs `RoldexStudioRuntime.plugin.lua` for screenshots, device simulation, virtual input, chat tests, and automated playtests.
9. Validates both Studio plugin files.
10. If no verified prebuilt release exists, uses the source-build fallback described below.

Restart Roblox Studio after Roldex is installed or updated so Studio loads both plugin files.

## No Git clone / low-disk fallback

The source fallback does **not** run `git clone`.

If a prebuilt release is unavailable but Cargo already exists, the installer:

1. Downloads the selected Roldex branch as a temporary ZIP.
2. Extracts it under `%TEMP%`.
3. Uses a temporary Cargo dependency/cache folder.
4. Builds only the release CLI.
5. Copies only the finished `roldex.exe` into `%LOCALAPPDATA%\Roldex\bin`.
6. Deletes the downloaded ZIP, extracted source, build output, and temporary Cargo cache.

So the source repository is not permanently stored on your PC.

## Fallbacks / troubleshooting

### `cargo --version` gives a version

Good. You do not need to do anything. If no prebuilt release is available, the one-line installer can use Cargo automatically.

### `cargo --version` says `cargo is not recognized`

First check the normal Rust location:

```powershell
& "$HOME\.cargo\bin\cargo.exe" --version
```

If that prints a Cargo version, close PowerShell, open it again, and rerun the **same one-line Roldex installer**. The Roldex installer also checks this standard path automatically.

### `$HOME\.cargo\bin\cargo.exe` does not exist

Rust/Cargo is not installed.

If Roldex has a published prebuilt Windows release, that is fine: **Rust is not required at all**.

If no prebuilt release is available yet, install Rust once from the official Rust installer at `https://rustup.rs/`, then reopen PowerShell and rerun the same one-line Roldex installer.

You still do not need to clone Roldex.

### Cargo build fails with `link.exe`, MSVC, Visual Studio, or C++ Build Tools errors

Your Rust installation is present but the Windows native linker/build tools are missing.

Install **Microsoft C++ Build Tools / Desktop development with C++**, then rerun the exact same Roldex one-line installer.

This fallback is only needed when building Roldex from source. A published prebuilt Roldex release avoids Rust/MSVC completely.

### `irm` or `Invoke-RestMethod` is blocked

Use this one-line download-and-run fallback instead:

```powershell
$env:ROLDEX_REF='roldex-intelligence-studio-vision'; $p="$env:TEMP\roldex-install.ps1"; Invoke-WebRequest -UseBasicParsing 'https://raw.githubusercontent.com/azurewraithofficial/Roldex/roldex-intelligence-studio-vision/scripts/install.ps1' -OutFile $p; powershell -ExecutionPolicy Bypass -File $p; Remove-Item $p -Force -ErrorAction SilentlyContinue
```

### PowerShell says script execution is disabled

The recommended one-line installer already applies a bypass only to the current PowerShell process:

```powershell
Set-ExecutionPolicy -Scope Process Bypass -Force
```

It does not permanently weaken your machine-wide execution policy.

### `roldex` is not recognized after installation

Try:

```powershell
& "$env:LOCALAPPDATA\Roldex\bin\roldex.exe" --version
```

If that works, close and reopen PowerShell so Windows reloads your user PATH.

Then try:

```powershell
roldex --version
```

### Studio plugin does not appear

1. Close Roblox Studio completely.
2. Rerun the same one-line installer.
3. Reopen Studio.
4. Check that these files exist:

```text
%LOCALAPPDATA%\Roblox\Plugins\RoldexStudio.plugin.lua
%LOCALAPPDATA%\Roblox\Plugins\RoldexStudioRuntime.plugin.lua
```

Roldex can also attempt to repair its plugins later through its own repair tooling.

## Configure the AI key

Roldex currently defaults to OpenRouter.

For the current PowerShell session:

```powershell
$env:OPENROUTER_API_KEY="YOUR_OPENROUTER_KEY"
```

To save it for future PowerShell windows:

```powershell
[Environment]::SetEnvironmentVariable("OPENROUTER_API_KEY", "YOUR_OPENROUTER_KEY", "User")
```

Open a new PowerShell window after saving a user environment variable.

Do not share your API key publicly.

Optional image/voice generation uses `POLLINATIONS_API_KEY` when configured:

```powershell
[Environment]::SetEnvironmentVariable("POLLINATIONS_API_KEY", "YOUR_MEDIA_KEY", "User")
```

Provider quotas or charges can still apply. Roldex itself does not require a Roldex subscription. Live web originality/name research can also depend on the configured provider's web-search availability/credits.

## Run Roldex

Open PowerShell in the Roblox/Rojo project folder and run:

```powershell
roldex
```

For tasks that genuinely require files/tools outside the project directory:

```powershell
roldex --full-access
```

Keep Roldex running while Roblox Studio is open. The local Studio bridge uses:

```text
http://127.0.0.1:38247
```

## Check installation

First verify the executable:

```powershell
roldex --version
```

Then start Roldex and run:

```text
/doctor
```

`/doctor` checks things such as:

- Windows architecture
- project detection
- OpenRouter/media keys
- Git availability
- Studio bridge
- Studio plugin installation

## Reinstall or update

Just rerun the same **one-line installer**. It replaces the CLI and both Studio plugin files with the newest selected version.

## Studio permissions

Both Roldex Studio plugins communicate only with the local Roldex bridge. Roblox Studio may ask for permission to access:

```text
http://127.0.0.1:38247
```

Allow localhost access.

The visual-testing runtime can also request Studio screenshot permission the first time Roldex visually inspects the viewport. Allow it if you want screenshot/vision QA.

If Studio was already open during installation or repair, restart Studio so it loads the new plugin source.
