[CmdletBinding()]
param(
    [string[]]$Surface = @('all'),
    [string]$OutputDirectory = ''
)

$ErrorActionPreference = 'Stop'

$repoRoot = Split-Path -Parent $PSScriptRoot
$packageReadmePath = Join-Path $repoRoot 'PACKAGE_README.md'

function Get-WrapperSurfaceMetadata {
    return [ordered]@{
        'elegy-memory' = @{
            Surface = 'elegy-memory'
            AssetPrefix = 'elegy-memory-wrapper'
            SurfaceRoot = 'src/Elegy-memory'
        }
        'elegy-mcp' = @{
            Surface = 'elegy-mcp'
            AssetPrefix = 'elegy-mcp-wrapper'
            SurfaceRoot = 'src/Elegy-mcp'
        }
        'elegy-planning' = @{
            Surface = 'elegy-planning'
            AssetPrefix = 'elegy-planning-wrapper'
            SurfaceRoot = 'src/Elegy-planning'
        }
        'elegy-skills' = @{
            Surface = 'elegy-skills'
            AssetPrefix = 'elegy-skills-wrapper'
            SurfaceRoot = 'src/Elegy-skills'
        }
        'elegy-configuration' = @{
            Surface = 'elegy-configuration'
            AssetPrefix = 'elegy-configuration-wrapper'
            SurfaceRoot = 'src/Elegy-configuration'
        }
        'elegy-documentation' = @{
            Surface = 'elegy-documentation'
            AssetPrefix = 'elegy-documentation-wrapper'
            SurfaceRoot = 'src/Elegy-documentation'
        }
    }
}

function Expand-SurfaceSelectors {
    param(
        [string[]]$Selectors
    )

    $expanded = [System.Collections.Generic.List[string]]::new()
    foreach ($selector in @($Selectors)) {
        foreach ($entry in @(([string]$selector) -split ',')) {
            $trimmedEntry = $entry.Trim()
            if ([string]::IsNullOrWhiteSpace($trimmedEntry)) {
                continue
            }

            $expanded.Add($trimmedEntry) | Out-Null
        }
    }

    return @($expanded)
}

function Resolve-WrapperSurfaces {
    param(
        [string[]]$RequestedSurfaces
    )

    $expandedSurfaces = Expand-SurfaceSelectors -Selectors $RequestedSurfaces
    $surfaceMetadata = Get-WrapperSurfaceMetadata
    if ($expandedSurfaces -contains 'all') {
        return @($surfaceMetadata.Keys)
    }

    $resolved = [System.Collections.Generic.List[string]]::new()
    foreach ($surfaceName in $expandedSurfaces) {
        if ($resolved.Contains($surfaceName)) {
            continue
        }

        if (-not $surfaceMetadata.Contains($surfaceName)) {
            throw "Unsupported wrapper surface selector: $surfaceName"
        }

        $resolved.Add($surfaceName) | Out-Null
    }

    return @($resolved)
}

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

if (-not (Test-Path $packageReadmePath)) {
    throw "Missing package README source: $packageReadmePath"
}

$surfaceMetadata = Get-WrapperSurfaceMetadata
$resolvedSurfaces = Resolve-WrapperSurfaces -RequestedSurfaces $Surface
$bundleVersion = Get-BundleVersion -RepositoryRoot $repoRoot
$installerScriptPath = Join-Path $repoRoot 'scripts/install-distribution.ps1'

if (-not (Test-Path $installerScriptPath)) {
    throw "Missing bundled installer source: $installerScriptPath"
}

New-Item -ItemType Directory -Path $OutputDirectory -Force | Out-Null

$packagedArchives = [System.Collections.Generic.List[object]]::new()
foreach ($surfaceName in $resolvedSurfaces) {
    $metadata = $surfaceMetadata[$surfaceName]
    $surfaceRoot = Join-Path $repoRoot $metadata.SurfaceRoot
    if (-not (Test-Path $surfaceRoot)) {
        throw "Wrapper surface root was not found: $surfaceRoot"
    }

    $assetBaseName = "$($metadata.AssetPrefix)-$bundleVersion"
    $stagingDirectory = Join-Path $OutputDirectory $assetBaseName
    $archivePath = Join-Path $OutputDirectory "$assetBaseName.zip"

    if (Test-Path $stagingDirectory) {
        Remove-Item -Path $stagingDirectory -Recurse -Force
    }

    if (Test-Path $archivePath) {
        Remove-Item -Path $archivePath -Force
    }

    New-Item -ItemType Directory -Path $stagingDirectory -Force | Out-Null
    Copy-Item -Path (Join-Path $surfaceRoot '*') -Destination $stagingDirectory -Recurse -Force
    Copy-Item -Path $packageReadmePath -Destination (Join-Path $stagingDirectory 'README.md') -Force

    $stagedScriptsPath = Join-Path $stagingDirectory 'scripts'
    New-Item -ItemType Directory -Path $stagedScriptsPath -Force | Out-Null
    Copy-Item -Path $installerScriptPath -Destination (Join-Path $stagedScriptsPath 'install-distribution.ps1') -Force

    Compress-Archive -Path (Join-Path $stagingDirectory '*') -DestinationPath $archivePath -CompressionLevel Optimal
    Remove-Item -Path $stagingDirectory -Recurse -Force

    $packagedArchives.Add([pscustomobject]@{
        Surface = $surfaceName
        ArchivePath = $archivePath
    }) | Out-Null
}

Write-Host 'Packaged wrapper archives.'
Write-Host " - bundle version: $bundleVersion"
foreach ($report in $packagedArchives) {
    Write-Host " - surface: $($report.Surface)"
    Write-Host "   archive: $($report.ArchivePath)"
}
