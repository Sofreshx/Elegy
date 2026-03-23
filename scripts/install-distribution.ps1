[CmdletBinding()]
param(
    [string]$Destination = '',
    [string]$Tag = '',
    [string]$Repository = 'Sofreshx/Elegy',
    [ValidateSet('elegy-cli', 'elegy-memory', 'elegy-mcp', 'elegy-skills', 'all')]
    [string[]]$CliSurfaces = @('elegy-cli'),
    [switch]$Force
)

$ErrorActionPreference = 'Stop'

function Get-ReleaseMetadata {
    param(
        [string]$RepositoryName,
        [string]$ReleaseTag
    )

    $headers = @{
        Accept = 'application/vnd.github+json'
        'User-Agent' = 'ElegyDistributionInstaller'
    }

    if ([string]::IsNullOrWhiteSpace($ReleaseTag)) {
        $releaseUri = "https://api.github.com/repos/$RepositoryName/releases/latest"
    }
    else {
        $escapedTag = [System.Uri]::EscapeDataString($ReleaseTag)
        $releaseUri = "https://api.github.com/repos/$RepositoryName/releases/tags/$escapedTag"
    }

    return Invoke-RestMethod -Headers $headers -Uri $releaseUri
}

function Get-PublishedCliTargets {
    return @{
        Windows = @{
            X64 = 'x86_64-pc-windows-msvc'
        }
        MacOS = @{
            Arm64 = 'aarch64-apple-darwin'
        }
        Linux = @{
            X64 = 'x86_64-unknown-linux-gnu'
        }
    }
}

function Get-CliSurfaceMetadata {
    return [ordered]@{
        'elegy-cli' = @{
            Surface = 'elegy-cli'
            AssetPrefix = 'elegy-cli'
            Binary = 'elegy'
        }
        'elegy-memory' = @{
            Surface = 'elegy-memory'
            AssetPrefix = 'elegy-memory'
            Binary = 'elegy-memory'
        }
        'elegy-mcp' = @{
            Surface = 'elegy-mcp'
            AssetPrefix = 'elegy-mcp'
            Binary = 'elegy-mcp'
        }
        'elegy-skills' = @{
            Surface = 'elegy-skills'
            AssetPrefix = 'elegy-skills'
            Binary = 'elegy-skills'
        }
    }
}

function Resolve-CliSurfaces {
    param(
        [string[]]$RequestedSurfaces
    )

    $surfaceMetadata = Get-CliSurfaceMetadata
    if ($RequestedSurfaces -contains 'all') {
        return @($surfaceMetadata.Keys)
    }

    $resolved = [System.Collections.Generic.List[string]]::new()
    foreach ($surface in $RequestedSurfaces) {
        if ($resolved.Contains($surface)) {
            continue
        }

        if (-not $surfaceMetadata.Contains($surface)) {
            throw "Unsupported CLI surface selector: $surface"
        }

        $resolved.Add($surface) | Out-Null
    }

    return @($resolved)
}

function Get-HostPublishedTarget {
    $architecture = [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture.ToString()
    $publishedTargets = Get-PublishedCliTargets

    if ($IsWindows) {
        switch ($architecture) {
            'X64' { return $publishedTargets.Windows.X64 }
            default {
                throw "Unsupported Windows architecture: $architecture. Published Elegy CLI assets currently support only X64 hosts ($($publishedTargets.Windows.X64))."
            }
        }
    }

    if ($IsMacOS) {
        switch ($architecture) {
            'Arm64' { return $publishedTargets.MacOS.Arm64 }
            default {
                throw "Unsupported macOS architecture: $architecture. Published Elegy CLI assets currently support only Arm64 hosts ($($publishedTargets.MacOS.Arm64))."
            }
        }
    }

    if ($IsLinux) {
        switch ($architecture) {
            'X64' { return $publishedTargets.Linux.X64 }
            default {
                throw "Unsupported Linux architecture: $architecture. Published Elegy CLI assets currently support only X64 hosts ($($publishedTargets.Linux.X64))."
            }
        }
    }

    $supportedTargets = @(
        $publishedTargets.Windows.X64,
        $publishedTargets.MacOS.Arm64,
        $publishedTargets.Linux.X64
    ) -join ', '
    throw "Unable to determine a supported host operating system for Elegy CLI assets. Published targets: $supportedTargets"
}

function Find-ReleaseAsset {
    param(
        [object[]]$Assets,
        [string[]]$Patterns,
        [string]$Description
    )

    foreach ($pattern in $Patterns) {
        $asset = $Assets | Where-Object { $_.name -like $pattern } | Sort-Object name | Select-Object -First 1
        if ($null -ne $asset) {
            return $asset
        }
    }

    throw "Unable to locate a $Description asset matching patterns: $($Patterns -join ', ')"
}

function Initialize-DestinationDirectory {
    param(
        [string]$Path,
        [switch]$AllowReplace
    )

    if (Test-Path $Path) {
        if (-not $AllowReplace) {
            throw "Destination path already exists: $Path. Re-run with -Force to replace it."
        }

        Remove-Item -Path $Path -Recurse -Force
    }

    New-Item -ItemType Directory -Path $Path -Force | Out-Null
}

function Get-ExecutableFileName {
    param(
        [string]$BinaryName,
        [string]$TargetTriple
    )

    if ($TargetTriple -match 'windows') {
        return "$BinaryName.exe"
    }

    return $BinaryName
}

if ([string]::IsNullOrWhiteSpace($Destination)) {
    $Destination = Join-Path (Get-Location) '.elegy'
}

$surfaceMetadata = Get-CliSurfaceMetadata
$resolvedCliSurfaces = Resolve-CliSurfaces -RequestedSurfaces $CliSurfaces
$release = Get-ReleaseMetadata -RepositoryName $Repository -ReleaseTag $Tag
$resolvedTag = $release.tag_name

if ([string]::IsNullOrWhiteSpace($resolvedTag)) {
    throw 'Resolved GitHub release metadata did not include a tag name.'
}

$resolvedTarget = Get-HostPublishedTarget
$contractsAsset = Find-ReleaseAsset -Assets $release.assets -Patterns @('elegy-contracts-*.zip') -Description 'contracts bundle'

$downloadRoot = Join-Path $Destination 'downloads'
$contractsPath = Join-Path $Destination 'contracts'
$binRoot = Join-Path $Destination 'bin'
$legacyCliPath = Join-Path $Destination 'cli'

Initialize-DestinationDirectory -Path $Destination -AllowReplace:$Force
Initialize-DestinationDirectory -Path $downloadRoot -AllowReplace:$true
Initialize-DestinationDirectory -Path $contractsPath -AllowReplace:$true
Initialize-DestinationDirectory -Path $binRoot -AllowReplace:$true

$contractsArchivePath = Join-Path $downloadRoot $contractsAsset.name

Invoke-WebRequest -Uri $contractsAsset.browser_download_url -OutFile $contractsArchivePath

Expand-Archive -Path $contractsArchivePath -DestinationPath $contractsPath -Force

$installedCliReports = [System.Collections.Generic.List[object]]::new()
foreach ($surface in $resolvedCliSurfaces) {
    $metadata = $surfaceMetadata[$surface]
    $cliAsset = Find-ReleaseAsset -Assets $release.assets -Patterns @("$($metadata.AssetPrefix)-*-$resolvedTarget.zip") -Description "$surface CLI archive"
    $cliArchivePath = Join-Path $downloadRoot $cliAsset.name
    $surfacePath = Join-Path $binRoot $surface

    Initialize-DestinationDirectory -Path $surfacePath -AllowReplace:$true
    Invoke-WebRequest -Uri $cliAsset.browser_download_url -OutFile $cliArchivePath
    Expand-Archive -Path $cliArchivePath -DestinationPath $surfacePath -Force

    $executableName = Get-ExecutableFileName -BinaryName $metadata.Binary -TargetTriple $resolvedTarget
    $executablePath = Join-Path $surfacePath $executableName
    if (-not (Test-Path $executablePath)) {
        throw "Installed CLI executable was not found at $executablePath"
    }

    if ($surface -eq 'elegy-cli') {
        Initialize-DestinationDirectory -Path $legacyCliPath -AllowReplace:$true
        Copy-Item -Path (Join-Path $surfacePath '*') -Destination $legacyCliPath -Recurse -Force
    }

    $installedCliReports.Add([pscustomobject]@{
        Surface = $surface
        Asset = $cliAsset.name
        InstallPath = $surfacePath
        ExecutablePath = $executablePath
    }) | Out-Null
}

Write-Host 'Installed Elegy distribution assets.'
Write-Host " - repository: $Repository"
Write-Host " - release tag: $resolvedTag"
Write-Host " - contracts asset: $($contractsAsset.name)"
Write-Host " - contracts path: $contractsPath"
foreach ($report in $installedCliReports) {
    Write-Host " - CLI surface: $($report.Surface)"
    Write-Host "   asset: $($report.Asset)"
    Write-Host "   path: $($report.InstallPath)"
    Write-Host "   executable path: $($report.ExecutablePath)"
}
if ($resolvedCliSurfaces -contains 'elegy-cli') {
    Write-Host " - compatibility cli path: $legacyCliPath"
}