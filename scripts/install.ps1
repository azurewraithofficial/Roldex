param(
    [switch]$SkipPlugin,
    [switch]$ForceCargo,
    [string]$Ref = ""
)

$ErrorActionPreference = "Stop"
$ProgressPreference = "SilentlyContinue"
$Repo = "azurewraithofficial/Roldex"
$DefaultRef = "main"

if (-not $Ref) {
    if ($env:ROLDEX_REF) {
        $Ref = $env:ROLDEX_REF
    }
    else {
        $Ref = $DefaultRef
    }
}

$InstallDir = Join-Path $env:LOCALAPPDATA "Roldex\bin"
$BinaryPath = Join-Path $InstallDir "roldex.exe"
$PluginDir = Join-Path $env:LOCALAPPDATA "Roblox\Plugins"
$MainPluginPath = Join-Path $PluginDir "RoldexStudio.plugin.lua"
$RuntimePluginPath = Join-Path $PluginDir "RoldexStudioRuntime.plugin.lua"
$MainPluginUrl = "https://raw.githubusercontent.com/$Repo/$Ref/plugins/roldex-studio/RoldexStudio.plugin.lua"
$RuntimePluginUrl = "https://raw.githubusercontent.com/$Repo/$Ref/plugins/roldex-studio/RoldexStudioRuntime.plugin.lua"

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

function Find-Cargo {
    $cargo = Get-Command cargo -ErrorAction SilentlyContinue
    if ($cargo) {
        return $cargo.Source
    }

    $standardCargo = Join-Path $HOME ".cargo\bin\cargo.exe"
    if (Test-Path $standardCargo) {
        $cargoBin = Split-Path -Parent $standardCargo
        if (($env:Path.Split(';') | Where-Object { $_ }) -notcontains $cargoBin) {
            $env:Path = "$cargoBin;$env:Path"
        }
        return $standardCargo
    }

    return $null
}

function Install-OneStudioPlugin(
    [string]$Url,
    [string]$Path,
    [string[]]$RequiredMarkers
) {
    $temporary = "$Path.download"
    Remove-Item $temporary -Force -ErrorAction SilentlyContinue
    Get-RemoteFile $Url $temporary

    $pluginText = Get-Content -Raw -Path $temporary
    foreach ($marker in $RequiredMarkers) {
        if ($pluginText -notmatch [regex]::Escape($marker)) {
            Remove-Item $temporary -Force -ErrorAction SilentlyContinue
            throw "Downloaded Studio plugin $([IO.Path]::GetFileName($Path)) failed validation: missing $marker"
        }
    }

    Move-Item -Force $temporary $Path
    Write-Host "Installed Studio plugin: $Path"
}

function Install-StudioPlugins {
    New-Item -ItemType Directory -Force -Path $PluginDir | Out-Null
    Install-OneStudioPlugin `
        $MainPluginUrl `
        $MainPluginPath `
        @("Roldex Studio", "X-Roldex-Bridge", "/v1/actions", "ChangeHistoryService")
    Install-OneStudioPlugin `
        $RuntimePluginUrl `
        $RuntimePluginPath `
        @("Roldex Studio Runtime", "X-Roldex-Bridge", "/v1/runtime-actions", "StudioCaptureService", "CreateVirtualInput")
    Write-Host "Restart Roblox Studio if it is currently open so both Roldex plugins reload."
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

    Write-Host "Trying latest verified Roldex Windows $architecture release..."
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

function Install-SourceArchive([string]$CargoPath) {
    $tempRoot = Join-Path $env:TEMP ("roldex-install-" + [guid]::NewGuid().ToString("N"))
    $archivePath = Join-Path $tempRoot "source.zip"
    $extractPath = Join-Path $tempRoot "source"
    $tempCargoHome = Join-Path $tempRoot "cargo-home"
    $oldCargoHome = $env:CARGO_HOME

    New-Item -ItemType Directory -Force -Path $tempRoot | Out-Null
    New-Item -ItemType Directory -Force -Path $extractPath | Out-Null
    New-Item -ItemType Directory -Force -Path $tempCargoHome | Out-Null

    try {
        Write-Host "No verified release binary is available. Using temporary source-build fallback from ref '$Ref'."
        Write-Host "No repository will be cloned or kept on this PC."
        Get-RemoteFile "https://github.com/$Repo/archive/refs/heads/$Ref.zip" $archivePath
        Expand-Archive -Path $archivePath -DestinationPath $extractPath -Force

        $repoRoot = Get-ChildItem -Path $extractPath -Directory | Select-Object -First 1
        if (-not $repoRoot) {
            throw "Downloaded Roldex source archive did not contain a project directory."
        }

        $env:CARGO_HOME = $tempCargoHome
        Push-Location $repoRoot.FullName
        try {
            & $CargoPath build --release -p roldex-cli
            if ($LASTEXITCODE -ne 0) {
                throw "cargo build failed with exit code $LASTEXITCODE"
            }
        }
        finally {
            Pop-Location
        }

        $builtBinary = Join-Path $repoRoot.FullName "target\release\roldex.exe"
        if (-not (Test-Path $builtBinary)) {
            throw "Cargo build completed but target\release\roldex.exe was not found."
        }

        New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
        Copy-Item -Force $builtBinary $BinaryPath
        Unblock-File -Path $BinaryPath -ErrorAction SilentlyContinue

        $versionOutput = & $BinaryPath --version 2>&1
        if ($LASTEXITCODE -ne 0 -or "$versionOutput" -notmatch "roldex") {
            Remove-Item $BinaryPath -Force -ErrorAction SilentlyContinue
            throw "Source-built Roldex binary failed startup validation."
        }

        Add-UserPath $InstallDir
        Write-Host "Installed Roldex CLI from temporary source build: $BinaryPath"
        Write-Host "Verified: $versionOutput"
    }
    finally {
        if ($null -eq $oldCargoHome) {
            Remove-Item Env:CARGO_HOME -ErrorAction SilentlyContinue
        }
        else {
            $env:CARGO_HOME = $oldCargoHome
        }
        Remove-Item -Recurse -Force $tempRoot -ErrorAction SilentlyContinue
    }
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
    }
}

if (-not $installedBinary) {
    $cargoPath = Find-Cargo
    if (-not $cargoPath) {
        throw @"
No verified prebuilt Roldex release is currently available, and Cargo was not found.

You do NOT need to clone the Roldex repository.

Fallback options:
1. If you already installed Rust before, close/reopen PowerShell and run this installer again.
2. Check whether Cargo exists at: $HOME\.cargo\bin\cargo.exe
3. If Rust is not installed, install Rust from https://rustup.rs/ and then rerun the SAME one-line Roldex installer.
4. If a Rust source build reports a linker/MSVC error, install Microsoft C++ Build Tools, then rerun the SAME installer.

Once Roldex has a published Windows release, none of the Rust/Cargo build fallback is required; the installer will download only the verified roldex.exe plus the two Studio plugin files.
"@
    }

    Install-SourceArchive $cargoPath
    $installedBinary = $true
}

if (-not $SkipPlugin) {
    Install-StudioPlugins
}

Write-Host ""
Write-Host "Roldex installation complete."
Write-Host "Source ref: $Ref"
Write-Host "No Roldex repository clone was kept on this PC."
Write-Host "Set OPENROUTER_API_KEY, open your Roblox/Rojo project folder, then run: roldex"
Write-Host "For tasks that truly require files outside the project, run: roldex --full-access"
Write-Host "Optional media generation uses POLLINATIONS_API_KEY."
Write-Host "Run /doctor inside Roldex to check your setup."
Write-Host "Both Studio plugins connect only to http://127.0.0.1:38247 by default."
