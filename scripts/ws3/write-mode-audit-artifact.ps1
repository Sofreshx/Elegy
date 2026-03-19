param(
    [string]$ModeSelectionPath = "artifacts/ws3/mode-selection.json",
    [string]$GateDecisionPath = "artifacts/ws3/gate-decision.json",
    [string]$OutputPath = "artifacts/ws3/mode-audit.json"
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Write-Utf8Json([string]$Path, $Value)
{
    $directory = Split-Path -Parent $Path
    if (-not [string]::IsNullOrWhiteSpace($directory))
    {
        New-Item -Path $directory -ItemType Directory -Force | Out-Null
    }

    $json = $Value | ConvertTo-Json -Depth 10
    $encoding = [System.Text.UTF8Encoding]::new($false)
    [System.IO.File]::WriteAllText($Path, $json, $encoding)
}

if (-not (Test-Path -LiteralPath $ModeSelectionPath))
{
    throw "Mode selection artifact not found: $ModeSelectionPath"
}

if (-not (Test-Path -LiteralPath $GateDecisionPath))
{
    throw "Gate decision artifact not found: $GateDecisionPath"
}

$modeSelection = Get-Content -LiteralPath $ModeSelectionPath -Raw | ConvertFrom-Json
$gateDecision = Get-Content -LiteralPath $GateDecisionPath -Raw | ConvertFrom-Json

$audit = [pscustomobject]@{
    policy = [pscustomobject]@{
        id = $modeSelection.policyId
        version = $modeSelection.policyVersion
    }
    modeSelection = $modeSelection
    gateDecision = $gateDecision
    generatedAtUtc = (Get-Date).ToUniversalTime().ToString("o")
}

Write-Utf8Json -Path $OutputPath -Value $audit
Write-Host "Mode audit artifact: $OutputPath"
