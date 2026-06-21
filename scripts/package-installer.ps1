[CmdletBinding()]
param(
    [string]$OutputDirectory = ''
)

$ErrorActionPreference = 'Stop'

$repoRoot = Split-Path -Parent $PSScriptRoot
$packageReadmePath = Join-Path $repoRoot 'PACKAGE_README.md'

# Returns the schema version from contracts/schemas/schema-version.json (not the bundle version — those are conceptually different)
function Get-SchemaVersion {
    param(
        [string]$RepositoryRoot
    )

    $schemaVersionPath = Join-Path $RepositoryRoot 'contracts/schemas/schema-version.json'
    if (-not (Test-Path $schemaVersionPath)) {
        throw "Missing schema version: $schemaVersionPath"
    }

    $versionPolicy = Get-Content -Raw -Path $schemaVersionPath | ConvertFrom-Json
    if ([string]::IsNullOrWhiteSpace($versionPolicy.schemaVersion)) {
        throw 'Schema version file did not include schemaVersion.'
    }

    return $versionPolicy.schemaVersion
}

if ([string]::IsNullOrWhiteSpace($OutputDirectory)) {
    $OutputDirectory = Join-Path $repoRoot 'artifacts/distribution'
}

$installerScriptPath = Join-Path $repoRoot 'scripts/install-distribution.ps1'
if (-not (Test-Path $installerScriptPath)) {
    throw "Missing installer source: $installerScriptPath"
}

if (-not (Test-Path $packageReadmePath)) {
    throw "Missing package README source: $packageReadmePath"
}

$schemaVersion = Get-SchemaVersion -RepositoryRoot $repoRoot

New-Item -ItemType Directory -Path $OutputDirectory -Force | Out-Null

$assetBaseName = "elegy-installer-$schemaVersion"
$stagingDirectory = Join-Path $OutputDirectory $assetBaseName
$archivePath = Join-Path $OutputDirectory "$assetBaseName.zip"

if (Test-Path $stagingDirectory) {
    Remove-Item -Path $stagingDirectory -Recurse -Force
}

if (Test-Path $archivePath) {
    Remove-Item -Path $archivePath -Force
}

New-Item -ItemType Directory -Path $stagingDirectory -Force | Out-Null
Copy-Item -Path $installerScriptPath -Destination (Join-Path $stagingDirectory 'install-distribution.ps1') -Force
Copy-Item -Path $packageReadmePath -Destination (Join-Path $stagingDirectory 'README.md') -Force

Compress-Archive -Path (Join-Path $stagingDirectory '*') -DestinationPath $archivePath -CompressionLevel Optimal
Remove-Item -Path $stagingDirectory -Recurse -Force

Write-Host "Packaged installer archive: $archivePath"
Write-Host " - schema version: $schemaVersion"
