$ErrorActionPreference = "Stop"

function Test-RoldexFile {
    param([Parameter(Mandatory = $true)][string]$Path)
    try {
        return [System.IO.File]::Exists($Path)
    }
    catch {
        return $false
    }
}

function Find-RoldexExternalRoot {
    $saved = [Environment]::GetEnvironmentVariable("ROLDEX_LOCAL_ROOT", "User")
    if (-not [string]::IsNullOrWhiteSpace($saved)) {
        $saved = [IO.Path]::GetFullPath($saved)
        $savedServer = Join-Path $saved "llama.cpp\llama-server.exe"
        $savedModel = Join-Path $saved "models\roldex-model.gguf"
        if ((Test-RoldexFile $savedServer) -and (Test-RoldexFile $savedModel)) {
            return $saved
        }
    }

    foreach ($drive in Get-PSDrive -PSProvider FileSystem -ErrorAction SilentlyContinue) {
        try {
            $candidate = Join-Path $drive.Root "RoldexAI"
            $server = Join-Path $candidate "llama.cpp\llama-server.exe"
            $model = Join-Path $candidate "models\roldex-model.gguf"

            if ((Test-RoldexFile $server) -and (Test-RoldexFile $model)) {
                [Environment]::SetEnvironmentVariable("ROLDEX_LOCAL_ROOT", $candidate, "User")
                return $candidate
            }
        }
        catch {
            continue
        }
    }

    return $null
}

$root = Find-RoldexExternalRoot
if (-not $root) {
    throw @"
Roldex could not access the external AI runtime/model.

Expected saved location:
  $([Environment]::GetEnvironmentVariable("ROLDEX_LOCAL_ROOT", "User"))

Make sure the external drive is connected and that your Windows account can read the RoldexAI folder.
"@
}

$server = Join-Path $root "llama.cpp\llama-server.exe"
$model = Join-Path $root "models\roldex-model.gguf"
$temp = Join-Path $root "temp"
$startScript = Join-Path $env:LOCALAPPDATA "Roldex\bin\start-local-ai.ps1"

if (-not (Test-RoldexFile $server)) {
    throw "Roldex cannot read llama-server.exe at $server"
}
if (-not (Test-RoldexFile $model)) {
    throw "Roldex cannot read the local model at $model"
}
if (-not (Test-Path -LiteralPath $startScript -PathType Leaf)) {
    throw "Roldex local startup script is missing at $startScript. Rerun the low-memory setup."
}

try {
    New-Item -ItemType Directory -Force -Path $temp | Out-Null
}
catch {
    throw "Roldex cannot write to the external temp folder at $temp. Check drive permissions. $($_.Exception.Message)"
}

$env:ROLDEX_LOCAL_ROOT = $root
$env:TEMP = $temp
$env:TMP = $temp

Write-Host "Using external AI: $root" -ForegroundColor Green
Write-Host "Model: $model"
Write-Host "Context: 4096 | GPU layers: 0 | Full access: enabled"
Write-Host ""

& $startScript `
    -LlamaServerPath $server `
    -ModelPath $model `
    -ContextSize 4096 `
    -GpuLayers "0" `
    -ModelAlias "roldex-local-lowmem" `
    -LaunchRoldex `
    -FullAccess
