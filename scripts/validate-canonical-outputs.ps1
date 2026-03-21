[CmdletBinding()]
param(
    [switch]$RequireGeneratedOutputs,
    [switch]$RequireArchive,
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
        $matches = Get-ChildItem -Path (Join-Path $repoRoot $pattern) -ErrorAction SilentlyContinue
        if ($null -eq $matches -or $matches.Count -eq 0) {
            $missingArchives.Add($pattern) | Out-Null
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

Write-Host 'Canonical output validation passed.'
Write-Host " - authority-only files: $($inventory.authorityOnly.Count)"
Write-Host " - mirrored outputs: $($inventory.mirroredOutputs.Count)"
if ($RequireArchive) {
    Write-Host " - archive patterns: $($inventory.archivePatterns.Count)"
}