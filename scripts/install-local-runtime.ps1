[CmdletBinding()]
param(
    [string]$InstallDir = "$env:LOCALAPPDATA\Roldex\llama.cpp",

    [ValidateSet("cpu", "vulkan", "cuda12", "cuda13")]
    [string]$Backend = "cpu",

    [switch]$Force
)

$ErrorActionPreference = "Stop"
$ProgressPreference = "SilentlyContinue"

if (-not [Environment]::Is64BitOperatingSystem) {
    throw "Roldex local AI runtime setup currently requires 64-bit Windows."
}

$arch = [Runtime.InteropServices.RuntimeInformation]::OSArchitecture.ToString().ToLowerInvariant()
if ($arch -notin @("x64", "arm64")) {
    throw "Unsupported Windows architecture: $arch"
}
if ($arch -eq "arm64" -and $Backend -ne "cpu") {
    throw "For Windows ARM64, use -Backend cpu for now."
}

$target = [IO.Path]::GetFullPath($InstallDir)
$serverPath = Join-Path $target "llama-server.exe"
if ((Test-Path -LiteralPath $serverPath -PathType Leaf) -and -not $Force) {
    Write-Host "llama.cpp is already installed at $serverPath" -ForegroundColor Green
    Write-Host "Use -Force to refresh it."
    exit 0
}

New-Item -ItemType Directory -Path $target -Force | Out-Null

$headers = @{
    "User-Agent" = "Roldex-Local-AI-Installer"
    "Accept" = "application/vnd.github+json"
    "X-GitHub-Api-Version" = "2022-11-28"
}

Write-Host "Discovering the current llama.cpp Windows runtime..."
$stable = Invoke-RestMethod `
    -Uri "https://api.github.com/repos/ggml-org/llama.cpp/releases/latest" `
    -Headers $headers `
    -Method Get

$runtimeRelease = $stable
$nightlyTagAsset = $stable.assets | Where-Object { $_.name -eq "nightly-tag.txt" } | Select-Object -First 1
if ($nightlyTagAsset) {
    $tag = (Invoke-RestMethod -Uri $nightlyTagAsset.browser_download_url -Headers $headers -Method Get).ToString().Trim()
    if ($tag) {
        $escapedTag = [Uri]::EscapeDataString($tag)
        $runtimeRelease = Invoke-RestMethod `
            -Uri "https://api.github.com/repos/ggml-org/llama.cpp/releases/tags/$escapedTag" `
            -Headers $headers `
            -Method Get
    }
}

$assetPattern = switch ($Backend) {
    "cpu" {
        if ($arch -eq "arm64") {
            '^llama-.*-bin-win-cpu-arm64\.zip$'
        } else {
            '^llama-.*-bin-win-cpu-x64\.zip$'
        }
    }
    "vulkan" { '^llama-.*-bin-win-vulkan-x64\.zip$' }
    "cuda12" { '^llama-.*-bin-win-cuda-12\.[0-9]+-x64\.zip$' }
    "cuda13" { '^llama-.*-bin-win-cuda-13\.[0-9]+-x64\.zip$' }
}

$asset = $runtimeRelease.assets |
    Where-Object { $_.name -match $assetPattern } |
    Sort-Object name -Descending |
    Select-Object -First 1

if (-not $asset) {
    throw "Could not find a Windows $Backend/$arch llama.cpp asset in release $($runtimeRelease.tag_name)."
}

$tempRoot = Join-Path ([IO.Path]::GetTempPath()) ("roldex-llama-" + [Guid]::NewGuid().ToString("N"))
$zipPath = Join-Path $tempRoot $asset.name
$extractDir = Join-Path $tempRoot "extracted"
New-Item -ItemType Directory -Path $tempRoot -Force | Out-Null

try {
    Write-Host "Downloading $($asset.name)..."
    Invoke-WebRequest -Uri $asset.browser_download_url -Headers $headers -OutFile $zipPath

    $expectedDigest = [string]$asset.digest
    if ($expectedDigest -match '^sha256:([0-9a-fA-F]{64})$') {
        $expected = $Matches[1].ToLowerInvariant()
        $actual = (Get-FileHash -LiteralPath $zipPath -Algorithm SHA256).Hash.ToLowerInvariant()
        if ($actual -ne $expected) {
            throw "llama.cpp download checksum mismatch. Expected $expected but received $actual."
        }
        Write-Host "SHA256 verified."
    } else {
        Write-Warning "GitHub did not provide a SHA256 digest for this asset; checksum verification was skipped."
    }

    Expand-Archive -LiteralPath $zipPath -DestinationPath $extractDir -Force
    $foundServer = Get-ChildItem -LiteralPath $extractDir -Filter "llama-server.exe" -File -Recurse |
        Select-Object -First 1
    if (-not $foundServer) {
        throw "The downloaded llama.cpp archive did not contain llama-server.exe."
    }

    if (Test-Path -LiteralPath $target) {
        Get-ChildItem -LiteralPath $target -Force -ErrorAction SilentlyContinue |
            Remove-Item -Recurse -Force
    }

    $sourceDir = $foundServer.Directory.FullName
    Copy-Item -Path (Join-Path $sourceDir "*") -Destination $target -Recurse -Force

    if (-not (Test-Path -LiteralPath $serverPath -PathType Leaf)) {
        throw "Runtime extraction finished but $serverPath was not created."
    }

    $versionInfo = @(
        "source=ggml-org/llama.cpp",
        "release=$($runtimeRelease.tag_name)",
        "asset=$($asset.name)",
        "backend=$Backend",
        "arch=$arch",
        "installed_utc=$([DateTime]::UtcNow.ToString('o'))"
    ) -join "`n"
    Set-Content -LiteralPath (Join-Path $target "ROLDEX_RUNTIME_VERSION.txt") -Value $versionInfo -Encoding UTF8

    [Environment]::SetEnvironmentVariable("ROLDEX_LLAMA_SERVER", $serverPath, "User")
    $env:ROLDEX_LLAMA_SERVER = $serverPath

    Write-Host ""
    Write-Host "llama.cpp local runtime installed successfully." -ForegroundColor Green
    Write-Host "Runtime: $serverPath"
    Write-Host "Release: $($runtimeRelease.tag_name)"
    Write-Host "Backend: $Backend"
    Write-Host ""
    Write-Host "No model was downloaded. When you choose a model/drive later, pass its .gguf path to start-local-ai.ps1."
} finally {
    Remove-Item -LiteralPath $tempRoot -Recurse -Force -ErrorAction SilentlyContinue
}
