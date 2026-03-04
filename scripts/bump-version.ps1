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
$propsPath = Join-Path $repoRoot 'Directory.Build.props'
$schemaPath = Join-Path $repoRoot 'schemas\schema-version.json'
$semVerRegex = '^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?(?:\+[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?$'

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

if (-not (Test-Path $propsPath)) {
    throw "Missing file: $propsPath"
}

if (-not (Test-Path $schemaPath)) {
    throw "Missing file: $schemaPath"
}

[xml]$propsXml = Get-Content -Raw -Path $propsPath
$versionPrefixNode = $propsXml.SelectSingleNode('//Project/PropertyGroup/VersionPrefix')
if (-not $versionPrefixNode) {
    throw "Directory.Build.props must include <VersionPrefix>."
}
$currentPackageVersion = [string]$versionPrefixNode.InnerText
Assert-SemVer -Value $currentPackageVersion -Name 'Current package version'

$schemaJson = Get-Content -Raw -Path $schemaPath | ConvertFrom-Json
if (-not $schemaJson.schemaVersion) {
    throw "schemas/schema-version.json must include schemaVersion."
}
$currentSchemaVersion = [string]$schemaJson.schemaVersion
Assert-SemVer -Value $currentSchemaVersion -Name 'Current schema version'

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
    throw "Schema major bump detected ($currentSchemaVersion -> $nextSchemaVersion). Package major must also increase ($currentPackageVersion -> $nextPackageVersion is invalid)."
}

Write-Host "Current package version: $currentPackageVersion"
Write-Host "Next package version:    $nextPackageVersion"
Write-Host "Current schema version:  $currentSchemaVersion"
Write-Host "Next schema version:     $nextSchemaVersion"

if ($DryRun) {
    Write-Host 'Dry run enabled. No files were changed.'
    exit 0
}

$versionPrefixNode.InnerText = $nextPackageVersion
$versionNode = $propsXml.SelectSingleNode('//Project/PropertyGroup/Version')
if ($versionNode) {
    $versionNode.InnerText = '$(VersionPrefix)'
}
$packageVersionNode = $propsXml.SelectSingleNode('//Project/PropertyGroup/PackageVersion')
if ($packageVersionNode) {
    $packageVersionNode.InnerText = '$(Version)'
}
$propsXml.Save($propsPath)

$schemaJson.schemaVersion = $nextSchemaVersion
$schemaJson | ConvertTo-Json -Depth 10 | Set-Content -Path $schemaPath

Write-Host 'Updated version files successfully.'
