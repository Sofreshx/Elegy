[CmdletBinding()]
param(
    [string]$OutputPath = ''
)

$ErrorActionPreference = 'Stop'

$repoRoot = Split-Path -Parent $PSScriptRoot
$contractsResourcesPath = Join-Path $repoRoot 'src\Elegy.Formalization.Contracts\Resources'
$propsPath = Join-Path $repoRoot 'Directory.Build.props'
$schemaVersionPath = Join-Path $repoRoot 'schemas\schema-version.json'

if ([string]::IsNullOrWhiteSpace($OutputPath)) {
    $OutputPath = Join-Path $repoRoot 'artifacts\contracts'
}

$schemaFile = Join-Path $contractsResourcesPath 'canonical-workflow.schema.json'
$fixtureFile = Join-Path $contractsResourcesPath 'fixtures\canonical-workflow.minimal.json'
$compatibilityManifestFile = Join-Path $contractsResourcesPath 'compatibility-manifest.json'
$compatibilityMatrixFile = Join-Path $contractsResourcesPath 'compatibility-matrix.json'

foreach ($requiredPath in @($schemaFile, $fixtureFile, $compatibilityManifestFile, $compatibilityMatrixFile, $propsPath, $schemaVersionPath)) {
    if (-not (Test-Path $requiredPath)) {
        throw "Missing required file: $requiredPath"
    }
}

[xml]$propsXml = Get-Content -Raw -Path $propsPath
$packageVersion = $propsXml.SelectSingleNode('//Project/PropertyGroup/VersionPrefix').InnerText
$schemaVersion = (Get-Content -Raw -Path $schemaVersionPath | ConvertFrom-Json).schemaVersion
$compatibilityManifest = Get-Content -Raw -Path $compatibilityManifestFile | ConvertFrom-Json
$compatibilityMatrix = Get-Content -Raw -Path $compatibilityMatrixFile | ConvertFrom-Json

if ($compatibilityManifest.package.version -ne $packageVersion) {
    throw "Compatibility manifest package version '$($compatibilityManifest.package.version)' does not match Directory.Build.props VersionPrefix '$packageVersion'."
}

$canonicalSchemaManifest = $compatibilityManifest.schemas | Where-Object { $_.name -eq 'canonical-workflow' } | Select-Object -First 1
if ($null -eq $canonicalSchemaManifest) {
    throw 'Compatibility manifest is missing the canonical-workflow entry.'
}

if ($canonicalSchemaManifest.schemaVersion -ne $schemaVersion) {
    throw "Compatibility manifest schema version '$($canonicalSchemaManifest.schemaVersion)' does not match schemas/schema-version.json '$schemaVersion'."
}

if (-not $compatibilityMatrix.matrixVersion) {
    throw 'Compatibility matrix is missing matrixVersion.'
}

if ($null -eq $compatibilityMatrix.entries -or $compatibilityMatrix.entries.Count -eq 0) {
    throw 'Compatibility matrix must include at least one entry.'
}

if (Test-Path $OutputPath) {
    Remove-Item -Path (Join-Path $OutputPath '*') -Recurse -Force
}

New-Item -ItemType Directory -Path $OutputPath -Force | Out-Null
New-Item -ItemType Directory -Path (Join-Path $OutputPath 'fixtures') -Force | Out-Null

Copy-Item -Path $schemaFile -Destination (Join-Path $OutputPath 'canonical-workflow.schema.json') -Force
Copy-Item -Path $fixtureFile -Destination (Join-Path $OutputPath 'fixtures\canonical-workflow.minimal.json') -Force
Copy-Item -Path $compatibilityManifestFile -Destination (Join-Path $OutputPath 'compatibility-manifest.json') -Force
Copy-Item -Path $compatibilityMatrixFile -Destination (Join-Path $OutputPath 'compatibility-matrix.json') -Force

Write-Host "Exported contracts artifacts to: $OutputPath"
Get-ChildItem -Path $OutputPath -Recurse -File | ForEach-Object {
    Write-Host " - $($_.FullName)"
}
