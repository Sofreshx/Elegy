[CmdletBinding()]
param(
    [string]$Destination = '',
    [string]$Tag = '',
    [string]$Repository = 'Sofreshx/Elegy',
    [ValidateSet('elegy-cli', 'elegy-memory', 'elegy-mcp', 'elegy-skills', 'all')]
    [string[]]$CliSurfaces = @('elegy-cli'),
    [ValidateSet('elegy-memory', 'elegy-mcp', 'elegy-skills', 'all')]
    [string[]]$WrapperSurfaces = @(),
    [string]$LocalArtifactsRoot = '',
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

function Get-WrapperSurfaceMetadata {
    return [ordered]@{
        'elegy-memory' = @{
            Surface = 'elegy-memory'
            AssetPrefix = 'elegy-memory-wrapper'
            Installer = 'install.ps1'
            SkillBridge = 'skills/elegy-memory/SKILL.md'
        }
        'elegy-mcp' = @{
            Surface = 'elegy-mcp'
            AssetPrefix = 'elegy-mcp-wrapper'
            Installer = 'install.ps1'
            SkillBridge = 'skills/elegy-mcp/SKILL.md'
        }
        'elegy-skills' = @{
            Surface = 'elegy-skills'
            AssetPrefix = 'elegy-skills-wrapper'
            Installer = 'install.ps1'
            SkillBridge = 'skills/elegy-skills/SKILL.md'
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

function Resolve-WrapperSurfaces {
    param(
        [string[]]$RequestedSurfaces
    )

    if ($null -eq $RequestedSurfaces -or $RequestedSurfaces.Count -eq 0) {
        return @()
    }

    $surfaceMetadata = Get-WrapperSurfaceMetadata
    if ($RequestedSurfaces -contains 'all') {
        return @($surfaceMetadata.Keys)
    }

    $resolved = [System.Collections.Generic.List[string]]::new()
    foreach ($surface in $RequestedSurfaces) {
        if ($resolved.Contains($surface)) {
            continue
        }

        if (-not $surfaceMetadata.Contains($surface)) {
            throw "Unsupported wrapper surface selector: $surface"
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

function Find-LocalArchive {
    param(
        [string]$ArtifactsRoot,
        [string[]]$Patterns,
        [string]$Description
    )

    $matches = @(
        foreach ($pattern in $Patterns) {
            Get-ChildItem -Path $ArtifactsRoot -Filter $pattern -File -ErrorAction SilentlyContinue
        }
    )

    $uniqueMatches = @(
        $matches |
            Group-Object FullName |
            ForEach-Object { $_.Group[0] } |
            Sort-Object Name
    )

    if ($uniqueMatches.Count -eq 1) {
        return $uniqueMatches[0]
    }

    if ($uniqueMatches.Count -gt 1) {
        $matchNames = $uniqueMatches | ForEach-Object { $_.Name }
        throw "Ambiguous local $Description archives in $ArtifactsRoot matching patterns: $($Patterns -join ', '). Matches: $($matchNames -join ', '). Provide a local artifacts root with exactly one matching archive per required asset."
    }

    throw "Unable to locate a local $Description archive in $ArtifactsRoot matching patterns: $($Patterns -join ', ')"
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

function Stage-ArchiveFromSource {
    param(
        [string]$DestinationPath,
        [string]$SourcePath,
        [string]$SourceUri
    )

    if (-not [string]::IsNullOrWhiteSpace($SourcePath)) {
        Copy-Item -Path $SourcePath -Destination $DestinationPath -Force
        return
    }

    Invoke-WebRequest -Uri $SourceUri -OutFile $DestinationPath
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
$wrapperMetadata = Get-WrapperSurfaceMetadata
$resolvedCliSurfaces = Resolve-CliSurfaces -RequestedSurfaces $CliSurfaces
$resolvedWrapperSurfaces = Resolve-WrapperSurfaces -RequestedSurfaces $WrapperSurfaces
$release = $null
$resolvedTag = ''
$resolvedLocalArtifactsRoot = ''

if ([string]::IsNullOrWhiteSpace($LocalArtifactsRoot)) {
    $release = Get-ReleaseMetadata -RepositoryName $Repository -ReleaseTag $Tag
    $resolvedTag = $release.tag_name

    if ([string]::IsNullOrWhiteSpace($resolvedTag)) {
        throw 'Resolved GitHub release metadata did not include a tag name.'
    }
}
else {
    $resolvedLocalArtifactsRoot = (Resolve-Path -Path $LocalArtifactsRoot).Path
    $resolvedTag = 'local-artifacts'
}

$resolvedTarget = Get-HostPublishedTarget
$contractsAsset = if ([string]::IsNullOrWhiteSpace($resolvedLocalArtifactsRoot)) {
    Find-ReleaseAsset -Assets $release.assets -Patterns @('elegy-contracts-*.zip') -Description 'contracts bundle'
}
else {
    Find-LocalArchive -ArtifactsRoot $resolvedLocalArtifactsRoot -Patterns @('elegy-contracts-*.zip') -Description 'contracts bundle'
}

$downloadRoot = Join-Path $Destination 'downloads'
$contractsPath = Join-Path $Destination 'contracts'
$binRoot = Join-Path $Destination 'bin'
$wrapperRoot = Join-Path $Destination 'wrappers'
$legacyCliPath = Join-Path $Destination 'cli'

Initialize-DestinationDirectory -Path $Destination -AllowReplace:$Force
Initialize-DestinationDirectory -Path $downloadRoot -AllowReplace:$true
Initialize-DestinationDirectory -Path $contractsPath -AllowReplace:$true
Initialize-DestinationDirectory -Path $binRoot -AllowReplace:$true
if ($resolvedWrapperSurfaces.Count -gt 0) {
    Initialize-DestinationDirectory -Path $wrapperRoot -AllowReplace:$true
}

$contractsArchivePath = Join-Path $downloadRoot $contractsAsset.name

if ([string]::IsNullOrWhiteSpace($resolvedLocalArtifactsRoot)) {
    Stage-ArchiveFromSource -DestinationPath $contractsArchivePath -SourceUri $contractsAsset.browser_download_url
}
else {
    Stage-ArchiveFromSource -DestinationPath $contractsArchivePath -SourcePath $contractsAsset.FullName
}

Expand-Archive -Path $contractsArchivePath -DestinationPath $contractsPath -Force

$installedCliReports = [System.Collections.Generic.List[object]]::new()
foreach ($surface in $resolvedCliSurfaces) {
    $metadata = $surfaceMetadata[$surface]
    $cliAsset = if ([string]::IsNullOrWhiteSpace($resolvedLocalArtifactsRoot)) {
        Find-ReleaseAsset -Assets $release.assets -Patterns @("$($metadata.AssetPrefix)-*-$resolvedTarget.zip") -Description "$surface CLI archive"
    }
    else {
        Find-LocalArchive -ArtifactsRoot $resolvedLocalArtifactsRoot -Patterns @("$($metadata.AssetPrefix)-*-$resolvedTarget.zip") -Description "$surface CLI archive"
    }
    $cliArchivePath = Join-Path $downloadRoot $cliAsset.name
    $surfacePath = Join-Path $binRoot $surface

    Initialize-DestinationDirectory -Path $surfacePath -AllowReplace:$true
    if ([string]::IsNullOrWhiteSpace($resolvedLocalArtifactsRoot)) {
        Stage-ArchiveFromSource -DestinationPath $cliArchivePath -SourceUri $cliAsset.browser_download_url
    }
    else {
        Stage-ArchiveFromSource -DestinationPath $cliArchivePath -SourcePath $cliAsset.FullName
    }
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

$installedWrapperReports = [System.Collections.Generic.List[object]]::new()
foreach ($surface in $resolvedWrapperSurfaces) {
    $metadata = $wrapperMetadata[$surface]
    $wrapperAsset = if ([string]::IsNullOrWhiteSpace($resolvedLocalArtifactsRoot)) {
        Find-ReleaseAsset -Assets $release.assets -Patterns @("$($metadata.AssetPrefix)-*.zip") -Description "$surface wrapper archive"
    }
    else {
        Find-LocalArchive -ArtifactsRoot $resolvedLocalArtifactsRoot -Patterns @("$($metadata.AssetPrefix)-*.zip") -Description "$surface wrapper archive"
    }
    $wrapperArchivePath = Join-Path $downloadRoot $wrapperAsset.name
    $surfacePath = Join-Path $wrapperRoot $surface

    Initialize-DestinationDirectory -Path $surfacePath -AllowReplace:$true
    if ([string]::IsNullOrWhiteSpace($resolvedLocalArtifactsRoot)) {
        Stage-ArchiveFromSource -DestinationPath $wrapperArchivePath -SourceUri $wrapperAsset.browser_download_url
    }
    else {
        Stage-ArchiveFromSource -DestinationPath $wrapperArchivePath -SourcePath $wrapperAsset.FullName
    }
    Expand-Archive -Path $wrapperArchivePath -DestinationPath $surfacePath -Force

    $installerPath = Join-Path $surfacePath $metadata.Installer
    $skillBridgePath = Join-Path $surfacePath $metadata.SkillBridge
    if (-not (Test-Path $installerPath)) {
        throw "Installed wrapper installer was not found at $installerPath"
    }

    if (-not (Test-Path $skillBridgePath)) {
        throw "Installed wrapper skill bridge was not found at $skillBridgePath"
    }

    $installedWrapperReports.Add([pscustomobject]@{
        Surface = $surface
        Asset = $wrapperAsset.name
        InstallPath = $surfacePath
        InstallerPath = $installerPath
        SkillBridgePath = $skillBridgePath
    }) | Out-Null
}

Write-Host 'Installed Elegy distribution assets.'
if ([string]::IsNullOrWhiteSpace($resolvedLocalArtifactsRoot)) {
    Write-Host " - repository: $Repository"
}
else {
    Write-Host " - local artifacts root: $resolvedLocalArtifactsRoot"
}
Write-Host " - release tag: $resolvedTag"
Write-Host " - contracts asset: $($contractsAsset.name)"
Write-Host " - contracts path: $contractsPath"
foreach ($report in $installedCliReports) {
    Write-Host " - CLI surface: $($report.Surface)"
    Write-Host "   asset: $($report.Asset)"
    Write-Host "   path: $($report.InstallPath)"
    Write-Host "   executable path: $($report.ExecutablePath)"
}
foreach ($report in $installedWrapperReports) {
    Write-Host " - wrapper surface: $($report.Surface)"
    Write-Host "   asset: $($report.Asset)"
    Write-Host "   path: $($report.InstallPath)"
    Write-Host "   installer path: $($report.InstallerPath)"
    Write-Host "   skill bridge path: $($report.SkillBridgePath)"
}
if ($resolvedCliSurfaces -contains 'elegy-cli') {
    Write-Host " - compatibility cli path: $legacyCliPath"
}