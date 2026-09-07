param(
    [switch]$SkipPlugin,
    [switch]$ForceCargo
)

$ErrorActionPreference = "Stop"
$ProgressPreference = "SilentlyContinue"
$Repo = "azurewraithofficial/Roldex"
$InstallDir = Join-Path $env:LOCALAPPDATA "Roldex\bin"
$BinaryPath = Join-Path $InstallDir "roldex.exe"
$PluginDir = Join-Path $env:LOCALAPPDATA "Roblox\Plugins"
$PluginPath = Join-Path $PluginDir "RoldexStudio.plugin.lua"
$PluginUrl = "https://raw.githubusercontent.com/$Repo/main/plugins/roldex-studio/RoldexStudio.plugin.lua"

function Get-RoldexArchitecture {
    $arch = $env:PROCESSOR_ARCHITEW6432
    if (-not $arch) {
        $arch = $env:PROCESSOR_ARCHITECTURE
    }
    if ($arch -match "ARM64|AARCH64") {
        return "arm64"
    }
    return "x86_64"
}

function Add-UserPath([string]$Directory) {
    $current = [Environment]::GetEnvironmentVariable("Path", "User")
    $parts = @()
    if ($current) {
        $parts = $current.Split(';') | Where-Object { $_ }
    }
    if ($parts -notcontains $Directory) {
        $newPath = (($parts + $Directory) -join ';')
        [Environment]::SetEnvironmentVariable("Path", $newPath, "User")
        Write-Host "Added $Directory to your user PATH."
    }

    $sessionParts = $env:Path.Split(';') | Where-Object { $_ }
    if ($sessionParts -notcontains $Directory) {
        $env:Path = "$Directory;$env:Path"
    }
}

function Get-RemoteFile([string]$Uri, [string]$OutFile) {
    Invoke-WebRequest `
        -UseBasicParsing `
        -Uri $Uri `
        -OutFile $OutFile `
        -Headers @{ "User-Agent" = "Roldex-Installer" }
}

function Install-StudioPlugin {
    New-Item -ItemType Directory -Force -Path $PluginDir | Out-Null
    $temporary = "$PluginPath.download"
    Remove-Item $temporary -Force -ErrorAction SilentlyContinue
    Get-RemoteFile $PluginUrl $temporary

    $pluginText = Get-Content -Raw -Path $temporary
    if ($pluginText -notmatch "Roldex Studio" -or $pluginText -notmatch "X-Roldex-Bridge") {
        Remove-Item $temporary -Force -ErrorAction SilentlyContinue
        throw "Downloaded Studio plugin did not pass the Roldex validation check."
    }

    Move-Item -Force $temporary $PluginPath
    Write-Host "Installed Roldex Studio plugin: $PluginPath"
    Write-Host "Restart Roblox Studio if it is currently open."
}

function Install-ReleaseBinary {
    $architecture = Get-RoldexArchitecture
    $asset = if ($architecture -eq "arm64") {
        "roldex-windows-arm64.exe"
    }
    else {
        "roldex-windows-x86_64.exe"
    }

    $releaseUrl = "https://github.com/$Repo/releases/latest/download/$asset"
    $checksumUrl = "$releaseUrl.sha256"
    $temporaryBinary = Join-Path $InstallDir "$asset.download"
    $temporaryChecksum = Join-Path $InstallDir "$asset.sha256.download"

    New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
    Remove-Item $temporaryBinary, $temporaryChecksum -Force -ErrorAction SilentlyContinue

    Write-Host "Trying latest Roldex Windows $architecture release..."
    Get-RemoteFile $releaseUrl $temporaryBinary
    Get-RemoteFile $checksumUrl $temporaryChecksum

    $expected = (Get-Content -Raw $temporaryChecksum).Trim().Split()[0].ToLowerInvariant()
    if ($expected -notmatch '^[a-f0-9]{64}$') {
        throw "Release checksum file is invalid."
    }
    $actual = (Get-FileHash -Algorithm SHA256 -Path $temporaryBinary).Hash.ToLowerInvariant()
    if ($actual -ne $expected) {
        throw "Roldex release checksum verification failed."
    }

    Move-Item -Force $temporaryBinary $BinaryPath
    Remove-Item $temporaryChecksum -Force -ErrorAction SilentlyContinue
    Unblock-File -Path $BinaryPath -ErrorAction SilentlyContinue

    $versionOutput = & $BinaryPath --version 2>&1
    if ($LASTEXITCODE -ne 0 -or "$versionOutput" -notmatch "roldex") {
        Remove-Item $BinaryPath -Force -ErrorAction SilentlyContinue
        throw "Downloaded Roldex binary failed its startup validation."
    }

    Add-UserPath $InstallDir
    Write-Host "Installed Roldex CLI: $BinaryPath"
    Write-Host "Verified: $versionOutput"
}

$installedBinary = $false

if (-not $ForceCargo) {
    try {
        Install-ReleaseBinary
        $installedBinary = $true
    }
    catch {
        Remove-Item $BinaryPath -Force -ErrorAction SilentlyContinue
        Write-Warning "Release install was unavailable or failed validation: $($_.Exception.Message)"
        Write-Warning "Falling back to Cargo when available."
    }
}

if (-not $installedBinary) {
    if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
        throw "Roldex needs either a published verified release binary or Rust/Cargo for the source install. Install Rust once from rustup, then run this installer again."
    }

    Write-Host "Installing Roldex CLI from source with Cargo..."
    cargo install --git "https://github.com/$Repo" roldex-cli --force
    if ($LASTEXITCODE -ne 0) {
        throw "cargo install failed with exit code $LASTEXITCODE"
    }

    $cargoHome = if ($env:CARGO_HOME) { $env:CARGO_HOME } else { Join-Path $HOME ".cargo" }
    $cargoBin = Join-Path $cargoHome "bin"
    Add-UserPath $cargoBin

    $command = Get-Command roldex -ErrorAction SilentlyContinue
    if (-not $command) {
        throw "Cargo reported success but roldex.exe was not found on PATH."
    }
    & $command.Source --version
    if ($LASTEXITCODE -ne 0) {
        throw "Cargo-installed Roldex failed its startup validation."
    }
    Write-Host "Installed Roldex through Cargo."
}

if (-not $SkipPlugin) {
    Install-StudioPlugin
}

Write-Host ""
Write-Host "Roldex installation complete."
Write-Host "Set OPENROUTER_API_KEY, open your Roblox/Rojo project folder, then run: roldex"
Write-Host "Optional media generation uses POLLINATIONS_API_KEY."
Write-Host "Run /doctor inside Roldex to check your setup."
Write-Host "The Studio plugin connects to http://127.0.0.1:38247 by default."
