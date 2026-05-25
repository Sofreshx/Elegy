[CmdletBinding()]
param(
    [ValidateSet('major', 'minor', 'patch')]
    [string]$PackageBump = 'patch',

    [string]$PackageVersion,

    [ValidateSet('major', 'minor', 'patch')]
    [string]$SchemaBump,

    [string]$SchemaVersion,

    [switch]$DryRun
)

$ErrorActionPreference = 'Stop'

$repoRoot = Split-Path -Parent $PSScriptRoot
$versionPolicyPath = Join-Path $repoRoot 'governance\version-policy.json'
$schemaPath = Join-Path $repoRoot 'schemas\schema-version.json'
$compatibilityManifestPath = Join-Path $repoRoot 'contracts\manifests\compatibility-manifest.json'
$compatibilityMatrixPath = Join-Path $repoRoot 'contracts\manifests\compatibility-matrix.json'
$rustSupportPath = Join-Path $repoRoot 'contracts\support\elegy-rust-support.json'
$semVerRegex = '^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?(?:\+[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?$'

# Compatibility note: the Package* parameter names remain for CLI stability, but the
# current authority model is file-native bundle/schema version governance.

function Assert-SemVer {
    param(
        [string]$Value,
        [string]$Name
    )

    if ([string]::IsNullOrWhiteSpace($Value) -or $Value -notmatch $semVerRegex) {
        throw "$Name '$Value' is not a valid SemVer string."
    }
}

function Bump-SemVer {
    param(
        [string]$Version,
        [ValidateSet('major', 'minor', 'patch')]
        [string]$Part
    )

    Assert-SemVer -Value $Version -Name 'Version'
    $base = $Version.Split('-', 2)[0].Split('+', 2)[0]
    $parts = $base.Split('.')
    $major = [int]$parts[0]
    $minor = [int]$parts[1]
    $patch = [int]$parts[2]

    switch ($Part) {
        'major' { $major++; $minor = 0; $patch = 0 }
        'minor' { $minor++; $patch = 0 }
        'patch' { $patch++ }
    }

    return "$major.$minor.$patch"
}

function Get-Major {
    param([string]$Version)
    Assert-SemVer -Value $Version -Name 'Version'
    return [int]($Version.Split('-', 2)[0].Split('+', 2)[0].Split('.')[0])
}

function New-CompatibleVersionRange {
    param([string]$Version)
    Assert-SemVer -Value $Version -Name 'Version'
    $nextMajor = (Get-Major -Version $Version) + 1
    return ">=$Version <$nextMajor.0.0"
}

if (-not (Test-Path $versionPolicyPath)) {
    throw "Missing file: $versionPolicyPath"
}

if (-not (Test-Path $schemaPath)) {
    throw "Missing file: $schemaPath"
}

if (-not (Test-Path $compatibilityManifestPath)) {
    throw "Missing file: $compatibilityManifestPath"
}

if (-not (Test-Path $compatibilityMatrixPath)) {
    throw "Missing file: $compatibilityMatrixPath"
}

if (-not (Test-Path $rustSupportPath)) {
    throw "Missing file: $rustSupportPath"
}

[pscustomobject]$versionPolicy = Get-Content -Raw -Path $versionPolicyPath | ConvertFrom-Json
if (-not $versionPolicy.bundleVersion) {
    throw "governance/version-policy.json must include bundleVersion."
}
$currentPackageVersion = [string]$versionPolicy.bundleVersion
Assert-SemVer -Value $currentPackageVersion -Name 'Current bundle version'

if (-not $versionPolicy.manifestPackage.version) {
    throw "governance/version-policy.json must include manifestPackage.version for legacy compatibility metadata."
}

$schemaJson = Get-Content -Raw -Path $schemaPath | ConvertFrom-Json
if (-not $schemaJson.schemaVersion) {
    throw "schemas/schema-version.json must include schemaVersion."
}
$currentSchemaVersion = [string]$schemaJson.schemaVersion
Assert-SemVer -Value $currentSchemaVersion -Name 'Current schema version'

[pscustomobject]$compatibilityManifest = Get-Content -Raw -Path $compatibilityManifestPath | ConvertFrom-Json
[pscustomobject]$compatibilityMatrix = Get-Content -Raw -Path $compatibilityMatrixPath | ConvertFrom-Json
[pscustomobject]$rustSupport = Get-Content -Raw -Path $rustSupportPath | ConvertFrom-Json

$nextPackageVersion = if ($PackageVersion) {
    Assert-SemVer -Value $PackageVersion -Name 'PackageVersion'
    $PackageVersion
}
else {
    Bump-SemVer -Version $currentPackageVersion -Part $PackageBump
}

$nextSchemaVersion = $currentSchemaVersion
if ($SchemaVersion) {
    Assert-SemVer -Value $SchemaVersion -Name 'SchemaVersion'
    $nextSchemaVersion = $SchemaVersion
}
elseif ($SchemaBump) {
    $nextSchemaVersion = Bump-SemVer -Version $currentSchemaVersion -Part $SchemaBump
}

if ((Get-Major -Version $nextSchemaVersion) -gt (Get-Major -Version $currentSchemaVersion) -and
    (Get-Major -Version $nextPackageVersion) -le (Get-Major -Version $currentPackageVersion)) {
    throw "Schema major bump detected ($currentSchemaVersion -> $nextSchemaVersion). Bundle major must also increase ($currentPackageVersion -> $nextPackageVersion is invalid)."
}

Write-Host "Current bundle version:  $currentPackageVersion"
Write-Host "Next bundle version:     $nextPackageVersion"
Write-Host "Current schema version:  $currentSchemaVersion"
Write-Host "Next schema version:     $nextSchemaVersion"

if ($DryRun) {
    Write-Host 'Dry run enabled. No files were changed.'
    exit 0
}

$versionPolicy.bundleVersion = $nextPackageVersion
$versionPolicy.manifestPackage.version = $nextPackageVersion
$versionPolicy.schemaVersion = $nextSchemaVersion
$versionPolicy.plannedFamilies | ForEach-Object {
    $_.targetBundleVersion = $nextPackageVersion
}
$versionPolicy | ConvertTo-Json -Depth 10 | Set-Content -Path $versionPolicyPath

$schemaJson.schemaVersion = $nextSchemaVersion
$schemaJson | ConvertTo-Json -Depth 10 | Set-Content -Path $schemaPath

$compatibilityManifest.package.version = $nextPackageVersion
$compatibilityManifest.plannedFamilies | ForEach-Object {
    $_.targetPackageVersion = $nextPackageVersion
}
$compatibilityManifest.schemas | ForEach-Object {
    $_.schemaVersion = $nextSchemaVersion
}
$compatibilityManifest | ConvertTo-Json -Depth 10 | Set-Content -Path $compatibilityManifestPath

$packageVersionRange = New-CompatibleVersionRange -Version $nextPackageVersion
$schemaVersionRange = New-CompatibleVersionRange -Version $nextSchemaVersion
$compatibilityMatrix.plannedFamilies | ForEach-Object {
    $_.targetPackageVersionRange = $packageVersionRange
}
$compatibilityMatrix.entries | ForEach-Object {
    $_.elegyPackageVersionRange = $packageVersionRange
    $_.elegySchemaVersionRange = $schemaVersionRange
    if (-not [string]::IsNullOrWhiteSpace([string]$_.notes)) {
        $_.notes = ([string]$_.notes) -replace 'bundle\s+\d+\.\d+\.\d+', "bundle $nextPackageVersion"
    }
}
$compatibilityMatrix | ConvertTo-Json -Depth 10 | Set-Content -Path $compatibilityMatrixPath

$rustSupport.upstreamPackage.version = $nextPackageVersion
$rustSupport.plannedFamilies | ForEach-Object {
    $_.targetUpstreamPackageVersion = $nextPackageVersion
}
if ($rustSupport.schemas) {
    foreach ($property in $rustSupport.schemas.PSObject.Properties) {
        $property.Value = $nextSchemaVersion
    }
}
$rustSupport | ConvertTo-Json -Depth 10 | Set-Content -Path $rustSupportPath

Write-Host 'Updated governed version metadata successfully.'
