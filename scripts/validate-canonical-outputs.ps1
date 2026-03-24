[CmdletBinding()]
param(
    [switch]$RequireGeneratedOutputs,
    [switch]$RequireArchive,
    [switch]$RequireWrapperArchives,
    [switch]$RequireInstallerArchives,
    [switch]$EmitJson
)

$ErrorActionPreference = 'Stop'

$repoRoot = Split-Path -Parent $PSScriptRoot
$inventoryPath = Join-Path $repoRoot 'governance\canonical-output-inventory.json'

if (-not (Test-Path $inventoryPath)) {
    throw "Missing canonical output inventory: $inventoryPath"
}

$inventory = Get-Content -Raw -Path $inventoryPath | ConvertFrom-Json

$missingAuthority = [System.Collections.Generic.List[string]]::new()
$missingSources = [System.Collections.Generic.List[string]]::new()
$missingGenerated = [System.Collections.Generic.List[string]]::new()
$contentMismatches = [System.Collections.Generic.List[string]]::new()
$missingArchives = [System.Collections.Generic.List[string]]::new()
$missingWrapperArchives = [System.Collections.Generic.List[string]]::new()
$invalidWrapperArchives = [System.Collections.Generic.List[string]]::new()
$missingInstallerArchives = [System.Collections.Generic.List[string]]::new()
$invalidInstallerArchives = [System.Collections.Generic.List[string]]::new()

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

foreach ($relativePath in $inventory.authorityOnly) {
    $fullPath = Join-Path $repoRoot $relativePath
    if (-not (Test-Path $fullPath)) {
        $missingAuthority.Add($relativePath) | Out-Null
    }
}

foreach ($entry in $inventory.mirroredOutputs) {
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
        $archiveFiles = Get-ChildItem -Path (Join-Path $repoRoot $pattern) -ErrorAction SilentlyContinue
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

        $archiveFiles = Get-ChildItem -Path (Join-Path $repoRoot $pattern) -ErrorAction SilentlyContinue
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
        $archiveFiles = Get-ChildItem -Path (Join-Path $repoRoot $pattern) -ErrorAction SilentlyContinue
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

Write-Host 'Canonical output validation passed.'
Write-Host " - authority-only files: $($inventory.authorityOnly.Count)"
Write-Host " - mirrored outputs: $($inventory.mirroredOutputs.Count)"
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