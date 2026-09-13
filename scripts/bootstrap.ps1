param(
    [string]$Ref = ""
)

$ErrorActionPreference = "Stop"
$ProgressPreference = "SilentlyContinue"
$Repo = "azurewraithofficial/Roldex"

if (-not $Ref) {
    if ($env:ROLDEX_REF) { $Ref = $env:ROLDEX_REF } else { $Ref = "main" }
}

function Download-File([string]$Url, [string]$OutFile) {
    Invoke-WebRequest -UseBasicParsing -Uri $Url -OutFile $OutFile -Headers @{ "User-Agent" = "Roldex-Bootstrap" }
}

function Invoke-OneLineInstaller {
    $tempInstaller = Join-Path $env:TEMP ("roldex-one-line-" + [guid]::NewGuid().ToString("N") + ".ps1")
    try {
        Download-File "https://raw.githubusercontent.com/$Repo/$Ref/scripts/install-one-line.ps1" $tempInstaller
        & $tempInstaller -Ref $Ref
        if ($LASTEXITCODE -ne 0) { throw "Roldex installer exited with code $LASTEXITCODE" }
    }
    finally {
        Remove-Item $tempInstaller -Force -ErrorAction SilentlyContinue
    }
}

function Find-Cargo {
    $cargo = Get-Command cargo -ErrorAction SilentlyContinue
    if ($cargo) { return $cargo.Source }
    $standard = Join-Path $HOME ".cargo\bin\cargo.exe"
    if (Test-Path $standard) {
        $bin = Split-Path -Parent $standard
        if (($env:Path.Split(';') | Where-Object { $_ }) -notcontains $bin) {
            $env:Path = "$bin;$env:Path"
        }
        return $standard
    }
    return $null
}

function Install-RustAutomatically {
    if (Find-Cargo) { return }

    $arch = $env:PROCESSOR_ARCHITEW6432
    if (-not $arch) { $arch = $env:PROCESSOR_ARCHITECTURE }
    $rustupUrl = if ($arch -match "ARM64|AARCH64") {
        "https://win.rustup.rs/aarch64"
    }
    else {
        "https://win.rustup.rs/x86_64"
    }

    $rustup = Join-Path $env:TEMP ("rustup-init-" + [guid]::NewGuid().ToString("N") + ".exe")
    try {
        Write-Host "Roldex needs a temporary source-build fallback; Cargo is missing. Installing Rust automatically..."
        Download-File $rustupUrl $rustup
        Unblock-File $rustup -ErrorAction SilentlyContinue
        & $rustup -y --profile minimal --default-toolchain stable
        if ($LASTEXITCODE -ne 0) { throw "rustup installation failed with exit code $LASTEXITCODE" }
    }
    finally {
        Remove-Item $rustup -Force -ErrorAction SilentlyContinue
    }

    $cargoBin = Join-Path $HOME ".cargo\bin"
    if (($env:Path.Split(';') | Where-Object { $_ }) -notcontains $cargoBin) {
        $env:Path = "$cargoBin;$env:Path"
    }
    if (-not (Find-Cargo)) {
        throw "Rust installed, but cargo.exe still could not be found. Open a new PowerShell and rerun the same one-line installer."
    }
}

function Install-MsvcBuildToolsAutomatically {
    $winget = Get-Command winget -ErrorAction SilentlyContinue
    if (-not $winget) {
        throw "The Microsoft C++ linker is required for the source-build fallback, but WinGet is not available. Update/install Windows App Installer, then rerun the same Roldex one-liner."
    }

    Write-Host "Installing Microsoft C++ Build Tools automatically for the temporary Rust build..."
    Write-Host "Windows may show an administrator/UAC prompt."
    & $winget.Source install --id Microsoft.VisualStudio.2022.BuildTools -e --source winget --accept-package-agreements --accept-source-agreements --override "--wait --passive --norestart --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
    if ($LASTEXITCODE -ne 0) {
        throw "Microsoft C++ Build Tools installation failed with exit code $LASTEXITCODE"
    }
}

try {
    Invoke-OneLineInstaller
    exit 0
}
catch {
    $firstError = $_.Exception.Message
    Write-Warning "Direct Roldex install did not complete: $firstError"
}

Install-RustAutomatically

try {
    Invoke-OneLineInstaller
    exit 0
}
catch {
    $secondError = $_.Exception.Message
    Write-Warning "Rust is available, but the source build still failed: $secondError"
}

Install-MsvcBuildToolsAutomatically
Invoke-OneLineInstaller
