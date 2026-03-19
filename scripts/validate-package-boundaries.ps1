[CmdletBinding()]
param(
    [switch]$EmitJson
)

$ErrorActionPreference = 'Stop'

$repoRoot = Split-Path -Parent $PSScriptRoot
$sourceRoot = Join-Path $repoRoot 'src'

$allowedReferences = [ordered]@{
    'Elegy.Formalization.Core' = @()
    'Elegy.Formalization.Contracts' = @('Elegy.Formalization.Core')
    'Elegy.Formalization.Serialization' = @('Elegy.Formalization.Core')
    'Elegy.Formalization.Validation' = @('Elegy.Formalization.Core')
    'Elegy.Formalization.Governance' = @('Elegy.Formalization.Core')
    'Elegy.Formalization.Projections.Mermaid' = @('Elegy.Formalization.Core')
    'Elegy.Formalization.Skills' = @('Elegy.Formalization.Core')
    'Elegy.Formalization.Skills.Discovery' = @('Elegy.Formalization.Core', 'Elegy.Formalization.Skills')
    'Elegy.Formalization.DynamicSkills' = @('Elegy.Formalization.Core', 'Elegy.Formalization.Skills', 'Elegy.Formalization.Monitoring')
    'Elegy.Formalization.Monitoring' = @('Elegy.Formalization.Core')
    'Elegy.Formalization.Mcp' = @('Elegy.Formalization.Skills')
    'Elegy.Formalization.SkillForge' = @('Elegy.Formalization.Core', 'Elegy.Formalization.Skills', 'Elegy.Formalization.DynamicSkills', 'Elegy.Formalization.Governance')
    'Elegy.Formalization.Agents' = @('Elegy.Formalization.Core')
    'Elegy.Formalization.AgentFactory' = @('Elegy.Formalization.Core', 'Elegy.Formalization.Agents', 'Elegy.Formalization.Governance')
}

$projectFiles = Get-ChildItem -Path $sourceRoot -Recurse -Filter *.csproj | Sort-Object FullName
if ($projectFiles.Count -eq 0) {
    throw 'No source projects were found under src/.'
}

$projectNames = @{}
foreach ($projectFile in $projectFiles) {
    $projectName = [System.IO.Path]::GetFileNameWithoutExtension($projectFile.Name)
    $projectNames[$projectName] = $projectFile.FullName
}

$missingPolicies = @($projectNames.Keys | Where-Object { -not $allowedReferences.Contains($_) } | Sort-Object)
$stalePolicies = @($allowedReferences.Keys | Where-Object { -not $projectNames.Contains($_) } | Sort-Object)

$violations = [System.Collections.Generic.List[object]]::new()

foreach ($projectName in ($projectNames.Keys | Sort-Object)) {
    if (-not $allowedReferences.Contains($projectName)) {
        continue
    }

    [xml]$projectXml = Get-Content -Raw -Path $projectNames[$projectName]
    $references = @($projectXml.SelectNodes('//ProjectReference'))

    foreach ($reference in $references) {
        $include = [string]$reference.Include
        if ([string]::IsNullOrWhiteSpace($include)) {
            continue
        }

        $referencedPath = [System.IO.Path]::GetFullPath((Join-Path (Split-Path -Parent $projectNames[$projectName]) $include))
        $referencedName = [System.IO.Path]::GetFileNameWithoutExtension($referencedPath)

        $violations.Add([pscustomobject]@{
            Project = $projectName
            Reference = $referencedName
            Allowed = $allowedReferences[$projectName] -contains $referencedName
            KnownReference = $projectNames.Contains($referencedName)
            Policy = $allowedReferences[$projectName] -join ', '
        }) | Out-Null
    }
}

$invalidReferences = @($violations | Where-Object { -not $_.Allowed -or -not $_.KnownReference })

if ($EmitJson) {
    [pscustomobject]@{
        missingPolicies = $missingPolicies
        stalePolicies = $stalePolicies
        references = $violations
        invalidReferences = $invalidReferences
    } | ConvertTo-Json -Depth 6
}

if ($missingPolicies.Count -gt 0) {
    throw ('Missing package-boundary policy entries for: ' + ($missingPolicies -join ', '))
}

if ($stalePolicies.Count -gt 0) {
    throw ('Package-boundary policy contains stale entries for: ' + ($stalePolicies -join ', '))
}

if ($invalidReferences.Count -gt 0) {
    $message = $invalidReferences | ForEach-Object {
        $reason = if (-not $_.KnownReference) {
            'reference target is not a known source project'
        }
        else {
            'reference is not allowed by policy'
        }

        "{0} -> {1}: {2}. Allowed references: [{3}]" -f $_.Project, $_.Reference, $reason, $_.Policy
    }

    throw ($message -join [Environment]::NewLine)
}

Write-Host 'Package-boundary validation passed.'
foreach ($projectName in ($projectNames.Keys | Sort-Object)) {
    $allowed = $allowedReferences[$projectName]
    if ($allowed.Count -eq 0) {
        Write-Host " - $projectName -> <none>"
        continue
    }

    Write-Host (" - {0} -> {1}" -f $projectName, ($allowed -join ', '))
}