[CmdletBinding()]
param(
    [string]$OutputDirectory = ''
)

$ErrorActionPreference = 'Stop'

$repoRoot = Split-Path -Parent $PSScriptRoot

function Get-BundleVersion {
    param(
        [string]$RepositoryRoot
    )

    $versionPolicyPath = Join-Path $RepositoryRoot 'governance/version-policy.json'
    if (-not (Test-Path $versionPolicyPath)) {
        throw "Missing version policy: $versionPolicyPath"
    }

    $versionPolicy = Get-Content -Raw -Path $versionPolicyPath | ConvertFrom-Json
    if ([string]::IsNullOrWhiteSpace($versionPolicy.bundleVersion)) {
        throw 'Version policy did not include bundleVersion.'
    }

    return $versionPolicy.bundleVersion
}

if ([string]::IsNullOrWhiteSpace($OutputDirectory)) {
    $OutputDirectory = Join-Path $repoRoot 'artifacts/distribution'
}

$installerScriptPath = Join-Path $repoRoot 'scripts/install-distribution.ps1'
if (-not (Test-Path $installerScriptPath)) {
    throw "Missing installer source: $installerScriptPath"
}

$bundleVersion = Get-BundleVersion -RepositoryRoot $repoRoot

New-Item -ItemType Directory -Path $OutputDirectory -Force | Out-Null

$assetBaseName = "elegy-installer-$bundleVersion"
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

Compress-Archive -Path (Join-Path $stagingDirectory '*') -DestinationPath $archivePath -CompressionLevel Optimal
Remove-Item -Path $stagingDirectory -Recurse -Force

Write-Host "Packaged installer archive: $archivePath"
Write-Host " - bundle version: $bundleVersion"