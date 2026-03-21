Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$repoRoot = Split-Path -Parent $PSScriptRoot

function Get-RelativePath([string]$path) {
    $relativePath = [System.IO.Path]::GetRelativePath($repoRoot, $path)
    return $relativePath.Replace('\', '/')
}

$allowedCsprojPaths = [System.Collections.Generic.HashSet[string]]::new([System.StringComparer]::OrdinalIgnoreCase)
foreach ($path in @(
    'src/Elegy.Formalization.AgentFactory/Elegy.Formalization.AgentFactory.csproj',
    'src/Elegy.Formalization.Agents/Elegy.Formalization.Agents.csproj',
    'src/Elegy.Formalization.Contracts/Elegy.Formalization.Contracts.csproj',
    'src/Elegy.Formalization.Core/Elegy.Formalization.Core.csproj',
    'src/Elegy.Formalization.DynamicSkills/Elegy.Formalization.DynamicSkills.csproj',
    'src/Elegy.Formalization.Governance/Elegy.Formalization.Governance.csproj',
    'src/Elegy.Formalization.Mcp/Elegy.Formalization.Mcp.csproj',
    'src/Elegy.Formalization.Monitoring/Elegy.Formalization.Monitoring.csproj',
    'src/Elegy.Formalization.Serialization/Elegy.Formalization.Serialization.csproj',
    'src/Elegy.Formalization.SkillForge/Elegy.Formalization.SkillForge.csproj',
    'src/Elegy.Formalization.Skills/Elegy.Formalization.Skills.csproj',
    'src/Elegy.Formalization.Skills.Discovery/Elegy.Formalization.Skills.Discovery.csproj',
    'src/Elegy.Formalization.Validation/Elegy.Formalization.Validation.csproj',
    'tests/Elegy.Formalization.AgentFactory.Tests/Elegy.Formalization.AgentFactory.Tests.csproj',
    'tests/Elegy.Formalization.Agents.Tests/Elegy.Formalization.Agents.Tests.csproj',
    'tests/Elegy.Formalization.Core.Tests/Elegy.Formalization.Core.Tests.csproj',
    'tests/Elegy.Formalization.DynamicSkills.Tests/Elegy.Formalization.DynamicSkills.Tests.csproj',
    'tests/Elegy.Formalization.Governance.Tests/Elegy.Formalization.Governance.Tests.csproj',
    'tests/Elegy.Formalization.Mcp.Tests/Elegy.Formalization.Mcp.Tests.csproj',
    'tests/Elegy.Formalization.Monitoring.Tests/Elegy.Formalization.Monitoring.Tests.csproj',
    'tests/Elegy.Formalization.Serialization.Tests/Elegy.Formalization.Serialization.Tests.csproj',
    'tests/Elegy.Formalization.SkillForge.Tests/Elegy.Formalization.SkillForge.Tests.csproj',
    'tests/Elegy.Formalization.Skills.Discovery.Tests/Elegy.Formalization.Skills.Discovery.Tests.csproj',
    'tests/Elegy.Formalization.Skills.Tests/Elegy.Formalization.Skills.Tests.csproj',
    'tests/Elegy.Formalization.Validation.Tests/Elegy.Formalization.Validation.Tests.csproj'
)) {
    $null = $allowedCsprojPaths.Add($path)
}

$currentCsprojPaths = Get-ChildItem -Path (Join-Path $repoRoot 'src'), (Join-Path $repoRoot 'tests') -Recurse -Filter '*.csproj' -File |
    ForEach-Object { Get-RelativePath $_.FullName }

$unexpectedCsprojPaths = @($currentCsprojPaths | Where-Object { -not $allowedCsprojPaths.Contains($_) } | Sort-Object)
if ($unexpectedCsprojPaths.Count -gt 0) {
    $message = @(
        'New compiled .NET project surfaces are frozen during the zero-dotnet Elegy migration.',
        'Do not add new src/ or tests/ csproj files while the current estate is being removed.',
        'Unexpected project paths:'
    ) + ($unexpectedCsprojPaths | ForEach-Object { "- $_" })
    throw ($message -join [Environment]::NewLine)
}

$currentSummary = @($currentCsprojPaths | Sort-Object)
Write-Host 'Dotnet exit freeze checks passed.'
Write-Host ("Tracked remaining csproj surfaces: {0}" -f ($currentSummary -join ', '))