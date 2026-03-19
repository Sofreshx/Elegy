param(
    [string]$ModeSelectionPath = "artifacts/ws3/mode-selection.json",
    [ValidateSet("warn", "strict")]
    [string]$Mode,
    [string]$ViolationsPath,
    [string]$OutputPath = "artifacts/ws3/gate-decision.json"
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

function Get-ResolvedMode([string]$ProvidedMode, [string]$SelectionPath)
{
    if (-not [string]::IsNullOrWhiteSpace($ProvidedMode))
    {
        return $ProvidedMode
    }

    if (-not (Test-Path -LiteralPath $SelectionPath))
    {
        throw "Mode selection artifact not found: $SelectionPath"
    }

    $selection = Get-Content -LiteralPath $SelectionPath -Raw | ConvertFrom-Json
    if ($null -eq $selection.resolvedMode -or [string]::IsNullOrWhiteSpace([string]$selection.resolvedMode))
    {
        throw "Mode selection artifact does not contain 'resolvedMode': $SelectionPath"
    }

    return [string]$selection.resolvedMode
}

function Get-Violations([string]$Path)
{
    if ([string]::IsNullOrWhiteSpace($Path) -or -not (Test-Path -LiteralPath $Path))
    {
        return @()
    }

    $raw = Get-Content -LiteralPath $Path -Raw
    if ([string]::IsNullOrWhiteSpace($raw))
    {
        return @()
    }

    $parsed = $raw | ConvertFrom-Json
    if ($null -eq $parsed)
    {
        return @()
    }

    if ($parsed -is [System.Array])
    {
        return @($parsed)
    }

    if ($null -ne $parsed.violations)
    {
        return @($parsed.violations)
    }

    return @($parsed)
}

$resolvedMode = Get-ResolvedMode -ProvidedMode $Mode -SelectionPath $ModeSelectionPath
if ($resolvedMode -notin @("warn", "strict"))
{
    throw "Unsupported mode '$resolvedMode'. Supported modes: warn, strict."
}

$violations = @(Get-Violations -Path $ViolationsPath)
$violationCount = $violations.Count
$severityCounts = @{
    critical = 0
    high = 0
    medium = 0
    low = 0
    unknown = 0
}

foreach ($violation in $violations)
{
    $severity = if ($null -ne $violation.severity -and -not [string]::IsNullOrWhiteSpace([string]$violation.severity))
    {
        ([string]$violation.severity).ToLowerInvariant()
    }
    else
    {
        "unknown"
    }

    if (-not $severityCounts.ContainsKey($severity))
    {
        $severity = "unknown"
    }

    $severityCounts[$severity]++
}

$isBlocking = ($resolvedMode -eq "strict" -and $violationCount -gt 0)
$decision = if ($isBlocking) { "block" } elseif ($violationCount -gt 0) { "warn" } else { "pass" }

$result = [pscustomobject]@{
    mode = $resolvedMode
    decision = $decision
    blocking = $isBlocking
    violationCount = $violationCount
    severityCounts = [pscustomobject]$severityCounts
    violationsPath = if ([string]::IsNullOrWhiteSpace($ViolationsPath)) { $null } else { $ViolationsPath }
    modeSelectionPath = if ([string]::IsNullOrWhiteSpace($ModeSelectionPath)) { $null } else { $ModeSelectionPath }
    generatedAtUtc = (Get-Date).ToUniversalTime().ToString("o")
}

Write-Utf8Json -Path $OutputPath -Value $result
Write-Host "Gate decision: $decision (mode: $resolvedMode, violations: $violationCount)."
Write-Host "Gate decision artifact: $OutputPath"

if ($isBlocking)
{
    Write-Error "Strict mode violation gate failed with $violationCount violation(s)."
    exit 1
}
