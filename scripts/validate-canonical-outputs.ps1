[CmdletBinding()]
param(
    [switch]$RequireGeneratedOutputs,
    [switch]$RequireArchive,
    [switch]$RequireWrapperArchives,
    [switch]$RequireInstallerArchives,
    [switch]$RequireReleaseMetadata,
    [string]$DistributionDirectory = '',
    [switch]$EmitJson
)

$ErrorActionPreference = 'Stop'

$repoRoot = Split-Path -Parent $PSScriptRoot
$inventoryPath = Join-Path $repoRoot 'governance\canonical-output-inventory.json'
$manifestPath = Join-Path $repoRoot 'contracts\manifests\compatibility-manifest.json'

if (-not (Test-Path $inventoryPath)) {
    throw "Missing canonical output inventory: $inventoryPath"
}

if (-not (Test-Path $manifestPath)) {
    throw "Missing compatibility manifest: $manifestPath"
}

$inventory = Get-Content -Raw -Path $inventoryPath | ConvertFrom-Json
$manifest = Get-Content -Raw -Path $manifestPath | ConvertFrom-Json

# Derive fixture-related mirrored outputs from the manifest.
# This is the single source of truth for fixture-to-artifact mapping.
$derivedOutputs = [System.Collections.Generic.List[object]]::new()

foreach ($schema in $manifest.schemas) {
    $schemaSource = "contracts/schemas/$($schema.file)"
    $schemaGenerated = "artifacts/contracts/$($schema.file)"
    $derivedOutputs.Add([pscustomobject]@{ source = $schemaSource; generated = $schemaGenerated }) | Out-Null

    foreach ($fixture in $schema.fixtures) {
        $source = "contracts/$fixture"
        $generated = "artifacts/contracts/$fixture"
        $derivedOutputs.Add([pscustomobject]@{ source = $source; generated = $generated }) | Out-Null
    }
}

foreach ($fixture in $manifest.supplementalFixtures) {
    $source = "contracts/$fixture"
    $generated = "artifacts/contracts/$fixture"
    $derivedOutputs.Add([pscustomobject]@{ source = $source; generated = $generated }) | Out-Null
}

# Add the manifest and matrix themselves.
$derivedOutputs.Add([pscustomobject]@{
    source    = "contracts/manifests/compatibility-manifest.json"
    generated = "artifacts/contracts/compatibility-manifest.json"
}) | Out-Null
$derivedOutputs.Add([pscustomobject]@{
    source    = "contracts/manifests/compatibility-matrix.json"
    generated = "artifacts/contracts/compatibility-matrix.json"
}) | Out-Null

# Merge: derived fixture entries + hand-maintained inventory entries.
# The inventory retains distribution metadata (archives, wrappers, etc.)
# that is not derivable from the manifest.
$allMirrored = [System.Collections.Generic.List[object]]::new()
foreach ($entry in $derivedOutputs) {
    $allMirrored.Add($entry) | Out-Null
}
foreach ($entry in $inventory.mirroredOutputs) {
    $allMirrored.Add($entry) | Out-Null
}

$missingAuthority = [System.Collections.Generic.List[string]]::new()
$missingSources = [System.Collections.Generic.List[string]]::new()
$missingGenerated = [System.Collections.Generic.List[string]]::new()
$contentMismatches = [System.Collections.Generic.List[string]]::new()
$missingArchives = [System.Collections.Generic.List[string]]::new()
$missingWrapperArchives = [System.Collections.Generic.List[string]]::new()
$invalidWrapperArchives = [System.Collections.Generic.List[string]]::new()
$missingInstallerArchives = [System.Collections.Generic.List[string]]::new()
$invalidInstallerArchives = [System.Collections.Generic.List[string]]::new()
$missingReleaseMetadata = [System.Collections.Generic.List[string]]::new()
$invalidReleaseMetadata = [System.Collections.Generic.List[string]]::new()

function Test-ArchiveRequiredEntries {
    param(
        [string]$ArchivePath,
        [string[]]$RequiredEntries
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

        $missingEntries = [System.Collections.Generic.List[string]]::new()
        foreach ($requiredEntry in $RequiredEntries) {
            if (-not $entryLookup.Contains($requiredEntry)) {
                $missingEntries.Add($requiredEntry) | Out-Null
            }
        }

        return @($missingEntries)
    }
    finally {
        $archive.Dispose()
    }
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

function Resolve-DistributionPatternPath {
    param(
        [string]$InventoryPattern,
        [string]$RepositoryRoot,
        [string]$OverrideDirectory
    )

    if ([string]::IsNullOrWhiteSpace($OverrideDirectory)) {
        return Join-Path $RepositoryRoot $InventoryPattern
    }

    return Join-Path $OverrideDirectory (Split-Path -Leaf $InventoryPattern)
}

function Get-SingleMatchedFile {
    param(
        [string]$PatternPath,
        [string]$Description,
        [System.Collections.Generic.List[string]]$MissingList,
        [System.Collections.Generic.List[string]]$InvalidList
    )

    $matchedFiles = @(Get-ChildItem -Path $PatternPath -File -ErrorAction SilentlyContinue | Sort-Object Name)
    if ($matchedFiles.Count -eq 0) {
        $MissingList.Add($Description) | Out-Null
        return $null
    }

    if ($matchedFiles.Count -gt 1) {
        $matchNames = $matchedFiles | ForEach-Object { $_.Name }
        $InvalidList.Add("Ambiguous $Description matches for pattern ${PatternPath}: $($matchNames -join ', ')") | Out-Null
        return $null
    }

    return $matchedFiles[0]
}

function New-ChecksumLookup {
    param(
        [object]$ChecksumsDocument,
        [System.Collections.Generic.List[string]]$InvalidList
    )

    $lookup = [System.Collections.Generic.Dictionary[string, string]]::new([System.StringComparer]::OrdinalIgnoreCase)
    foreach ($entry in @($ChecksumsDocument.entries)) {
        $fileName = [string]$entry.fileName
        if ([string]::IsNullOrWhiteSpace($fileName)) {
            $InvalidList.Add('Release checksums metadata included an empty fileName entry.') | Out-Null
            continue
        }

        if ($lookup.ContainsKey($fileName)) {
            $InvalidList.Add("Release checksums metadata included a duplicate fileName entry for $fileName") | Out-Null
            continue
        }

        $lookup[$fileName] = ([string]$entry.sha256).ToLowerInvariant()
    }

    return $lookup
}

foreach ($relativePath in $inventory.authorityOnly) {
    $fullPath = Join-Path $repoRoot $relativePath
    if (-not (Test-Path $fullPath)) {
        $missingAuthority.Add($relativePath) | Out-Null
    }
}

foreach ($entry in $allMirrored) {
    $sourcePath = Join-Path $repoRoot $entry.source
    $generatedPath = Join-Path $repoRoot $entry.generated

    if (-not (Test-Path $sourcePath)) {
        $missingSources.Add($entry.source) | Out-Null
        continue
    }

    if (-not (Test-Path $generatedPath)) {
        if ($RequireGeneratedOutputs) {
            $missingGenerated.Add($entry.generated) | Out-Null
        }

        continue
    }

    $sourceHash = (Get-FileHash -Path $sourcePath -Algorithm SHA256).Hash
    $generatedHash = (Get-FileHash -Path $generatedPath -Algorithm SHA256).Hash

    if ($sourceHash -ne $generatedHash) {
        $contentMismatches.Add("$($entry.source) != $($entry.generated)") | Out-Null
    }
}

if ($RequireArchive) {
    foreach ($pattern in $inventory.archivePatterns) {
        $resolvedPattern = Resolve-DistributionPatternPath -InventoryPattern $pattern -RepositoryRoot $repoRoot -OverrideDirectory $DistributionDirectory
        $archiveFiles = Get-ChildItem -Path $resolvedPattern -ErrorAction SilentlyContinue
        if ($null -eq $archiveFiles -or $archiveFiles.Count -eq 0) {
            $missingArchives.Add($pattern) | Out-Null
        }
    }
}

if ($RequireWrapperArchives -and $null -ne $inventory.wrapperArchivePatterns) {
    foreach ($pattern in $inventory.wrapperArchivePatterns) {
        $requiredWrapperEntries = [System.Collections.Generic.List[string]]::new()
        foreach ($entry in @($inventory.wrapperArchiveRequiredEntries)) {
            $requiredWrapperEntries.Add([string]$entry) | Out-Null
        }

        $surfaceSpecificEntries = $null
        if ($null -ne $inventory.wrapperArchiveSurfaceSpecificEntries) {
            $surfaceSpecificEntries = $inventory.wrapperArchiveSurfaceSpecificEntries.$pattern
        }

        foreach ($entry in @($surfaceSpecificEntries)) {
            $requiredWrapperEntries.Add([string]$entry) | Out-Null
        }

        $resolvedPattern = Resolve-DistributionPatternPath -InventoryPattern $pattern -RepositoryRoot $repoRoot -OverrideDirectory $DistributionDirectory
        $archiveFiles = Get-ChildItem -Path $resolvedPattern -ErrorAction SilentlyContinue
        if ($null -eq $archiveFiles -or $archiveFiles.Count -eq 0) {
            $missingWrapperArchives.Add($pattern) | Out-Null
            continue
        }

        foreach ($archive in $archiveFiles) {
            $missingEntries = Test-ArchiveRequiredEntries -ArchivePath $archive.FullName -RequiredEntries @($requiredWrapperEntries)
            if ($missingEntries.Count -gt 0) {
                $invalidWrapperArchives.Add("$($archive.Name) missing required entries: $($missingEntries -join ', ')") | Out-Null
            }
        }
    }
}

if ($RequireInstallerArchives -and $null -ne $inventory.installerArchivePatterns) {
    $requiredInstallerEntries = [System.Collections.Generic.List[string]]::new()
    foreach ($entry in @($inventory.installerArchiveRequiredEntries)) {
        $requiredInstallerEntries.Add([string]$entry) | Out-Null
    }

    foreach ($pattern in $inventory.installerArchivePatterns) {
        $resolvedPattern = Resolve-DistributionPatternPath -InventoryPattern $pattern -RepositoryRoot $repoRoot -OverrideDirectory $DistributionDirectory
        $archiveFiles = Get-ChildItem -Path $resolvedPattern -ErrorAction SilentlyContinue
        if ($null -eq $archiveFiles -or $archiveFiles.Count -eq 0) {
            $missingInstallerArchives.Add($pattern) | Out-Null
            continue
        }

        foreach ($archive in $archiveFiles) {
            $missingEntries = Test-ArchiveRequiredEntries -ArchivePath $archive.FullName -RequiredEntries @($requiredInstallerEntries)
            if ($missingEntries.Count -gt 0) {
                $invalidInstallerArchives.Add("$($archive.Name) missing required entries: $($missingEntries -join ', ')") | Out-Null
            }
        }
    }
}

if ($RequireReleaseMetadata) {
    if ($null -eq $inventory.releaseMetadataPatterns) {
        $invalidReleaseMetadata.Add('Canonical output inventory did not define releaseMetadataPatterns.') | Out-Null
    }
    else {
        $manifestPattern = Resolve-DistributionPatternPath -InventoryPattern ([string]$inventory.releaseMetadataPatterns.manifest) -RepositoryRoot $repoRoot -OverrideDirectory $DistributionDirectory
        $checksumsPattern = Resolve-DistributionPatternPath -InventoryPattern ([string]$inventory.releaseMetadataPatterns.checksums) -RepositoryRoot $repoRoot -OverrideDirectory $DistributionDirectory

        $manifestFile = Get-SingleMatchedFile -PatternPath $manifestPattern -Description 'release manifest metadata' -MissingList $missingReleaseMetadata -InvalidList $invalidReleaseMetadata
        $checksumsFile = Get-SingleMatchedFile -PatternPath $checksumsPattern -Description 'release checksums metadata' -MissingList $missingReleaseMetadata -InvalidList $invalidReleaseMetadata

        if ($null -ne $manifestFile -and $null -ne $checksumsFile) {
            $manifestDocument = Get-Content -Raw -Path $manifestFile.FullName | ConvertFrom-Json
            $checksumsDocument = Get-Content -Raw -Path $checksumsFile.FullName | ConvertFrom-Json

            if ([string]$manifestDocument.documentType -ne 'elegy-release-manifest') {
                $invalidReleaseMetadata.Add("Unexpected release manifest documentType: $($manifestDocument.documentType)") | Out-Null
            }

            if ([string]$checksumsDocument.documentType -ne 'elegy-release-checksums') {
                $invalidReleaseMetadata.Add("Unexpected release checksums documentType: $($checksumsDocument.documentType)") | Out-Null
            }

            if ([string]::IsNullOrWhiteSpace([string]$manifestDocument.bundleVersion)) {
                $invalidReleaseMetadata.Add('Release manifest metadata did not include bundleVersion.') | Out-Null
            }
            else {
                $expectedManifestFileName = "elegy-release-manifest-$($manifestDocument.bundleVersion).json"
                if ($manifestFile.Name -ne $expectedManifestFileName) {
                    $invalidReleaseMetadata.Add("Release manifest file name $($manifestFile.Name) did not match bundleVersion $($manifestDocument.bundleVersion)") | Out-Null
                }

                $expectedChecksumsFileName = "elegy-release-checksums-$($manifestDocument.bundleVersion).json"
                if ($checksumsFile.Name -ne $expectedChecksumsFileName) {
                    $invalidReleaseMetadata.Add("Release checksums file name $($checksumsFile.Name) did not match bundleVersion $($manifestDocument.bundleVersion)") | Out-Null
                }
            }

            if (-not [string]::Equals([string]$checksumsDocument.algorithm, 'sha256', [System.StringComparison]::OrdinalIgnoreCase)) {
                $invalidReleaseMetadata.Add("Unsupported release checksums algorithm: $($checksumsDocument.algorithm)") | Out-Null
            }

            if (-not [string]::Equals([string]$manifestDocument.tag, [string]$checksumsDocument.tag, [System.StringComparison]::Ordinal)) {
                $invalidReleaseMetadata.Add('Release manifest and checksums metadata did not agree on the tag marker.') | Out-Null
            }

            $checksumLookup = New-ChecksumLookup -ChecksumsDocument $checksumsDocument -InvalidList $invalidReleaseMetadata
            if ($checksumLookup.ContainsKey($manifestFile.Name)) {
                $manifestHash = Get-FileSha256 -Path $manifestFile.FullName
                if ($checksumLookup[$manifestFile.Name] -ne $manifestHash) {
                    $invalidReleaseMetadata.Add("Release manifest checksum mismatch for $($manifestFile.Name)") | Out-Null
                }
            }
            else {
                $invalidReleaseMetadata.Add("Release checksums metadata did not include the manifest file $($manifestFile.Name)") | Out-Null
            }

            $manifestAssetNames = [System.Collections.Generic.HashSet[string]]::new([System.StringComparer]::OrdinalIgnoreCase)
            $distributionRoot = Split-Path -Parent $manifestFile.FullName
            foreach ($asset in @($manifestDocument.assets)) {
                $assetFileName = [string]$asset.fileName
                if ([string]::IsNullOrWhiteSpace($assetFileName)) {
                    $invalidReleaseMetadata.Add('Release manifest metadata included an asset with an empty fileName.') | Out-Null
                    continue
                }

                if (-not $manifestAssetNames.Add($assetFileName)) {
                    $invalidReleaseMetadata.Add("Release manifest metadata included a duplicate asset entry for $assetFileName") | Out-Null
                    continue
                }

                $assetPath = Join-Path $distributionRoot $assetFileName
                if (-not (Test-Path $assetPath)) {
                    $invalidReleaseMetadata.Add("Release manifest asset was not found: $assetFileName") | Out-Null
                    continue
                }

                $fileInfo = Get-Item -Path $assetPath
                if ($fileInfo.Length -ne [int64]$asset.sizeBytes) {
                    $invalidReleaseMetadata.Add("Release manifest size mismatch for $assetFileName") | Out-Null
                }

                $actualHash = Get-FileSha256 -Path $assetPath
                $manifestHash = ([string]$asset.sha256).ToLowerInvariant()
                if ($actualHash -ne $manifestHash) {
                    $invalidReleaseMetadata.Add("Release manifest SHA-256 mismatch for $assetFileName") | Out-Null
                }

                if ($checksumLookup.ContainsKey($assetFileName)) {
                    if ($checksumLookup[$assetFileName] -ne $manifestHash) {
                        $invalidReleaseMetadata.Add("Release checksums SHA-256 mismatch for $assetFileName") | Out-Null
                    }
                }
                else {
                    $invalidReleaseMetadata.Add("Release checksums metadata did not include $assetFileName") | Out-Null
                }

                $requiredEntries = ConvertTo-StringArray -Value $asset.requiredEntries
                if ($requiredEntries.Count -gt 0) {
                    $missingEntries = Test-ArchiveRequiredEntries -ArchivePath $assetPath -RequiredEntries $requiredEntries
                    if ($missingEntries.Count -gt 0) {
                        $invalidReleaseMetadata.Add("$assetFileName missing manifest required entries: $($missingEntries -join ', ')") | Out-Null
                    }
                }
            }
        }
    }
}

$result = [pscustomobject]@{
    inventoryVersion = $inventory.inventoryVersion
    missingAuthority = $missingAuthority
    missingSources = $missingSources
    missingGenerated = $missingGenerated
    contentMismatches = $contentMismatches
    missingArchives = $missingArchives
    missingWrapperArchives = $missingWrapperArchives
    invalidWrapperArchives = $invalidWrapperArchives
    missingInstallerArchives = $missingInstallerArchives
    invalidInstallerArchives = $invalidInstallerArchives
    missingReleaseMetadata = $missingReleaseMetadata
    invalidReleaseMetadata = $invalidReleaseMetadata
}

if ($EmitJson) {
    $result | ConvertTo-Json -Depth 6
}

if ($missingAuthority.Count -gt 0) {
    throw ('Missing authority-only files: ' + ($missingAuthority -join ', '))
}

if ($missingSources.Count -gt 0) {
    throw ('Missing source files from canonical output inventory: ' + ($missingSources -join ', '))
}

if ($missingGenerated.Count -gt 0) {
    throw ('Missing generated files from canonical output inventory: ' + ($missingGenerated -join ', '))
}

if ($contentMismatches.Count -gt 0) {
    throw ('Canonical output mismatches detected: ' + ($contentMismatches -join '; '))
}

if ($missingArchives.Count -gt 0) {
    throw ('Missing generated archives from canonical output inventory: ' + ($missingArchives -join ', '))
}

if ($missingWrapperArchives.Count -gt 0) {
    throw ('Missing wrapper archives from canonical output inventory: ' + ($missingWrapperArchives -join ', '))
}

if ($invalidWrapperArchives.Count -gt 0) {
    throw ('Wrapper archives are missing required payload entries: ' + ($invalidWrapperArchives -join '; '))
}

if ($missingInstallerArchives.Count -gt 0) {
    throw ('Missing installer archives from canonical output inventory: ' + ($missingInstallerArchives -join ', '))
}

if ($invalidInstallerArchives.Count -gt 0) {
    throw ('Installer archives are missing required payload entries: ' + ($invalidInstallerArchives -join '; '))
}

if ($missingReleaseMetadata.Count -gt 0) {
    throw ('Missing release metadata outputs from canonical output inventory: ' + ($missingReleaseMetadata -join ', '))
}

if ($invalidReleaseMetadata.Count -gt 0) {
    throw ('Release metadata validation failures detected: ' + ($invalidReleaseMetadata -join '; '))
}

Write-Host 'Canonical output validation passed.'
Write-Host " - authority-only files: $($inventory.authorityOnly.Count)"
Write-Host " - mirrored outputs: $($allMirrored.Count)"
if ($RequireArchive) {
    Write-Host " - archive patterns: $($inventory.archivePatterns.Count)"
}
if ($RequireWrapperArchives -and $null -ne $inventory.wrapperArchivePatterns) {
    Write-Host " - wrapper archive patterns: $($inventory.wrapperArchivePatterns.Count)"
    Write-Host " - wrapper archive required entries: $($inventory.wrapperArchiveRequiredEntries.Count)"
    if ($null -ne $inventory.wrapperArchiveSurfaceSpecificEntries) {
        Write-Host " - wrapper archive surface-specific entry sets: $($inventory.wrapperArchiveSurfaceSpecificEntries.PSObject.Properties.Count)"
    }
}
if ($RequireInstallerArchives -and $null -ne $inventory.installerArchivePatterns) {
    Write-Host " - installer archive patterns: $($inventory.installerArchivePatterns.Count)"
    Write-Host " - installer archive required entries: $($inventory.installerArchiveRequiredEntries.Count)"
}
if ($RequireReleaseMetadata -and $null -ne $inventory.releaseMetadataPatterns) {
    Write-Host ' - release metadata patterns: 2'
}