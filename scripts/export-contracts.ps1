[CmdletBinding()]
param(
    [string]$OutputPath = '',
    [switch]$CreateArchive,
    [string]$ArchiveOutputPath = ''
)

$ErrorActionPreference = 'Stop'

$repoRoot = Split-Path -Parent $PSScriptRoot
$contractsRoot = Join-Path $repoRoot 'contracts'
$schemaSourcePath = Join-Path $contractsRoot 'schemas'
$manifestsSourcePath = Join-Path $contractsRoot 'manifests'
$versionPolicyPath = Join-Path $repoRoot 'governance\version-policy.json'

if ([string]::IsNullOrWhiteSpace($OutputPath)) {
    $OutputPath = Join-Path $repoRoot 'artifacts\contracts'
}

$compatibilityManifestFile = Join-Path $manifestsSourcePath 'compatibility-manifest.json'
$compatibilityMatrixFile = Join-Path $manifestsSourcePath 'compatibility-matrix.json'

foreach ($requiredPath in @($compatibilityManifestFile, $compatibilityMatrixFile, $versionPolicyPath)) {
    if (-not (Test-Path $requiredPath)) {
        throw "Missing required file: $requiredPath"
    }
}

$versionPolicy = Get-Content -Raw -Path $versionPolicyPath | ConvertFrom-Json
$bundleVersion = $versionPolicy.bundleVersion
$schemaVersion = $versionPolicy.schemaVersion
$manifestPackageName = $versionPolicy.manifestPackage.name
$manifestPackageVersion = $versionPolicy.manifestPackage.version
$compatibilityManifest = Get-Content -Raw -Path $compatibilityManifestFile | ConvertFrom-Json
$compatibilityMatrix = Get-Content -Raw -Path $compatibilityMatrixFile | ConvertFrom-Json

if ($CreateArchive -and [string]::IsNullOrWhiteSpace($ArchiveOutputPath)) {
    $ArchiveOutputPath = Join-Path $repoRoot "artifacts\distribution\elegy-contracts-$bundleVersion.zip"
}

if ($compatibilityManifest.package.name -ne $manifestPackageName) {
    throw "Compatibility manifest package name '$($compatibilityManifest.package.name)' does not match governance/version-policy.json manifest package name '$manifestPackageName'."
}

if ($compatibilityManifest.package.version -ne $manifestPackageVersion) {
    throw "Compatibility manifest package version '$($compatibilityManifest.package.version)' does not match governance/version-policy.json manifest package version '$manifestPackageVersion'."
}

foreach ($schemaEntry in $compatibilityManifest.schemas) {
    $schemaFilePath = Join-Path $schemaSourcePath $schemaEntry.file
    if (-not (Test-Path $schemaFilePath)) {
        throw "Schema file referenced in manifest not found: $schemaFilePath"
    }
    foreach ($fixture in $schemaEntry.fixtures) {
        $fixturePath = Join-Path $contractsRoot $fixture
        if (-not (Test-Path $fixturePath)) {
            throw "Fixture file referenced in manifest not found: $fixturePath"
        }
    }
}

$canonicalSchemaManifest = $compatibilityManifest.schemas | Where-Object { $_.name -eq 'canonical-workflow' } | Select-Object -First 1
if ($null -eq $canonicalSchemaManifest) {
    throw 'Compatibility manifest is missing the canonical-workflow entry.'
}

if ($canonicalSchemaManifest.schemaVersion -ne $schemaVersion) {
    throw "Compatibility manifest schema version '$($canonicalSchemaManifest.schemaVersion)' does not match governance/version-policy.json schemaVersion '$schemaVersion'."
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

foreach ($schemaEntry in $compatibilityManifest.schemas) {
    $schemaFilePath = Join-Path $schemaSourcePath $schemaEntry.file
    Copy-Item -Path $schemaFilePath -Destination (Join-Path $OutputPath $schemaEntry.file) -Force
    foreach ($fixture in $schemaEntry.fixtures) {
        $fixturePath = Join-Path $contractsRoot $fixture
        Copy-Item -Path $fixturePath -Destination (Join-Path $OutputPath $fixture) -Force
    }
}

$supplementalFixtureFiles = @(
    'fixtures\mcp-server-descriptor.parity.json',
    'fixtures\mcp-analysis-result.parity.json',
    'fixtures\mcp-parity-expected.json'
)

foreach ($fixture in $supplementalFixtureFiles) {
    $fixturePath = Join-Path $contractsRoot $fixture
    if (-not (Test-Path $fixturePath)) {
        throw "Supplemental fixture file not found: $fixturePath"
    }

    Copy-Item -Path $fixturePath -Destination (Join-Path $OutputPath $fixture) -Force
}

Copy-Item -Path $compatibilityManifestFile -Destination (Join-Path $OutputPath 'compatibility-manifest.json') -Force
Copy-Item -Path $compatibilityMatrixFile -Destination (Join-Path $OutputPath 'compatibility-matrix.json') -Force

Write-Host "Exported contracts artifacts to: $OutputPath"
Get-ChildItem -Path $OutputPath -Recurse -File | ForEach-Object {
    Write-Host " - $($_.FullName)"
}

if ($CreateArchive) {
    $archiveDirectory = Split-Path -Parent $ArchiveOutputPath
    if (-not [string]::IsNullOrWhiteSpace($archiveDirectory)) {
        New-Item -ItemType Directory -Path $archiveDirectory -Force | Out-Null
    }

    if (Test-Path $ArchiveOutputPath) {
        Remove-Item -Path $ArchiveOutputPath -Force
    }

    Compress-Archive -Path (Join-Path $OutputPath '*') -DestinationPath $ArchiveOutputPath -CompressionLevel Optimal
    Write-Host "Created contracts archive: $ArchiveOutputPath"
}
