[CmdletBinding()]
param(
    [switch]$EmitJson
)

$ErrorActionPreference = 'Stop'

$repoRoot = Split-Path -Parent $PSScriptRoot
$policyPath = Join-Path $repoRoot 'governance\boundary-policy.json'

if (-not (Test-Path $policyPath)) {
    throw "Missing boundary policy: $policyPath"
}

$policy = Get-Content -Raw -Path $policyPath | ConvertFrom-Json

$missingFiles = [System.Collections.Generic.List[string]]::new()
$ruleViolations = [System.Collections.Generic.List[object]]::new()

foreach ($relativePath in $policy.requiredFiles) {
    $fullPath = Join-Path $repoRoot $relativePath
    if (-not (Test-Path $fullPath)) {
        $missingFiles.Add($relativePath) | Out-Null
    }
}

foreach ($rule in $policy.rules) {
    foreach ($relativePath in $rule.files) {
        $fullPath = Join-Path $repoRoot $relativePath

        if (-not (Test-Path $fullPath)) {
            $ruleViolations.Add([pscustomobject]@{
                Rule = $rule.name
                File = $relativePath
                Type = 'missing-file'
                Pattern = $null
                Message = 'Policy-scoped file was not found.'
            }) | Out-Null
            continue
        }

        $content = Get-Content -Raw -Path $fullPath

        foreach ($pattern in $rule.forbiddenPatterns) {
            if ($content -match $pattern) {
                $ruleViolations.Add([pscustomobject]@{
                    Rule = $rule.name
                    File = $relativePath
                    Type = 'forbidden-pattern'
                    Pattern = $pattern
                    Message = 'Forbidden pattern matched.'
                }) | Out-Null
            }
        }

        foreach ($pattern in $rule.requiredPatterns) {
            if ($content -notmatch $pattern) {
                $ruleViolations.Add([pscustomobject]@{
                    Rule = $rule.name
                    File = $relativePath
                    Type = 'required-pattern-missing'
                    Pattern = $pattern
                    Message = 'Required pattern was not found.'
                }) | Out-Null
            }
        }
    }
}

if ($EmitJson) {
    [pscustomobject]@{
        missingFiles = $missingFiles
        violations = $ruleViolations
    } | ConvertTo-Json -Depth 6
}

if ($missingFiles.Count -gt 0) {
    throw ('Missing repo-boundary required files (package-boundaries compatibility policy): ' + ($missingFiles -join ', '))
}

if ($ruleViolations.Count -gt 0) {
    $message = $ruleViolations | ForEach-Object {
        "[{0}] {1}: {2} ({3})" -f $_.Rule, $_.File, $_.Message, $_.Pattern
    }

    throw ($message -join [Environment]::NewLine)
}

Write-Host 'Repo-boundary validation passed.'
Write-Host " - required files: $($policy.requiredFiles.Count)"
Write-Host " - policy rules: $($policy.rules.Count)"
foreach ($rule in $policy.rules) {
    Write-Host (" - {0} -> {1} file(s)" -f $rule.name, $rule.files.Count)
}