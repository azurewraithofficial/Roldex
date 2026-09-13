[CmdletBinding()]
param(
    [string]$ExternalRoot = "E:\RoldexAI",
    [switch]$ForceModel
)

$ErrorActionPreference = "Stop"
$ProgressPreference = "SilentlyContinue"

$Repo = "azurewraithofficial/Roldex"
$Ref = "roldex-intelligence-studio-vision"
$ModelUrl = "https://huggingface.co/Qwen/Qwen2.5-Coder-0.5B-Instruct-GGUF/resolve/main/qwen2.5-coder-0.5b-instruct-q4_k_m.gguf?download=true"
$ModelFileName = "roldex-model.gguf"
$ContextSize = 4096
$GpuLayers = "0"
$ModelAlias = "roldex-local-lowmem"

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

$root = [IO.Path]::GetFullPath($ExternalRoot)
$runtimeDir = Join-Path $root "llama.cpp"
$serverPath = Join-Path $runtimeDir "llama-server.exe"
$modelDir = Join-Path $root "models"
$modelPath = Join-Path $modelDir $ModelFileName
$cacheDir = Join-Path $root "cache"
$tempDir = Join-Path $root "temp"
$markerPath = Join-Path $root ".roldex-external-ai"

New-Item -ItemType Directory -Force -Path $root, $modelDir, $cacheDir, $tempDir | Out-Null
Set-Content -LiteralPath $markerPath -Value "Roldex External AI" -Encoding ASCII

if (-not (Test-Path -LiteralPath $serverPath -PathType Leaf)) {
    Write-Warning "llama-server.exe is missing from $runtimeDir. Roldex will repair the CPU runtime automatically."
    $runtimeInstaller = Join-Path $env:TEMP "roldex-install-local-runtime.ps1"
    Invoke-WebRequest -UseBasicParsing `
        -Uri "https://raw.githubusercontent.com/$Repo/$Ref/scripts/install-local-runtime.ps1" `
        -OutFile $runtimeInstaller `
        -Headers @{ "User-Agent" = "Roldex-Low-Memory-Setup" }

    try {
        & powershell -NoProfile -ExecutionPolicy Bypass -File $runtimeInstaller `
            -InstallDir $runtimeDir `
            -Backend cpu `
            -Force
        if ($LASTEXITCODE -ne 0) {
            throw "The llama.cpp runtime repair process exited with code $LASTEXITCODE."
        }
    }
    finally {
        Remove-Item -LiteralPath $runtimeInstaller -Force -ErrorAction SilentlyContinue
    }
}

if (-not (Test-Path -LiteralPath $serverPath -PathType Leaf)) {
    throw @"
The local runtime repair completed, but llama-server.exe is still missing at:
  $serverPath

Check whether Windows Security quarantined llama-server.exe or whether the external drive is writable, then rerun this setup.
"@
}

if ($ForceModel -or -not (Test-Path -LiteralPath $modelPath -PathType Leaf)) {
    $downloadPath = "$modelPath.download"
    Remove-Item -LiteralPath $downloadPath -Force -ErrorAction SilentlyContinue
    Write-Host "Downloading low-memory coding model directly to the external drive..."
    Write-Host "Destination: $modelPath"
    Invoke-WebRequest -UseBasicParsing -Uri $ModelUrl -OutFile $downloadPath -Headers @{ "User-Agent" = "Roldex-Low-Memory-Setup" }

    $size = (Get-Item -LiteralPath $downloadPath).Length
    if ($size -lt 100MB) {
        Remove-Item -LiteralPath $downloadPath -Force -ErrorAction SilentlyContinue
        throw "Downloaded model file is unexpectedly small ($size bytes). The download may have failed."
    }

    Move-Item -LiteralPath $downloadPath -Destination $modelPath -Force
    Write-Host "Model downloaded successfully." -ForegroundColor Green
} else {
    Write-Host "Model already exists at $modelPath"
}

$bin = Join-Path $env:LOCALAPPDATA "Roldex\bin"
New-Item -ItemType Directory -Force -Path $bin | Out-Null
Add-UserPath $bin

$startScript = Join-Path $bin "start-local-ai.ps1"
$testScript = Join-Path $bin "test-local-ai.ps1"

Invoke-WebRequest -UseBasicParsing `
    -Uri "https://raw.githubusercontent.com/$Repo/$Ref/scripts/start-local-ai.ps1" `
    -OutFile $startScript `
    -Headers @{ "User-Agent" = "Roldex-Low-Memory-Setup" }

Invoke-WebRequest -UseBasicParsing `
    -Uri "https://raw.githubusercontent.com/$Repo/$Ref/scripts/test-local-ai.ps1" `
    -OutFile $testScript `
    -Headers @{ "User-Agent" = "Roldex-Low-Memory-Setup" }

$launcherPs1 = Join-Path $bin "roldex-local.ps1"
$launcherCmd = Join-Path $bin "roldex-local.cmd"

$launcher = @'
$ErrorActionPreference = "Stop"

$root = Get-PSDrive -PSProvider FileSystem | ForEach-Object {
    $candidate = Join-Path $_.Root "RoldexAI"
    if (Test-Path -LiteralPath (Join-Path $candidate ".roldex-external-ai")) {
        $candidate
    }
} | Select-Object -First 1

if (-not $root) {
    throw "Roldex external AI drive was not found. Plug in the drive containing the RoldexAI folder."
}

$server = Join-Path $root "llama.cpp\llama-server.exe"
$model = Join-Path $root "models\roldex-model.gguf"
$temp = Join-Path $root "temp"

if (-not (Test-Path -LiteralPath $server -PathType Leaf)) {
    throw "llama-server.exe was not found at $server. Rerun the low-memory setup to repair the runtime."
}
if (-not (Test-Path -LiteralPath $model -PathType Leaf)) {
    throw "Roldex local model was not found at $model. Rerun the low-memory setup to download it."
}

New-Item -ItemType Directory -Force -Path $temp | Out-Null
$env:TEMP = $temp
$env:TMP = $temp

& "$env:LOCALAPPDATA\Roldex\bin\start-local-ai.ps1" `
    -LlamaServerPath $server `
    -ModelPath $model `
    -ContextSize 4096 `
    -GpuLayers "0" `
    -ModelAlias "roldex-local-lowmem" `
    -LaunchRoldex `
    -FullAccess
'@

Set-Content -LiteralPath $launcherPs1 -Value $launcher -Encoding UTF8
Set-Content -LiteralPath $launcherCmd -Value '@powershell -NoProfile -ExecutionPolicy Bypass -File "%LOCALAPPDATA%\Roldex\bin\roldex-local.ps1" %*' -Encoding ASCII

[Environment]::SetEnvironmentVariable("ROLDEX_LOCAL_ROOT", $root, "User")
$env:ROLDEX_LOCAL_ROOT = $root

Write-Host ""
Write-Host "Roldex low-memory local AI setup is ready." -ForegroundColor Green
Write-Host "External AI root: $root"
Write-Host "Runtime: $serverPath"
Write-Host "Model: $modelPath"
Write-Host "Context: $ContextSize"
Write-Host "GPU layers: $GpuLayers (CPU-only to preserve shared RAM)"
Write-Host ""
Write-Host "The large model, cache, and temp files stay on the external drive."
Write-Host "Roldex itself and the Studio plugins remain on the normal Windows drive."
Write-Host ""
Write-Host "From any project folder, start with:"
Write-Host "  roldex-local"
