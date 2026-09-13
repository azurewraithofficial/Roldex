param(
    [string]$Ref = ""
)

$ErrorActionPreference = "Stop"
$ProgressPreference = "SilentlyContinue"
$Repo = "azurewraithofficial/Roldex"

if (-not $Ref) {
    if ($env:ROLDEX_REF) {
        $Ref = $env:ROLDEX_REF
    }
    else {
        $Ref = "main"
    }
}

$InstallDir = Join-Path $env:LOCALAPPDATA "Roldex\bin"
$BinaryPath = Join-Path $InstallDir "roldex.exe"
$PluginDir = Join-Path $env:LOCALAPPDATA "Roblox\Plugins"
$MainPluginPath = Join-Path $PluginDir "RoldexStudio.plugin.lua"
$RuntimePluginPath = Join-Path $PluginDir "RoldexStudioRuntime.plugin.lua"

function Add-UserPath([string]$Directory) {
    $current = [Environment]::GetEnvironmentVariable("Path", "User")
    $parts = @()
    if ($current) {
        $parts = $current.Split(';') | Where-Object { $_ }
    }
    if ($parts -notcontains $Directory) {
        [Environment]::SetEnvironmentVariable("Path", (($parts + $Directory) -join ';'), "User")
    }
    if (($env:Path.Split(';') | Where-Object { $_ }) -notcontains $Directory) {
        $env:Path = "$Directory;$env:Path"
    }
}

function Get-RemoteFile([string]$Uri, [string]$OutFile) {
    Invoke-WebRequest -UseBasicParsing -Uri $Uri -OutFile $OutFile -Headers @{ "User-Agent" = "Roldex-Installer" }
}

function Install-Plugin([string]$Url, [string]$Path, [string[]]$Markers) {
    $temp = "$Path.download"
    Remove-Item $temp -Force -ErrorAction SilentlyContinue
    Get-RemoteFile $Url $temp
    $text = Get-Content -Raw $temp
    foreach ($marker in $Markers) {
        if ($text -notmatch [regex]::Escape($marker)) {
            Remove-Item $temp -Force -ErrorAction SilentlyContinue
            throw "Downloaded plugin failed validation: $([IO.Path]::GetFileName($Path)) missing '$marker'"
        }
    }
    Move-Item -Force $temp $Path
}

$arch = $env:PROCESSOR_ARCHITEW6432
if (-not $arch) { $arch = $env:PROCESSOR_ARCHITECTURE }
if ($arch -match "ARM64|AARCH64") {
    Write-Warning "The development prebuilt is currently Windows x64 only. Falling back to the normal installer for ARM64."
    $fallback = Join-Path $env:TEMP "roldex-install-fallback.ps1"
    Get-RemoteFile "https://raw.githubusercontent.com/$Repo/$Ref/scripts/install.ps1" $fallback
    try { & $fallback -Ref $Ref } finally { Remove-Item $fallback -Force -ErrorAction SilentlyContinue }
    exit $LASTEXITCODE
}

New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
New-Item -ItemType Directory -Force -Path $PluginDir | Out-Null

$asset = "roldex-windows-x86_64.exe"
$base = "https://raw.githubusercontent.com/$Repo/$Ref/dist/dev"
$tempExe = Join-Path $env:TEMP "roldex-dev-download.exe"
$tempSha = Join-Path $env:TEMP "roldex-dev-download.exe.sha256"

try {
    Write-Host "Installing Roldex from verified GitHub-built Windows binary..."
    Get-RemoteFile "$base/$asset" $tempExe
    Get-RemoteFile "$base/$asset.sha256" $tempSha

    $expected = (Get-Content -Raw $tempSha).Trim().Split()[0].ToLowerInvariant()
    if ($expected -notmatch '^[a-f0-9]{64}$') {
        throw "Development binary checksum file is invalid."
    }
    $actual = (Get-FileHash -Algorithm SHA256 $tempExe).Hash.ToLowerInvariant()
    if ($actual -ne $expected) {
        throw "Development Roldex binary checksum verification failed."
    }

    Copy-Item -Force $tempExe $BinaryPath
    Unblock-File -Path $BinaryPath -ErrorAction SilentlyContinue
    $version = & $BinaryPath --version 2>&1
    if ($LASTEXITCODE -ne 0 -or "$version" -notmatch "roldex") {
        Remove-Item $BinaryPath -Force -ErrorAction SilentlyContinue
        throw "Downloaded Roldex binary failed startup validation."
    }

    Add-UserPath $InstallDir

    Install-Plugin `
        "https://raw.githubusercontent.com/$Repo/$Ref/plugins/roldex-studio/RoldexStudio.plugin.lua" `
        $MainPluginPath `
        @("Roldex Studio", "X-Roldex-Bridge", "/v1/actions", "ChangeHistoryService")

    Install-Plugin `
        "https://raw.githubusercontent.com/$Repo/$Ref/plugins/roldex-studio/RoldexStudioRuntime.plugin.lua" `
        $RuntimePluginPath `
        @("Roldex Studio Runtime", "X-Roldex-Bridge", "/v1/runtime-actions", "StudioCaptureService", "CreateVirtualInput")

    Write-Host ""
    Write-Host "Roldex installed successfully."
    Write-Host "Verified: $version"
    Write-Host "CLI: $BinaryPath"
    Write-Host "Studio plugins: $PluginDir"
    Write-Host "Restart Roblox Studio if it was open."
    Write-Host "Set OPENROUTER_API_KEY, then run: roldex"
}
catch {
    Write-Warning "GitHub-built development binary is not available yet or failed verification: $($_.Exception.Message)"
    Write-Host "Trying the normal no-clone installer fallback..."
    $fallback = Join-Path $env:TEMP "roldex-install-fallback.ps1"
    Get-RemoteFile "https://raw.githubusercontent.com/$Repo/$Ref/scripts/install.ps1" $fallback
    try {
        & $fallback -Ref $Ref
    }
    finally {
        Remove-Item $fallback -Force -ErrorAction SilentlyContinue
    }
}
finally {
    Remove-Item $tempExe, $tempSha -Force -ErrorAction SilentlyContinue
}
