param(
    [switch]$SkipPlugin,
    [switch]$ForceCargo
)

$ErrorActionPreference = "Stop"
$Repo = "azurewraithofficial/Roldex"
$InstallDir = Join-Path $env:LOCALAPPDATA "Roldex\bin"
$BinaryPath = Join-Path $InstallDir "roldex.exe"
$PluginDir = Join-Path $env:LOCALAPPDATA "Roblox\Plugins"
$PluginPath = Join-Path $PluginDir "RoldexStudio.plugin.lua"
$PluginUrl = "https://raw.githubusercontent.com/$Repo/main/plugins/roldex-studio/RoldexStudio.plugin.lua"
$ReleaseUrl = "https://github.com/$Repo/releases/latest/download/roldex-windows-x86_64.exe"

function Add-UserPath([string]$Directory) {
    $current = [Environment]::GetEnvironmentVariable("Path", "User")
    $parts = @()
    if ($current) {
        $parts = $current.Split(';') | Where-Object { $_ }
    }
    if ($parts -notcontains $Directory) {
        $newPath = (($parts + $Directory) -join ';')
        [Environment]::SetEnvironmentVariable("Path", $newPath, "User")
        Write-Host "Added $Directory to your user PATH. Open a new terminal after installation."
    }
}

function Install-StudioPlugin {
    New-Item -ItemType Directory -Force -Path $PluginDir | Out-Null
    Invoke-WebRequest -UseBasicParsing -Uri $PluginUrl -OutFile $PluginPath -Headers @{ "User-Agent" = "Roldex-Installer" }
    Write-Host "Installed Roldex Studio plugin: $PluginPath"
    Write-Host "Restart Roblox Studio if it is currently open."
}

$installedBinary = $false

if (-not $ForceCargo) {
    try {
        New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
        Write-Host "Trying latest Roldex release binary..."
        Invoke-WebRequest -UseBasicParsing -Uri $ReleaseUrl -OutFile $BinaryPath -Headers @{ "User-Agent" = "Roldex-Installer" }
        Unblock-File -Path $BinaryPath -ErrorAction SilentlyContinue
        Add-UserPath $InstallDir
        $installedBinary = $true
        Write-Host "Installed Roldex CLI: $BinaryPath"
    }
    catch {
        Remove-Item $BinaryPath -Force -ErrorAction SilentlyContinue
        Write-Warning "No compatible release binary was available yet. Falling back to Cargo."
    }
}

if (-not $installedBinary) {
    if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
        throw "Roldex needs either a published release binary or Rust/Cargo for the source install. Install Rust once from rustup, then run this installer again."
    }

    Write-Host "Installing Roldex CLI from source with Cargo..."
    cargo install --git "https://github.com/$Repo" roldex-cli --force
    if ($LASTEXITCODE -ne 0) {
        throw "cargo install failed with exit code $LASTEXITCODE"
    }
    Write-Host "Installed Roldex through Cargo."
}

if (-not $SkipPlugin) {
    Install-StudioPlugin
}

Write-Host ""
Write-Host "Roldex installation complete."
Write-Host "Set OPENROUTER_API_KEY, open your Roblox/Rojo project folder, then run: roldex"
Write-Host "The Studio plugin connects to http://127.0.0.1:38247 by default."
