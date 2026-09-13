[CmdletBinding()]
param(
    [string]$Endpoint = "http://127.0.0.1:8080/v1/chat/completions",
    [string]$Model = "roldex-local",
    [ValidateRange(5, 300)]
    [int]$TimeoutSeconds = 90
)

$ErrorActionPreference = "Stop"

$endpointUri = [Uri]$Endpoint
$baseUri = "{0}://{1}:{2}" -f $endpointUri.Scheme, $endpointUri.Host, $endpointUri.Port
$healthUrl = "$baseUri/health"

Write-Host "Roldex local AI compatibility check"
Write-Host "Endpoint: $Endpoint"
Write-Host "Model: $Model"
Write-Host ""

try {
    Invoke-RestMethod -Uri $healthUrl -Method Get -TimeoutSec 5 | Out-Null
    Write-Host "[PASS] Local server health endpoint" -ForegroundColor Green
} catch {
    throw "Local AI server is not healthy at $healthUrl. $($_.Exception.Message)"
}

$chatBody = @{
    model = $Model
    messages = @(
        @{
            role = "system"
            content = "You are a concise Roblox Studio coding assistant."
        },
        @{
            role = "user"
            content = "Reply with exactly ROLDEX_OK and nothing else."
        }
    )
    temperature = 0
    stream = $false
} | ConvertTo-Json -Depth 12

$stopwatch = [Diagnostics.Stopwatch]::StartNew()
$chat = Invoke-RestMethod `
    -Uri $Endpoint `
    -Method Post `
    -ContentType "application/json" `
    -Body $chatBody `
    -TimeoutSec $TimeoutSeconds
$stopwatch.Stop()

$content = $chat.choices[0].message.content
if (-not $content) {
    throw "The server returned no assistant text for the basic chat test."
}
Write-Host ("[PASS] Basic chat response ({0:N2}s)" -f $stopwatch.Elapsed.TotalSeconds) -ForegroundColor Green

$toolBody = @{
    model = $Model
    messages = @(
        @{
            role = "system"
            content = "You are testing function calling. When instructed, use the supplied tool instead of answering in prose."
        },
        @{
            role = "user"
            content = "Call roldex_probe with value roblox-studio now."
        }
    )
    tools = @(
        @{
            type = "function"
            function = @{
                name = "roldex_probe"
                description = "Compatibility probe for the Roldex agent tool protocol."
                parameters = @{
                    type = "object"
                    properties = @{
                        value = @{
                            type = "string"
                        }
                    }
                    required = @("value")
                    additionalProperties = $false
                }
            }
        }
    )
    tool_choice = "auto"
    parallel_tool_calls = $false
    temperature = 0
    stream = $false
} | ConvertTo-Json -Depth 20

$toolStopwatch = [Diagnostics.Stopwatch]::StartNew()
$toolResponse = Invoke-RestMethod `
    -Uri $Endpoint `
    -Method Post `
    -ContentType "application/json" `
    -Body $toolBody `
    -TimeoutSec $TimeoutSeconds
$toolStopwatch.Stop()

$toolCalls = $toolResponse.choices[0].message.tool_calls
if (-not $toolCalls -or $toolCalls.Count -lt 1) {
    throw @"
The model/server answered, but it did not return an OpenAI-style tool call.
Roldex needs reliable tool calling for files, shell, Git, and Roblox Studio control.
Try a model with native function/tool calling and run llama-server with --jinja.
"@
}

$probe = $toolCalls | Where-Object { $_.function.name -eq "roldex_probe" } | Select-Object -First 1
if (-not $probe) {
    throw "A tool call was returned, but not the requested roldex_probe call."
}

Write-Host ("[PASS] Tool calling ({0:N2}s)" -f $toolStopwatch.Elapsed.TotalSeconds) -ForegroundColor Green
Write-Host ""
Write-Host "This local model/server is compatible with the core Roldex chat + tool protocol." -ForegroundColor Green
