param(
    [string]$PolicyPath,
    [string]$BranchName = $env:GITHUB_REF_NAME,
    [string]$EnvironmentName = $env:GITHUB_ENVIRONMENT,
    [string]$WorkflowName = $env:GITHUB_WORKFLOW,
    [string]$OutputPath = "artifacts/ws3/mode-selection.json"
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

function Test-Match([string]$InputValue, [string]$MatchType, [string]$Expected)
{
    if ([string]::IsNullOrWhiteSpace($InputValue))
    {
        return $false
    }

    $candidate = $InputValue.Trim()
    $target = if ($null -eq $Expected) { "" } else { $Expected }

    switch ($MatchType)
    {
        "exact" { return $candidate.Equals($target, [System.StringComparison]::OrdinalIgnoreCase) }
        "prefix" { return $candidate.StartsWith($target, [System.StringComparison]::OrdinalIgnoreCase) }
        "contains" { return $candidate.IndexOf($target, [System.StringComparison]::OrdinalIgnoreCase) -ge 0 }
        "regex" { return [System.Text.RegularExpressions.Regex]::IsMatch($candidate, $target, [System.Text.RegularExpressions.RegexOptions]::IgnoreCase) }
        default { throw "Unsupported matchType '$MatchType'." }
    }
}

function Find-RuleMatch($Rules, [string]$InputValue)
{
    if ($null -eq $Rules)
    {
        return $null
    }

    foreach ($rule in $Rules)
    {
        if (Test-Match -InputValue $InputValue -MatchType $rule.matchType -Expected $rule.value)
        {
            return $rule
        }
    }

    return $null
}

$repoRoot = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$resolvedPolicyPath = if ([string]::IsNullOrWhiteSpace($PolicyPath))
{
    Join-Path $repoRoot "policies/formalization/visual-llm-enforcement-policy.json"
}
else
{
    $PolicyPath
}

if (-not (Test-Path -LiteralPath $resolvedPolicyPath))
{
    throw "Policy file not found: $resolvedPolicyPath"
}

$policy = Get-Content -LiteralPath $resolvedPolicyPath -Raw | ConvertFrom-Json
$precedence = @($policy.precedence)
if ($precedence.Count -eq 0)
{
    throw "Policy precedence is empty: $resolvedPolicyPath"
}

$contextByScope = @{
    workflow = if ([string]::IsNullOrWhiteSpace($WorkflowName)) { "" } else { $WorkflowName.Trim() }
    environment = if ([string]::IsNullOrWhiteSpace($EnvironmentName)) { "" } else { $EnvironmentName.Trim() }
    branch = if ([string]::IsNullOrWhiteSpace($BranchName)) { "" } else { $BranchName.Trim() }
}

$rulesByScope = @{
    workflow = $policy.workflowRules
    environment = $policy.environmentRules
    branch = $policy.branchRules
}

$selectedRule = $null
$selectedScope = $null
$evaluations = @()

foreach ($scope in $precedence)
{
    $scopeName = [string]$scope
    if (-not $contextByScope.ContainsKey($scopeName))
    {
        throw "Unsupported scope in precedence: '$scopeName'"
    }

    $inputValue = [string]$contextByScope[$scopeName]
    $matchingRule = Find-RuleMatch -Rules $rulesByScope[$scopeName] -InputValue $inputValue

    $evaluations += [pscustomobject]@{
        scope = $scopeName
        input = $inputValue
        ruleMatched = ($null -ne $matchingRule)
        ruleId = if ($null -ne $matchingRule) { $matchingRule.id } else { $null }
    }

    if ($null -eq $selectedRule -and $null -ne $matchingRule)
    {
        $selectedRule = $matchingRule
        $selectedScope = $scopeName
    }
}

$resolvedMode = if ($null -ne $selectedRule)
{
    [string]$selectedRule.mode
}
elseif ($null -ne $policy.defaults -and -not [string]::IsNullOrWhiteSpace($policy.defaults.mode))
{
    [string]$policy.defaults.mode
}
else
{
    "warn"
}

if ($resolvedMode -notin @("warn", "strict"))
{
    throw "Resolved mode '$resolvedMode' is invalid. Supported modes: warn, strict."
}

$result = [pscustomobject]@{
    policyId = $policy.policyId
    policyVersion = $policy.version
    resolvedMode = $resolvedMode
    source = if ($null -ne $selectedRule) { $selectedScope } else { "default" }
    matchedRuleId = if ($null -ne $selectedRule) { $selectedRule.id } else { "default-mode" }
    precedence = $precedence
    context = [pscustomobject]@{
        workflow = $contextByScope.workflow
        environment = $contextByScope.environment
        branch = $contextByScope.branch
    }
    evaluations = $evaluations
    generatedAtUtc = (Get-Date).ToUniversalTime().ToString("o")
}

Write-Utf8Json -Path $OutputPath -Value $result
Write-Host "Resolved mode '$($result.resolvedMode)' from '$($result.source)' (rule: $($result.matchedRuleId))."
Write-Host "Mode selection artifact: $OutputPath"
