[CmdletBinding()]
param(
    [string]$Destination = '',
    [string]$Tag = '',
    [string]$Repository = 'Sofreshx/Elegy',
    [string[]]$CliSurfaces = @('elegy-cli'),
    [string[]]$WrapperSurfaces = @(),
    [string]$LocalArtifactsRoot = '',
    [switch]$Force,
    [switch]$AddToPath,
    [switch]$NoCommandShims
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
        'elegy-planning' = @{
            Surface = 'elegy-planning'
            AssetPrefix = 'elegy-planning'
            Binary = 'elegy-planning'
        }
        'elegy-skills' = @{
            Surface = 'elegy-skills'
            AssetPrefix = 'elegy-skills'
            Binary = 'elegy-skills'
        }
        'elegy-configuration' = @{
            Surface = 'elegy-configuration'
            AssetPrefix = 'elegy-configuration'
            Binary = 'elegy-configuration'
        }
        'elegy-documentation' = @{
            Surface = 'elegy-documentation'
            AssetPrefix = 'elegy-documentation'
            Binary = 'elegy-documentation'
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
        'elegy-planning' = @{
            Surface = 'elegy-planning'
            AssetPrefix = 'elegy-planning-wrapper'
            Installer = 'install.ps1'
            SkillBridge = 'skills/elegy-planning/SKILL.md'
        }
        'elegy-skills' = @{
            Surface = 'elegy-skills'
            AssetPrefix = 'elegy-skills-wrapper'
            Installer = 'install.ps1'
            SkillBridge = 'skills/elegy-skills/SKILL.md'
        }
        'elegy-configuration' = @{
            Surface = 'elegy-configuration'
            AssetPrefix = 'elegy-configuration-wrapper'
            Installer = 'install.ps1'
            SkillBridge = 'skills/elegy-configuration/SKILL.md'
        }
        'elegy-documentation' = @{
            Surface = 'elegy-documentation'
            AssetPrefix = 'elegy-documentation-wrapper'
            Installer = 'install.ps1'
            SkillBridge = 'skills/elegy-documentation/SKILL.md'
        }
        'elegy-obsidian' = @{
            Surface = 'elegy-obsidian'
            AssetPrefix = 'elegy-obsidian-wrapper'
            Installer = 'install.ps1'
            SkillBridge = 'skills/elegy-obsidian/SKILL.md'
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

function Resolve-CliSurfaces {
    param(
        [string[]]$RequestedSurfaces
    )

    $expandedSurfaces = Expand-SurfaceSelectors -Selectors $RequestedSurfaces
    $surfaceMetadata = Get-CliSurfaceMetadata
    if ($expandedSurfaces -contains 'all') {
        return @($surfaceMetadata.Keys)
    }

    $resolved = [System.Collections.Generic.List[string]]::new()
    foreach ($surface in $expandedSurfaces) {
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

    $expandedSurfaces = Expand-SurfaceSelectors -Selectors $RequestedSurfaces
    $surfaceMetadata = Get-WrapperSurfaceMetadata
    if ($expandedSurfaces -contains 'all') {
        return @($surfaceMetadata.Keys)
    }

    $resolved = [System.Collections.Generic.List[string]]::new()
    foreach ($surface in $expandedSurfaces) {
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

function Resolve-ReleaseAssetByPattern {
    param(
        [object[]]$Assets,
        [string[]]$Patterns,
        [string]$Description
    )

    $candidateAssets = [System.Collections.Generic.List[object]]::new()
    foreach ($pattern in $Patterns) {
        foreach ($asset in @($Assets | Where-Object { $_.name -like $pattern })) {
            $candidateAssets.Add($asset) | Out-Null
        }
    }

    $resolvedAssets = @(
        $candidateAssets |
            Group-Object name |
            ForEach-Object { $_.Group[0] } |
            Sort-Object name
    )

    if ($resolvedAssets.Count -eq 1) {
        return [pscustomobject]@{
            FileName = [string]$resolvedAssets[0].name
            SourcePath = ''
            SourceUri = [string]$resolvedAssets[0].browser_download_url
            PublishedSize = [int64]$resolvedAssets[0].size
        }
    }

    if ($resolvedAssets.Count -gt 1) {
        $matchNames = $resolvedAssets | ForEach-Object { $_.name }
        throw "Ambiguous release $Description assets. Patterns: $($Patterns -join ', '). Matches: $($matchNames -join ', ')."
    }

    throw "Unable to locate a release $Description asset matching patterns: $($Patterns -join ', ')"
}

function Resolve-ReleaseAssetByName {
    param(
        [object[]]$Assets,
        [string]$FileName,
        [string]$Description
    )

    $candidateAssets = @(
        $Assets |
            Where-Object { $_.name -eq $FileName } |
            Sort-Object name
    )

    if ($candidateAssets.Count -eq 1) {
        return [pscustomobject]@{
            FileName = [string]$candidateAssets[0].name
            SourcePath = ''
            SourceUri = [string]$candidateAssets[0].browser_download_url
            PublishedSize = [int64]$candidateAssets[0].size
        }
    }

    if ($candidateAssets.Count -gt 1) {
        throw "Ambiguous release $Description assets named $FileName."
    }

    throw "Unable to locate a release $Description asset named $FileName"
}

function Resolve-LocalAssetByPattern {
    param(
        [string]$ArtifactsRoot,
        [string[]]$Patterns,
        [string]$Description
    )

    $candidateAssets = [System.Collections.Generic.List[object]]::new()
    foreach ($pattern in $Patterns) {
        foreach ($asset in @(Get-ChildItem -Path $ArtifactsRoot -Filter $pattern -File -ErrorAction SilentlyContinue)) {
            $candidateAssets.Add($asset) | Out-Null
        }
    }

    $resolvedAssets = @(
        $candidateAssets |
            Group-Object FullName |
            ForEach-Object { $_.Group[0] } |
            Sort-Object Name
    )

    if ($resolvedAssets.Count -eq 1) {
        return [pscustomobject]@{
            FileName = [string]$resolvedAssets[0].Name
            SourcePath = [string]$resolvedAssets[0].FullName
            SourceUri = ''
            PublishedSize = [int64]$resolvedAssets[0].Length
        }
    }

    if ($resolvedAssets.Count -gt 1) {
        $matchNames = $resolvedAssets | ForEach-Object { $_.Name }
        throw "Ambiguous local $Description assets in $ArtifactsRoot matching patterns: $($Patterns -join ', '). Matches: $($matchNames -join ', ')."
    }

    throw "Unable to locate a local $Description asset in $ArtifactsRoot matching patterns: $($Patterns -join ', ')"
}

function Resolve-LocalAssetByName {
    param(
        [string]$ArtifactsRoot,
        [string]$FileName,
        [string]$Description
    )

    $sourcePath = Join-Path $ArtifactsRoot $FileName
    if (-not (Test-Path $sourcePath)) {
        throw "Unable to locate a local $Description asset named $FileName in $ArtifactsRoot"
    }

    $fileInfo = Get-Item -Path $sourcePath
    if ($fileInfo.PSIsContainer) {
        throw "Expected a file for local $Description asset $FileName but found a directory at $sourcePath"
    }

    return [pscustomobject]@{
        FileName = [string]$fileInfo.Name
        SourcePath = [string]$fileInfo.FullName
        SourceUri = ''
        PublishedSize = [int64]$fileInfo.Length
    }
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

function Copy-FileFromSource {
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

function Restore-UnixExecutablePermission {
    param(
        [string]$ExecutablePath
    )

    if ($IsWindows) {
        return
    }

    & chmod +x $ExecutablePath
    if ($LASTEXITCODE -ne 0) {
        throw "Failed to restore executable permissions for $ExecutablePath"
    }
}

function Read-JsonFile {
    param(
        [string]$Path
    )

    return Get-Content -Raw -Path $Path | ConvertFrom-Json
}

function ConvertTo-StringArray {
    param(
        [object]$Value
    )

    $result = [System.Collections.Generic.List[string]]::new()
    foreach ($item in @($Value)) {
        $stringValue = [string]$item
        if (-not [string]::IsNullOrWhiteSpace($stringValue)) {
            $result.Add($stringValue) | Out-Null
        }
    }

    return @($result)
}

function Get-FileSha256 {
    param(
        [string]$Path
    )

    return (Get-FileHash -Path $Path -Algorithm SHA256).Hash.ToLowerInvariant()
}

function Get-NormalizedArchiveEntries {
    param(
        [string]$ArchivePath
    )

    $archive = [System.IO.Compression.ZipFile]::OpenRead($ArchivePath)
    try {
        $entryLookup = [System.Collections.Generic.HashSet[string]]::new([System.StringComparer]::OrdinalIgnoreCase)
        foreach ($entry in $archive.Entries) {
            $normalizedEntry = $entry.FullName.Replace('\\', '/').TrimStart([char[]]@('/', '.'))
            if (-not [string]::IsNullOrWhiteSpace($normalizedEntry)) {
                $entryLookup.Add($normalizedEntry) | Out-Null
            }
        }

        return $entryLookup
    }
    finally {
        $archive.Dispose()
    }
}

function Assert-ArchiveRequiredEntries {
    param(
        [string]$ArchivePath,
        [string[]]$RequiredEntries
    )

    $entryLookup = Get-NormalizedArchiveEntries -ArchivePath $ArchivePath
    $missingEntries = [System.Collections.Generic.List[string]]::new()
    foreach ($requiredEntry in @($RequiredEntries)) {
        if (-not $entryLookup.Contains($requiredEntry)) {
            $missingEntries.Add($requiredEntry) | Out-Null
        }
    }

    if ($missingEntries.Count -gt 0) {
        throw "Archive $ArchivePath is missing required entries: $($missingEntries -join ', ')"
    }
}

function New-ChecksumLookup {
    param(
        [object]$ChecksumsDocument
    )

    $lookup = [System.Collections.Generic.Dictionary[string, string]]::new([System.StringComparer]::OrdinalIgnoreCase)
    foreach ($entry in @($ChecksumsDocument.entries)) {
        $fileName = [string]$entry.fileName
        if ([string]::IsNullOrWhiteSpace($fileName)) {
            throw 'Checksums metadata included an empty fileName entry.'
        }

        if ($lookup.ContainsKey($fileName)) {
            throw "Checksums metadata included a duplicate fileName entry for $fileName"
        }

        $lookup[$fileName] = ([string]$entry.sha256).ToLowerInvariant()
    }

    return $lookup
}

function Test-OptionalFieldMatch {
    param(
        [object]$ActualValue,
        [string]$ExpectedValue
    )

    $actualString = [string]$ActualValue
    if ([string]::IsNullOrWhiteSpace($ExpectedValue)) {
        return [string]::IsNullOrWhiteSpace($actualString)
    }

    return [string]::Equals($actualString, $ExpectedValue, [System.StringComparison]::OrdinalIgnoreCase)
}

function Get-ManifestAsset {
    param(
        [object]$ManifestDocument,
        [string]$AssetKind,
        [string]$Surface,
        [string]$Target,
        [string]$Description
    )

    $selectedAssets = @(
        $ManifestDocument.assets |
            Where-Object {
                [string]::Equals([string]$_.assetKind, $AssetKind, [System.StringComparison]::OrdinalIgnoreCase) -and
                (Test-OptionalFieldMatch -ActualValue $_.surface -ExpectedValue $Surface) -and
                (Test-OptionalFieldMatch -ActualValue $_.target -ExpectedValue $Target)
            }
    )

    if ($selectedAssets.Count -eq 1) {
        return $selectedAssets[0]
    }

    if ($selectedAssets.Count -gt 1) {
        throw "Manifest metadata resolved multiple $Description entries."
    }

    throw "Manifest metadata did not include a $Description entry."
}

function Assert-StagedFileMatchesMetadata {
    param(
        [string]$FilePath,
        [object]$ManifestAsset,
        [System.Collections.Generic.Dictionary[string, string]]$ChecksumLookup,
        [int64]$PublishedSize
    )

    $fileName = [string]$ManifestAsset.fileName
    $expectedSize = [int64]$ManifestAsset.sizeBytes
    $expectedManifestHash = ([string]$ManifestAsset.sha256).ToLowerInvariant()

    if (-not $ChecksumLookup.ContainsKey($fileName)) {
        throw "Checksums metadata did not include an entry for $fileName"
    }

    if ($ChecksumLookup[$fileName] -ne $expectedManifestHash) {
        throw "Manifest and checksums metadata disagreed on the SHA-256 hash for $fileName"
    }

    if ($PublishedSize -gt 0 -and $PublishedSize -ne $expectedSize) {
        throw "Published size for $fileName was $PublishedSize bytes, but the manifest expected $expectedSize bytes"
    }

    $fileInfo = Get-Item -Path $FilePath
    if ($fileInfo.Length -ne $expectedSize) {
        throw "Staged file $fileName was $($fileInfo.Length) bytes, but the manifest expected $expectedSize bytes"
    }

    $actualHash = Get-FileSha256 -Path $FilePath
    if ($actualHash -ne $expectedManifestHash) {
        throw "SHA-256 mismatch for $fileName. Expected $expectedManifestHash but found $actualHash"
    }

    return [pscustomobject]@{
        fileName = $fileName
        sizeBytes = $expectedSize
        sha256 = $actualHash
    }
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
    $resolvedTag = [string]$release.tag_name

    if ([string]::IsNullOrWhiteSpace($resolvedTag)) {
        throw 'Resolved GitHub release metadata did not include a tag name.'
    }
}
else {
    $resolvedLocalArtifactsRoot = (Resolve-Path -Path $LocalArtifactsRoot).Path
    $resolvedTag = 'local-artifacts'
}

$resolvedTarget = Get-HostPublishedTarget
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

$manifestSource = if ([string]::IsNullOrWhiteSpace($resolvedLocalArtifactsRoot)) {
    Resolve-ReleaseAssetByPattern -Assets $release.assets -Patterns @('elegy-release-manifest-*.json') -Description 'release manifest'
}
else {
    Resolve-LocalAssetByPattern -ArtifactsRoot $resolvedLocalArtifactsRoot -Patterns @('elegy-release-manifest-*.json') -Description 'release manifest'
}

$checksumsSource = if ([string]::IsNullOrWhiteSpace($resolvedLocalArtifactsRoot)) {
    Resolve-ReleaseAssetByPattern -Assets $release.assets -Patterns @('elegy-release-checksums-*.json') -Description 'release checksums'
}
else {
    Resolve-LocalAssetByPattern -ArtifactsRoot $resolvedLocalArtifactsRoot -Patterns @('elegy-release-checksums-*.json') -Description 'release checksums'
}

$manifestPath = Join-Path $downloadRoot $manifestSource.FileName
$checksumsPath = Join-Path $downloadRoot $checksumsSource.FileName
Copy-FileFromSource -DestinationPath $manifestPath -SourcePath $manifestSource.SourcePath -SourceUri $manifestSource.SourceUri
Copy-FileFromSource -DestinationPath $checksumsPath -SourcePath $checksumsSource.SourcePath -SourceUri $checksumsSource.SourceUri

$manifestDocument = Read-JsonFile -Path $manifestPath
$checksumsDocument = Read-JsonFile -Path $checksumsPath

if ([string]$manifestDocument.documentType -ne 'elegy-release-manifest') {
    throw 'Release manifest metadata had an unexpected documentType.'
}

if ([string]$checksumsDocument.documentType -ne 'elegy-release-checksums') {
    throw 'Release checksums metadata had an unexpected documentType.'
}

if ([string]::IsNullOrWhiteSpace([string]$manifestDocument.schemaVersion)) {
    throw 'Release manifest metadata did not include schemaVersion.'
}

if ([string]::IsNullOrWhiteSpace([string]$checksumsDocument.schemaVersion)) {
    throw 'Release checksums metadata did not include schemaVersion.'
}

if ([string]::IsNullOrWhiteSpace([string]$manifestDocument.bundleVersion)) {
    throw 'Release manifest metadata did not include bundleVersion.'
}

if (-not [string]::Equals([string]$manifestDocument.tag, [string]$checksumsDocument.tag, [System.StringComparison]::Ordinal)) {
    throw 'Release manifest and checksums metadata did not agree on the resolved tag marker.'
}

if (-not [string]::Equals([string]$checksumsDocument.algorithm, 'sha256', [System.StringComparison]::OrdinalIgnoreCase)) {
    throw "Unsupported checksums algorithm: $($checksumsDocument.algorithm)"
}

if ([string]::IsNullOrWhiteSpace($resolvedLocalArtifactsRoot)) {
    if (-not [string]::Equals([string]$manifestDocument.repository, $Repository, [System.StringComparison]::OrdinalIgnoreCase)) {
        throw "Release manifest metadata targeted repository $($manifestDocument.repository), but installer expected $Repository"
    }

    if (-not [string]::Equals([string]$manifestDocument.tag, $resolvedTag, [System.StringComparison]::Ordinal)) {
        throw "Release manifest metadata targeted tag $($manifestDocument.tag), but installer resolved tag $resolvedTag"
    }
}
elseif (-not [string]::Equals([string]$manifestDocument.tag, 'local-artifacts', [System.StringComparison]::Ordinal)) {
    throw "Local artifact installs require manifest tag marker 'local-artifacts', but found $($manifestDocument.tag)"
}

$checksumLookup = New-ChecksumLookup -ChecksumsDocument $checksumsDocument
$manifestFileName = Split-Path -Leaf $manifestPath
if (-not $checksumLookup.ContainsKey($manifestFileName)) {
    throw "Checksums metadata did not include the manifest file $manifestFileName"
}

$manifestSha256 = Get-FileSha256 -Path $manifestPath
if ($checksumLookup[$manifestFileName] -ne $manifestSha256) {
    throw "Release manifest SHA-256 did not match the published checksums entry for $manifestFileName"
}

$publishedTargets = ConvertTo-StringArray -Value $manifestDocument.publishedTargets
if ($resolvedCliSurfaces.Count -gt 0 -and -not ($publishedTargets -contains $resolvedTarget)) {
    throw "Release metadata did not publish the current host target $resolvedTarget"
}

$verifiedFiles = [System.Collections.Generic.List[object]]::new()
$verifiedFiles.Add([pscustomobject]@{
    fileName = $manifestFileName
    sizeBytes = [int64](Get-Item -Path $manifestPath).Length
    sha256 = $manifestSha256
}) | Out-Null

$installedAssets = [System.Collections.Generic.List[object]]::new()

$contractsManifestAsset = Get-ManifestAsset -ManifestDocument $manifestDocument -AssetKind 'contracts-bundle' -Surface '' -Target '' -Description 'contracts bundle'
$contractsSource = if ([string]::IsNullOrWhiteSpace($resolvedLocalArtifactsRoot)) {
    Resolve-ReleaseAssetByName -Assets $release.assets -FileName ([string]$contractsManifestAsset.fileName) -Description 'contracts bundle'
}
else {
    Resolve-LocalAssetByName -ArtifactsRoot $resolvedLocalArtifactsRoot -FileName ([string]$contractsManifestAsset.fileName) -Description 'contracts bundle'
}

$contractsArchivePath = Join-Path $downloadRoot $contractsSource.FileName
Copy-FileFromSource -DestinationPath $contractsArchivePath -SourcePath $contractsSource.SourcePath -SourceUri $contractsSource.SourceUri
$verifiedContractsFile = Assert-StagedFileMatchesMetadata -FilePath $contractsArchivePath -ManifestAsset $contractsManifestAsset -ChecksumLookup $checksumLookup -PublishedSize $contractsSource.PublishedSize
$verifiedFiles.Add($verifiedContractsFile) | Out-Null
Assert-ArchiveRequiredEntries -ArchivePath $contractsArchivePath -RequiredEntries (ConvertTo-StringArray -Value $contractsManifestAsset.requiredEntries)
Expand-Archive -Path $contractsArchivePath -DestinationPath $contractsPath -Force

$installedAssets.Add([pscustomobject]@{
    assetKind = [string]$contractsManifestAsset.assetKind
    surface = $null
    target = $null
    fileName = [string]$contractsManifestAsset.fileName
    installPath = $contractsPath
    requiredEntries = ConvertTo-StringArray -Value $contractsManifestAsset.requiredEntries
    sizeBytes = [int64]$contractsManifestAsset.sizeBytes
    sha256 = [string]$contractsManifestAsset.sha256
}) | Out-Null

$installedCliReports = [System.Collections.Generic.List[object]]::new()
foreach ($surface in $resolvedCliSurfaces) {
    $metadata = $surfaceMetadata[$surface]
    $cliManifestAsset = Get-ManifestAsset -ManifestDocument $manifestDocument -AssetKind 'cli' -Surface $surface -Target $resolvedTarget -Description "$surface CLI archive for $resolvedTarget"
    $cliSource = if ([string]::IsNullOrWhiteSpace($resolvedLocalArtifactsRoot)) {
        Resolve-ReleaseAssetByName -Assets $release.assets -FileName ([string]$cliManifestAsset.fileName) -Description "$surface CLI archive"
    }
    else {
        Resolve-LocalAssetByName -ArtifactsRoot $resolvedLocalArtifactsRoot -FileName ([string]$cliManifestAsset.fileName) -Description "$surface CLI archive"
    }

    $cliArchivePath = Join-Path $downloadRoot $cliSource.FileName
    $surfacePath = Join-Path $binRoot $surface
    Initialize-DestinationDirectory -Path $surfacePath -AllowReplace:$true
    Copy-FileFromSource -DestinationPath $cliArchivePath -SourcePath $cliSource.SourcePath -SourceUri $cliSource.SourceUri

    $verifiedCliFile = Assert-StagedFileMatchesMetadata -FilePath $cliArchivePath -ManifestAsset $cliManifestAsset -ChecksumLookup $checksumLookup -PublishedSize $cliSource.PublishedSize
    $verifiedFiles.Add($verifiedCliFile) | Out-Null
    Assert-ArchiveRequiredEntries -ArchivePath $cliArchivePath -RequiredEntries (ConvertTo-StringArray -Value $cliManifestAsset.requiredEntries)

    Expand-Archive -Path $cliArchivePath -DestinationPath $surfacePath -Force

    $executableName = Get-ExecutableFileName -BinaryName $metadata.Binary -TargetTriple $resolvedTarget
    $executablePath = Join-Path $surfacePath $executableName
    if (-not (Test-Path $executablePath)) {
        throw "Installed CLI executable was not found at $executablePath"
    }

    Restore-UnixExecutablePermission -ExecutablePath $executablePath

    if ($surface -eq 'elegy-cli') {
        Initialize-DestinationDirectory -Path $legacyCliPath -AllowReplace:$true
        Copy-Item -Path (Join-Path $surfacePath '*') -Destination $legacyCliPath -Recurse -Force

        $legacyExecutablePath = Join-Path $legacyCliPath $executableName
        if (-not (Test-Path $legacyExecutablePath)) {
            throw "Installed compatibility CLI executable was not found at $legacyExecutablePath"
        }

        Restore-UnixExecutablePermission -ExecutablePath $legacyExecutablePath
    }

    $installedCliReports.Add([pscustomobject]@{
        Surface = $surface
        Asset = [string]$cliManifestAsset.fileName
        InstallPath = $surfacePath
        ExecutablePath = $executablePath
    }) | Out-Null

    $installedAssets.Add([pscustomobject]@{
        assetKind = [string]$cliManifestAsset.assetKind
        surface = $surface
        target = $resolvedTarget
        fileName = [string]$cliManifestAsset.fileName
        installPath = $surfacePath
        executablePath = $executablePath
        requiredEntries = ConvertTo-StringArray -Value $cliManifestAsset.requiredEntries
        sizeBytes = [int64]$cliManifestAsset.sizeBytes
        sha256 = [string]$cliManifestAsset.sha256
    }) | Out-Null
}

$installedWrapperReports = [System.Collections.Generic.List[object]]::new()
foreach ($surface in $resolvedWrapperSurfaces) {
    $metadata = $wrapperMetadata[$surface]
    $wrapperManifestAsset = Get-ManifestAsset -ManifestDocument $manifestDocument -AssetKind 'wrapper' -Surface $surface -Target '' -Description "$surface wrapper archive"
    $wrapperSource = if ([string]::IsNullOrWhiteSpace($resolvedLocalArtifactsRoot)) {
        Resolve-ReleaseAssetByName -Assets $release.assets -FileName ([string]$wrapperManifestAsset.fileName) -Description "$surface wrapper archive"
    }
    else {
        Resolve-LocalAssetByName -ArtifactsRoot $resolvedLocalArtifactsRoot -FileName ([string]$wrapperManifestAsset.fileName) -Description "$surface wrapper archive"
    }

    $wrapperArchivePath = Join-Path $downloadRoot $wrapperSource.FileName
    $surfacePath = Join-Path $wrapperRoot $surface
    Initialize-DestinationDirectory -Path $surfacePath -AllowReplace:$true
    Copy-FileFromSource -DestinationPath $wrapperArchivePath -SourcePath $wrapperSource.SourcePath -SourceUri $wrapperSource.SourceUri

    $verifiedWrapperFile = Assert-StagedFileMatchesMetadata -FilePath $wrapperArchivePath -ManifestAsset $wrapperManifestAsset -ChecksumLookup $checksumLookup -PublishedSize $wrapperSource.PublishedSize
    $verifiedFiles.Add($verifiedWrapperFile) | Out-Null
    Assert-ArchiveRequiredEntries -ArchivePath $wrapperArchivePath -RequiredEntries (ConvertTo-StringArray -Value $wrapperManifestAsset.requiredEntries)

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
        Asset = [string]$wrapperManifestAsset.fileName
        InstallPath = $surfacePath
        InstallerPath = $installerPath
        SkillBridgePath = $skillBridgePath
    }) | Out-Null

    $installedAssets.Add([pscustomobject]@{
        assetKind = [string]$wrapperManifestAsset.assetKind
        surface = $surface
        target = $null
        fileName = [string]$wrapperManifestAsset.fileName
        installPath = $surfacePath
        installerPath = $installerPath
        skillBridgePath = $skillBridgePath
        requiredEntries = ConvertTo-StringArray -Value $wrapperManifestAsset.requiredEntries
        sizeBytes = [int64]$wrapperManifestAsset.sizeBytes
        sha256 = [string]$wrapperManifestAsset.sha256
    }) | Out-Null
}

# Tool resolution contract (host-neutral):
# Priority order for agents, MCP hosts, and verifiers:
# 1. Install receipt commandShims[].shimPath
# 2. Install receipt commandShims[].targetExecutablePath
# 3. Explicit --bin-dir argument
# 4. PATH fallback

if (-not $NoCommandShims -and $installedCliReports.Count -gt 0) {
    $shimRoot = Join-Path $binRoot 'shims'
    New-Item -ItemType Directory -Path $shimRoot -Force | Out-Null
    $commandShims = [System.Collections.Generic.List[object]]::new()
    foreach ($report in $installedCliReports) {
        $shimPath = Join-Path $shimRoot "$($report.Surface).ps1"
        $shimContent = @"
& `"$($report.ExecutablePath)`" @args
"@
        Set-Content -Path $shimPath -Value $shimContent -Encoding utf8
        $commandShims.Add([pscustomobject]@{
            toolName = $report.Surface
            shimPath = $shimPath
            targetExecutablePath = $report.ExecutablePath
        })
    }
}
else {
    $shimRoot = $null
    $commandShims = @()
}

$pathUpdate = $null
if ($AddToPath -and $NoCommandShims) {
    Write-Warning 'AddToPath requires command shims. Ignoring -AddToPath.'
}
elseif ($AddToPath -and -not $NoCommandShims -and $shimRoot) {
    $normalizedShimRoot = $shimRoot.TrimEnd('\')
    $currentUserPath = [Environment]::GetEnvironmentVariable('PATH', 'User')
    $paths = $currentUserPath -split ';' | Where-Object { $_ } | ForEach-Object { $_.TrimEnd('\') }
    if ($normalizedShimRoot -notin $paths) {
        [Environment]::SetEnvironmentVariable('PATH', "$currentUserPath;$normalizedShimRoot", 'User')
        $pathUpdate = [ordered]@{
            variable = 'PATH'
            scope    = 'User'
            appendedPath = $normalizedShimRoot
            alreadyPresent = $false
        }
    }
    else {
        $pathUpdate = [ordered]@{
            variable = 'PATH'
            scope    = 'User'
            appendedPath = $normalizedShimRoot
            alreadyPresent = $true
        }
    }
}

$destinationRoot = (Resolve-Path -Path $Destination).Path
$receiptPath = Join-Path $destinationRoot 'install-receipt.json'
$installReceipt = [ordered]@{
    schemaVersion = '1.1.0'
    documentType = 'elegy-install-receipt'
    installedAtUtc = (Get-Date).ToUniversalTime().ToString('o')
    request = [ordered]@{
        destination = $destinationRoot
        cliSurfaces = @($resolvedCliSurfaces)
        wrapperSurfaces = @($resolvedWrapperSurfaces)
        force = [bool]$Force.IsPresent
    }
    source = [ordered]@{
        mode = if ([string]::IsNullOrWhiteSpace($resolvedLocalArtifactsRoot)) { 'github-release' } else { 'local-artifacts' }
        repository = if ([string]::IsNullOrWhiteSpace($resolvedLocalArtifactsRoot)) { $Repository } else { $null }
        tag = [string]$manifestDocument.tag
        localArtifactsRoot = if ([string]::IsNullOrWhiteSpace($resolvedLocalArtifactsRoot)) { $null } else { $resolvedLocalArtifactsRoot }
        manifest = $manifestSource.FileName
        checksums = $checksumsSource.FileName
    }
    hostTarget = $resolvedTarget
    verification = [ordered]@{
        algorithm = 'sha256'
        manifestBundleVersion = [string]$manifestDocument.bundleVersion
        verifiedAtUtc = (Get-Date).ToUniversalTime().ToString('o')
        files = @($verifiedFiles)
    }
    installedAssets = @($installedAssets)
    commandShimRoot = if ($shimRoot) { $shimRoot } else { $null }
    commandShims = @($commandShims)
    pathUpdate = $pathUpdate
}

$installReceipt | ConvertTo-Json -Depth 8 | Set-Content -Path $receiptPath -Encoding utf8

Write-Host 'Installed Elegy distribution assets.'
if ([string]::IsNullOrWhiteSpace($resolvedLocalArtifactsRoot)) {
    Write-Host " - repository: $Repository"
}
else {
    Write-Host " - local artifacts root: $resolvedLocalArtifactsRoot"
}
Write-Host " - release tag: $resolvedTag"
Write-Host " - contracts asset: $($contractsManifestAsset.fileName)"
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
Write-Host " - install receipt: $receiptPath"
if (-not $NoCommandShims -and $installedCliReports.Count -gt 0) {
    Write-Host ''
    Write-Host "Command shims created at: $shimRoot"
    if (-not $AddToPath) {
        Write-Host 'Add this directory to your PATH to invoke Elegy tools directly:'
        Write-Host "  [Environment]::SetEnvironmentVariable('PATH', `"`$env:PATH;$shimRoot`", 'User')"
        Write-Host 'Or re-run with -AddToPath.'
        Write-Host '(Note: PATH changes take effect in new PowerShell sessions.)'
    }
    elseif ($pathUpdate -and -not $pathUpdate.alreadyPresent) {
        Write-Host "Added to user PATH: $shimRoot"
        Write-Host '(Note: PATH changes take effect in new PowerShell sessions.)'
    }
    elseif ($pathUpdate -and $pathUpdate.alreadyPresent) {
        Write-Host "PATH already contains: $shimRoot (skipped)"
    }
}
