[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$ModelPath,

    [string]$LlamaServerPath = "",

    [ValidateRange(1024, 1048576)]
    [int]$ContextSize = 32768,

    [ValidateRange(1024, 65535)]
    [int]$Port = 8080,

    [string]$GpuLayers = "auto",

    [string]$ModelAlias = "roldex-local",

    [switch]$LaunchRoldex,

    [switch]$FullAccess
)

$ErrorActionPreference = "Stop"

function Resolve-LlamaServer {
    param([string]$RequestedPath)

    $candidates = @()
    if ($RequestedPath) {
        $candidates += $RequestedPath
    }
    if ($env:ROLDEX_LLAMA_SERVER) {
        $candidates += $env:ROLDEX_LLAMA_SERVER
    }

    $command = Get-Command "llama-server.exe" -ErrorAction SilentlyContinue
    if ($command) {
        $candidates += $command.Source
    }

    $commandNoExtension = Get-Command "llama-server" -ErrorAction SilentlyContinue
    if ($commandNoExtension) {
        $candidates += $commandNoExtension.Source
    }

    $candidates += (Join-Path $env:LOCALAPPDATA "Roldex\llama.cpp\llama-server.exe")
    $candidates += (Join-Path $PSScriptRoot "llama-server.exe")

    foreach ($candidate in $candidates) {
        if (-not $candidate) {
            continue
        }
        try {
            $resolved = (Resolve-Path -LiteralPath $candidate -ErrorAction Stop).Path
            if (Test-Path -LiteralPath $resolved -PathType Leaf) {
                return $resolved
            }
        } catch {
        }
    }

    throw @"
Could not find llama-server.exe.

Later, install llama.cpp with scripts/install-local-runtime.ps1, or either:
  1. pass -LlamaServerPath "X:\path\to\llama-server.exe", or
  2. set ROLDEX_LLAMA_SERVER to that executable path.

The large model file can live on any drive, including an external USB drive.
"@
}

function Quote-NativeArgument {
    param([string]$Value)
    if ($Value.Contains('"')) {
        throw "Native launcher arguments cannot contain a double-quote character: $Value"
    }
    return '"' + $Value + '"'
}

$model = (Resolve-Path -LiteralPath $ModelPath -ErrorAction Stop).Path
if (-not (Test-Path -LiteralPath $model -PathType Leaf)) {
    throw "Model file not found: $ModelPath"
}
if ([IO.Path]::GetExtension($model) -ne ".gguf") {
    Write-Warning "The selected model does not end in .gguf. llama.cpp normally expects a GGUF model file."
}
if ([string]::IsNullOrWhiteSpace($ModelAlias)) {
    throw "ModelAlias cannot be empty."
}

$server = Resolve-LlamaServer -RequestedPath $LlamaServerPath
$healthUrl = "http://127.0.0.1:$Port/health"
$chatUrl = "http://127.0.0.1:$Port/v1/chat/completions"

$alreadyRunning = $false
try {
    $health = Invoke-RestMethod -Uri $healthUrl -Method Get -TimeoutSec 2
    if ($health) {
        $alreadyRunning = $true
    }
} catch {
}

if (-not $alreadyRunning) {
    $argumentLine = @(
        "--model", (Quote-NativeArgument $model),
        "--host", "127.0.0.1",
        "--port", "$Port",
        "--ctx-size", "$ContextSize",
        "--alias", (Quote-NativeArgument $ModelAlias),
        "--jinja",
        "--n-gpu-layers", "$GpuLayers"
    ) -join " "

    Write-Host "Starting local Roldex AI..."
    Write-Host "Model: $model"
    Write-Host "Server: $server"
    Write-Host "Endpoint: $chatUrl"

    Start-Process -FilePath $server -ArgumentList $argumentLine -WindowStyle Normal | Out-Null

    $ready = $false
    for ($attempt = 0; $attempt -lt 120; $attempt++) {
        Start-Sleep -Milliseconds 500
        try {
            $health = Invoke-RestMethod -Uri $healthUrl -Method Get -TimeoutSec 2
            if ($health) {
                $ready = $true
                break
            }
        } catch {
        }
    }

    if (-not $ready) {
        throw "llama-server did not become healthy at $healthUrl. Check the llama-server window for a model/RAM/GPU error."
    }
} else {
    Write-Host "A local AI server is already responding at $healthUrl"
}

$env:ROLDEX_AI_MODE = "local"
$env:ROLDEX_AI_ENDPOINT = $chatUrl
$env:ROLDEX_MODEL = $ModelAlias
$env:ROLDEX_API_KEY_ENV = ""

Write-Host ""
Write-Host "Local Roldex AI is ready." -ForegroundColor Green
Write-Host "No OpenRouter API key is required for this local session."
Write-Host ""
Write-Host "To launch manually in this PowerShell window:"
Write-Host "  roldex --local-ai --local-endpoint `"$chatUrl`" --local-model `"$ModelAlias`""

if ($LaunchRoldex) {
    $roldexArgs = @(
        "--local-ai",
        "--local-endpoint", $chatUrl,
        "--local-model", $ModelAlias
    )
    if ($FullAccess) {
        $roldexArgs += "--full-access"
    }
    & roldex @roldexArgs
}
