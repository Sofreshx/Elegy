[CmdletBinding()]
param(
    [string]$OutputDirectory = '',
    [string]$Repository = 'Sofreshx/Elegy',
    [string]$Tag = 'local-artifacts'
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

    return [string]$versionPolicy.bundleVersion
}

function Get-PublishedCliTargets {
    return @(
        'x86_64-pc-windows-msvc',
        'x86_64-unknown-linux-gnu',
        'aarch64-apple-darwin'
    )
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
    }
}

function Get-WrapperSurfaceMetadata {
    return [ordered]@{
        'elegy-memory' = @{
            Surface = 'elegy-memory'
            AssetPrefix = 'elegy-memory-wrapper'
            SkillBridge = 'skills/elegy-memory/SKILL.md'
        }
        'elegy-mcp' = @{
            Surface = 'elegy-mcp'
            AssetPrefix = 'elegy-mcp-wrapper'
            SkillBridge = 'skills/elegy-mcp/SKILL.md'
        }
        'elegy-planning' = @{
            Surface = 'elegy-planning'
            AssetPrefix = 'elegy-planning-wrapper'
            SkillBridge = 'skills/elegy-planning/SKILL.md'
        }
        'elegy-skills' = @{
            Surface = 'elegy-skills'
            AssetPrefix = 'elegy-skills-wrapper'
            SkillBridge = 'skills/elegy-skills/SKILL.md'
        }
        'elegy-configuration' = @{
            Surface = 'elegy-configuration'
            AssetPrefix = 'elegy-configuration-wrapper'
            SkillBridge = 'skills/elegy-configuration/SKILL.md'
        }
    }
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

function New-DistributionAssetRecord {
    param(
        [System.IO.FileInfo]$File,
        [string]$BundleVersion
    )

    $assetName = $File.Name
    $cliMetadata = Get-CliSurfaceMetadata
    $wrapperMetadata = Get-WrapperSurfaceMetadata
    $publishedTargets = Get-PublishedCliTargets

    foreach ($surfaceName in $cliMetadata.Keys) {
        $metadata = $cliMetadata[$surfaceName]
        $prefix = "$($metadata.AssetPrefix)-"
        foreach ($target in $publishedTargets) {
            $suffix = "-$target.zip"
            if ($assetName.StartsWith($prefix, [System.StringComparison]::OrdinalIgnoreCase) -and $assetName.EndsWith($suffix, [System.StringComparison]::OrdinalIgnoreCase)) {
                $versionLength = $assetName.Length - $prefix.Length - $suffix.Length
                if ($versionLength -le 0) {
                    throw "Unable to resolve the version segment for CLI asset $assetName"
                }

                $version = $assetName.Substring($prefix.Length, $versionLength)
                $executableEntry = Get-ExecutableFileName -BinaryName $metadata.Binary -TargetTriple $target
                $requiredEntries = @(
                    $executableEntry,
                    'README.md'
                )
                Assert-ArchiveRequiredEntries -ArchivePath $File.FullName -RequiredEntries $requiredEntries

                return [ordered]@{
                    fileName = $assetName
                    assetKind = 'cli'
                    surface = $metadata.Surface
                    target = $target
                    version = $version
                    sizeBytes = [int64]$File.Length
                    sha256 = Get-FileSha256 -Path $File.FullName
                    requiredEntries = @($requiredEntries)
                }
            }
        }
    }

    foreach ($surfaceName in $wrapperMetadata.Keys) {
        $metadata = $wrapperMetadata[$surfaceName]
        $prefix = "$($metadata.AssetPrefix)-"
        $suffix = '.zip'
        if ($assetName.StartsWith($prefix, [System.StringComparison]::OrdinalIgnoreCase) -and $assetName.EndsWith($suffix, [System.StringComparison]::OrdinalIgnoreCase)) {
            $version = $assetName.Substring($prefix.Length, $assetName.Length - $prefix.Length - $suffix.Length)
            if ($version -ne $BundleVersion) {
                return $null
            }

            $requiredEntries = @(
                'README.md',
                'install.ps1',
                'wrapper-entrypoint.json',
                'scripts/install-distribution.ps1',
                $metadata.SkillBridge
            )
            Assert-ArchiveRequiredEntries -ArchivePath $File.FullName -RequiredEntries $requiredEntries

            return [ordered]@{
                fileName = $assetName
                assetKind = 'wrapper'
                surface = $metadata.Surface
                target = $null
                version = $version
                sizeBytes = [int64]$File.Length
                sha256 = Get-FileSha256 -Path $File.FullName
                requiredEntries = @($requiredEntries)
            }
        }
    }

    if ($assetName.StartsWith('elegy-contracts-', [System.StringComparison]::OrdinalIgnoreCase) -and $assetName.EndsWith('.zip', [System.StringComparison]::OrdinalIgnoreCase)) {
        $version = $assetName.Substring('elegy-contracts-'.Length, $assetName.Length - 'elegy-contracts-'.Length - 4)
        if ($version -ne $BundleVersion) {
            return $null
        }

        $requiredEntries = @(
            'compatibility-manifest.json',
            'compatibility-matrix.json'
        )
        Assert-ArchiveRequiredEntries -ArchivePath $File.FullName -RequiredEntries $requiredEntries

        return [ordered]@{
            fileName = $assetName
            assetKind = 'contracts-bundle'
            surface = $null
            target = $null
            version = $version
            sizeBytes = [int64]$File.Length
            sha256 = Get-FileSha256 -Path $File.FullName
            requiredEntries = @($requiredEntries)
        }
    }

    if ($assetName.StartsWith('elegy-installer-', [System.StringComparison]::OrdinalIgnoreCase) -and $assetName.EndsWith('.zip', [System.StringComparison]::OrdinalIgnoreCase)) {
        $version = $assetName.Substring('elegy-installer-'.Length, $assetName.Length - 'elegy-installer-'.Length - 4)
        if ($version -ne $BundleVersion) {
            return $null
        }

        $requiredEntries = @(
            'install-distribution.ps1',
            'README.md'
        )
        Assert-ArchiveRequiredEntries -ArchivePath $File.FullName -RequiredEntries $requiredEntries

        return [ordered]@{
            fileName = $assetName
            assetKind = 'installer-bootstrap'
            surface = $null
            target = $null
            version = $version
            sizeBytes = [int64]$File.Length
            sha256 = Get-FileSha256 -Path $File.FullName
            requiredEntries = @($requiredEntries)
        }
    }

    throw "Unsupported distribution asset name: $assetName"
}

if ([string]::IsNullOrWhiteSpace($OutputDirectory)) {
    $OutputDirectory = Join-Path $repoRoot 'artifacts/distribution'
}

if (-not (Test-Path $OutputDirectory)) {
    throw "Distribution output directory was not found: $OutputDirectory"
}

$resolvedOutputDirectory = (Resolve-Path -Path $OutputDirectory).Path
$bundleVersion = Get-BundleVersion -RepositoryRoot $repoRoot
$zipFiles = Get-ChildItem -Path $resolvedOutputDirectory -Filter 'elegy-*.zip' -File | Sort-Object Name
if ($zipFiles.Count -eq 0) {
    throw "No distribution zip assets were found in $resolvedOutputDirectory"
}

$assetRecords = [System.Collections.Generic.List[object]]::new()
$fileNameLookup = [System.Collections.Generic.HashSet[string]]::new([System.StringComparer]::OrdinalIgnoreCase)
foreach ($zipFile in $zipFiles) {
    $assetRecord = New-DistributionAssetRecord -File $zipFile -BundleVersion $bundleVersion
    if ($null -eq $assetRecord) {
        continue
    }

    if (-not $fileNameLookup.Add([string]$assetRecord.fileName)) {
        throw "Duplicate distribution asset entry detected for $($assetRecord.fileName)"
    }

    $assetRecords.Add($assetRecord) | Out-Null
}

if ($assetRecords.Count -eq 0) {
    throw "No distribution assets matched the current bundle version $bundleVersion in $resolvedOutputDirectory"
}

$manifestFileName = "elegy-release-manifest-$bundleVersion.json"
$checksumsFileName = "elegy-release-checksums-$bundleVersion.json"
$manifestPath = Join-Path $resolvedOutputDirectory $manifestFileName
$checksumsPath = Join-Path $resolvedOutputDirectory $checksumsFileName

if (Test-Path $manifestPath) {
    Remove-Item -Path $manifestPath -Force
}

if (Test-Path $checksumsPath) {
    Remove-Item -Path $checksumsPath -Force
}

$publishedTargets = @(
    $assetRecords |
        Where-Object { $_.assetKind -eq 'cli' } |
        ForEach-Object { [string]$_.target } |
        Sort-Object -Unique
)

$manifestDocument = [ordered]@{
    schemaVersion = '1.0.0'
    documentType = 'elegy-release-manifest'
    repository = $Repository
    tag = $Tag
    bundleVersion = $bundleVersion
    generatedAtUtc = (Get-Date).ToUniversalTime().ToString('o')
    publishedTargets = @($publishedTargets)
    assets = @(
        $assetRecords |
            Sort-Object fileName |
            ForEach-Object {
                [ordered]@{
                    fileName = [string]$_.fileName
                    assetKind = [string]$_.assetKind
                    surface = if ($null -eq $_.surface) { $null } else { [string]$_.surface }
                    target = if ($null -eq $_.target) { $null } else { [string]$_.target }
                    version = [string]$_.version
                    sizeBytes = [int64]$_.sizeBytes
                    sha256 = [string]$_.sha256
                    requiredEntries = @([string[]]$_.requiredEntries)
                }
            }
    )
}

$manifestDocument | ConvertTo-Json -Depth 8 | Set-Content -Path $manifestPath -Encoding utf8

$checksumEntries = [System.Collections.Generic.List[object]]::new()
foreach ($asset in @($manifestDocument.assets)) {
    $checksumEntries.Add([ordered]@{
        fileName = [string]$asset.fileName
        sha256 = [string]$asset.sha256
    }) | Out-Null
}

$checksumEntries.Add([ordered]@{
    fileName = $manifestFileName
    sha256 = Get-FileSha256 -Path $manifestPath
}) | Out-Null

$checksumsDocument = [ordered]@{
    schemaVersion = '1.0.0'
    documentType = 'elegy-release-checksums'
    tag = $Tag
    algorithm = 'sha256'
    entries = @(
        $checksumEntries |
            Sort-Object fileName |
            ForEach-Object {
                [ordered]@{
                    fileName = [string]$_.fileName
                    sha256 = [string]$_.sha256
                }
            }
    )
}

$checksumsDocument | ConvertTo-Json -Depth 6 | Set-Content -Path $checksumsPath -Encoding utf8

Write-Host 'Wrote distribution release metadata.'
Write-Host " - output directory: $resolvedOutputDirectory"
Write-Host " - manifest: $manifestPath"
Write-Host " - checksums: $checksumsPath"
Write-Host " - tag: $Tag"
Write-Host " - published targets: $($publishedTargets -join ', ')"
Write-Host " - asset count: $($assetRecords.Count)"
