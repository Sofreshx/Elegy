<#
.SYNOPSIS
Validates Elegy registry alignment across built-in skill definitions, governed fixtures,
discovery indexes, manifest references, canonical output inventory, and boundary policy.

.DESCRIPTION
Performs six cross-checks:
  1. Built-in skill definitions → fixture file existence
  2. All governed fixtures in the built-in array
  3. Discovery indexes reference valid skill fixtures
  4. Manifest wrapper-surface paths exist
  5. Canonical output inventory source files exist
  6. Boundary policy required files exist

.PARAMETER RepoRoot
Path to the repository root. Defaults to the script's parent directory.

.PARAMETER Json
Switch to output machine-readable JSON instead of human-readable text.

.EXAMPLE
& ./scripts/validate-registry-alignment.ps1 -RepoRoot .
& ./scripts/validate-registry-alignment.ps1 -RepoRoot . -Json
#>

[CmdletBinding()]
param(
    [string]$RepoRoot = (Split-Path -Parent $PSScriptRoot),
    [switch]$Json
)

$ErrorActionPreference = 'Stop'

# --- Helper functions ---

function Write-ErrorLine {
    param([string]$Message)
    [Console]::Error.WriteLine($Message)
}

function Add-Issue {
    param(
        [string]$Check,
        [string]$Severity,
        [string]$Message
    )
    $script:issues.Add([pscustomobject]@{
        check = $Check
        severity = $Severity
        message = $Message
    })
}

function Get-FileContent {
    param([string]$RelativePath)
    $fullPath = Join-Path $RepoRoot $RelativePath
    if (-not (Test-Path $fullPath)) {
        return $null
    }
    return Get-Content -Raw -Path $fullPath
}

# --- Initialize ---
$issues = [System.Collections.Generic.List[object]]::new()
$checkResults = [ordered]@{}

$libRsPath = 'rust/crates/elegy-contracts/src/lib.rs'
$libRsContent = Get-FileContent -RelativePath $libRsPath

# =====================================================================
# CHECK 1: Built-in skill definitions → fixture file existence
# =====================================================================
$check1Issues = [System.Collections.Generic.List[string]]::new()
$builtinEntries = [System.Collections.Generic.List[pscustomobject]]::new()

if ($null -eq $libRsContent) {
    Add-Issue -Check 1 -Severity error -Message "Cannot read $libRsPath — skipping Check 1"
} else {
    # Extract BUILTIN_SKILL_DEFINITIONS array
    # Match pattern: id: "..." and include_str!("...")
    $pattern = '(?s)BUILTIN_SKILL_DEFINITIONS\s*:\s*&\[BuiltinSkillDefinition\]\s*=\s*&\[(.*?)\];'
    $match = [regex]::Match($libRsContent, $pattern)

    if (-not $match.Success) {
        Add-Issue -Check 1 -Severity error -Message "Could not find BUILTIN_SKILL_DEFINITIONS array in $libRsPath"
    } else {
        $arrayBody = $match.Groups[1].Value

        # Extract all BuiltinSkillDefinition entries
        $entryPattern = 'BuiltinSkillDefinition\s*\{[^}]*id:\s*"([^"]+)"[^}]*json:\s*include_str!\s*\(\s*"([^"]+)"\s*\)[^}]*\}'
        $entryMatches = [regex]::Matches($arrayBody, $entryPattern)

        if ($entryMatches.Count -eq 0) {
            Add-Issue -Check 1 -Severity error -Message "Could not parse any BuiltinSkillDefinition entries in BUILTIN_SKILL_DEFINITIONS"
        } else {
            foreach ($entryMatch in $entryMatches) {
                $skillId = $entryMatch.Groups[1].Value
                $includePath = $entryMatch.Groups[2].Value
                $fixtureFilename = $includePath -replace '.*/', ''

                $builtinEntries.Add([pscustomobject]@{
                    id = $skillId
                    includeStr = $includePath
                    fixtureFilename = $fixtureFilename
                })

                $fixturePath = "contracts/fixtures/$fixtureFilename"
                $fullFixturePath = Join-Path $RepoRoot $fixturePath
                if (-not (Test-Path $fullFixturePath)) {
                    $msg = "Built-in skill '$skillId' references fixture '$fixtureFilename' but file does not exist at $fixturePath"
                    $check1Issues.Add($msg)
                    Add-Issue -Check 1 -Severity error -Message $msg
                }
            }

            # Sanity cross-check: count include_str! occurrences in the file
            $includeStrCount = ([regex]::Matches($libRsContent, 'include_str!\s*\(')).Count
            if ($includeStrCount -ne $builtinEntries.Count) {
                Add-Issue -Check 1 -Severity warn -Message "include_str! count mismatch: regex parsed $($builtinEntries.Count) entries but file contains $includeStrCount include_str! calls. The regex may have missed entries."
            }
        }
    }
}

$checkResults['check1'] = @{
    name = 'Built-in skill definitions → fixture files'
    entriesParsed = $builtinEntries.Count
    missingFixtures = $check1Issues.Count
    passed = ($check1Issues.Count -eq 0)
}

# =====================================================================
# CHECK 2: All governed fixtures referenced in built-in array
# =====================================================================
$check2Issues = [System.Collections.Generic.List[string]]::new()

$fixturesDir = Join-Path $RepoRoot 'contracts/fixtures'
$governedFixtures = @(Get-ChildItem -Path $fixturesDir -Filter 'skill.elegy-*.json' -File |
    Where-Object { $_.Name -ne 'skill.minimal.json' -and $_.Name -ne 'skill.negative-no-output-schema.json' } |
    ForEach-Object { $_.Name })

$referencedFilenames = [System.Collections.Generic.HashSet[string]]::new([System.StringComparer]::OrdinalIgnoreCase)
foreach ($entry in $builtinEntries) {
    $referencedFilenames.Add($entry.fixtureFilename) | Out-Null
}

if ($builtinEntries.Count -eq 0) {
    Write-Host "  CHECK 2: SKIP - no built-in entries parsed (Check 1 may have failed)"
    Add-Issue -Check 2 -Severity warn -Message "Skipping Check 2: no built-in entries parsed from rust source (Check 1 may have failed)"
    return  # skip the rest of Check 2
}

foreach ($fixtureName in $governedFixtures) {
    if (-not $referencedFilenames.Contains($fixtureName)) {
        $msg = "Governed fixture '$fixtureName' is NOT referenced in BUILTIN_SKILL_DEFINITIONS"
        $check2Issues.Add($msg)
        Add-Issue -Check 2 -Severity warn -Message $msg
    }
}

$checkResults['check2'] = @{
    name = 'All governed fixtures in built-in array'
    governedFixtures = $governedFixtures.Count
    notInBuiltin = $check2Issues.Count
    passed = ($check2Issues.Count -eq 0)
}

# =====================================================================
# CHECK 3: Discovery indexes reference valid skill fixtures
# =====================================================================
$check3Issues = [System.Collections.Generic.List[string]]::new()

$discoveryIndexes = @(Get-ChildItem -Path $fixturesDir -Filter 'skill-discovery-index.*.json' -File |
    Where-Object { $_.Name -ne 'skill-discovery-index.minimal.json' })

foreach ($indexFile in $discoveryIndexes) {
    try {
        $indexContent = Get-Content -Raw -Path $indexFile.FullName | ConvertFrom-Json
    } catch {
        Add-Issue -Check 3 -Severity error -Message "Could not parse discovery index '$($indexFile.Name)': $_"
        continue
    }

    if ($null -eq $indexContent.entries) {
        Add-Issue -Check 3 -Severity warn -Message "Discovery index '$($indexFile.Name)' has no entries array"
        continue
    }

    foreach ($entry in $indexContent.entries) {
        $skillId = [string]$entry.skillId
        if ([string]::IsNullOrWhiteSpace($skillId)) {
            Add-Issue -Check 3 -Severity warn -Message "Discovery index '$($indexFile.Name)' has entry with missing skillId"
            continue
        }

        # Check both naming patterns
        $fixtureCandidates = @(
            "skill.$skillId.json",
            "skill.elegy-$skillId.json"
        )
        $found = $false
        foreach ($candidate in $fixtureCandidates) {
            $candidatePath = Join-Path $fixturesDir $candidate
            if (Test-Path $candidatePath) {
                $found = $true
                break
            }
        }

        # Also check if the skillId already starts with "elegy-"
        if (-not $found) {
            # Try without double prefix
            if ($skillId -like 'elegy-*') {
                $plainName = "skill.$skillId.json"
                $plainPath = Join-Path $fixturesDir $plainName
                if (Test-Path $plainPath) {
                    $found = $true
                }
            }
        }

        if (-not $found) {
            $msg = "Discovery index '$($indexFile.Name)' references skillId '$skillId' but no matching fixture found (tried: $($fixtureCandidates -join ', '))"
            $check3Issues.Add($msg)
            Add-Issue -Check 3 -Severity error -Message $msg
        }
    }
}

$checkResults['check3'] = @{
    name = 'Discovery indexes reference valid skill fixtures'
    indexesChecked = $discoveryIndexes.Count
    orphanEntries = $check3Issues.Count
    passed = ($check3Issues.Count -eq 0)
}

# =====================================================================
# CHECK 4: Manifest fixture references (wrapper surfaces)
# =====================================================================
$check4Issues = [System.Collections.Generic.List[string]]::new()

$manifestContent = Get-FileContent -RelativePath 'contracts/manifests/compatibility-manifest.json'
if ($null -eq $manifestContent) {
    Add-Issue -Check 4 -Severity error -Message "Cannot read contracts/manifests/compatibility-manifest.json"
} else {
    try {
        $manifest = $manifestContent | ConvertFrom-Json
    } catch {
        Add-Issue -Check 4 -Severity error -Message "Could not parse compatibility-manifest.json: $_"
    }

    if ($null -ne $manifest) {
        foreach ($family in $manifest.plannedFamilies) {
            $ws = $family.wrapperSurface
            if ($null -eq $ws) {
                continue  # Skip families without wrapperSurface (e.g., elegy-piloting-v1)
            }

            $familyName = [string]$family.name

            $root = [string]$ws.root
            $entrypoint = [string]$ws.entrypoint
            $skillBridge = [string]$ws.skillBridge

            if (-not [string]::IsNullOrWhiteSpace($root)) {
                $rootPath = Join-Path $RepoRoot $root
                if (-not (Test-Path $rootPath)) {
                    $msg = "Manifest family '$familyName': wrapperSurface.root '$root' does not exist"
                    $check4Issues.Add($msg)
                    Add-Issue -Check 4 -Severity error -Message $msg
                }
            }

            if (-not [string]::IsNullOrWhiteSpace($entrypoint)) {
                $entrypointPath = Join-Path $RepoRoot $entrypoint
                if (-not (Test-Path $entrypointPath)) {
                    $msg = "Manifest family '$familyName': wrapperSurface.entrypoint '$entrypoint' does not exist"
                    $check4Issues.Add($msg)
                    Add-Issue -Check 4 -Severity error -Message $msg
                }
            }

            if (-not [string]::IsNullOrWhiteSpace($skillBridge)) {
                $skillBridgePath = Join-Path $RepoRoot $skillBridge
                if (-not (Test-Path $skillBridgePath)) {
                    $msg = "Manifest family '$familyName': wrapperSurface.skillBridge '$skillBridge' does not exist"
                    $check4Issues.Add($msg)
                    Add-Issue -Check 4 -Severity error -Message $msg
                }
            }

            if ($ws.PSObject.Properties.Name -contains 'installer' -and $ws.installer) {
                $installerPath = Join-Path $RepoRoot $ws.installer
                if (-not (Test-Path -LiteralPath $installerPath)) {
                    $check4Issues++
                    Add-Issue -Check 4 -Severity error -Message "Missing wrapper-surface installer: $($ws.installer) for family '$($family.name)'"
                }
            }
        }
    }
}

$checkResults['check4'] = @{
    name = 'Manifest wrapper-surface path existence'
    issuesFound = $check4Issues.Count
    passed = ($check4Issues.Count -eq 0)
}

# =====================================================================
# CHECK 5: Canonical output inventory source files exist
# =====================================================================
$check5Issues = [System.Collections.Generic.List[string]]::new()

$inventoryContent = Get-FileContent -RelativePath 'governance/canonical-output-inventory.json'
if ($null -eq $inventoryContent) {
    Add-Issue -Check 5 -Severity error -Message "Cannot read governance/canonical-output-inventory.json"
} else {
    try {
        $inventory = $inventoryContent | ConvertFrom-Json
    } catch {
        Add-Issue -Check 5 -Severity error -Message "Could not parse canonical-output-inventory.json: $_"
    }

    if ($null -ne $inventory) {
        # Check authorityOnly entries
        foreach ($relativePath in $inventory.authorityOnly) {
            $fullPath = Join-Path $RepoRoot $relativePath
            if (-not (Test-Path $fullPath)) {
                $msg = "Canonical inventory: authority-only file '$relativePath' does not exist"
                $check5Issues.Add($msg)
                Add-Issue -Check 5 -Severity error -Message $msg
            }
        }

        # Check mirroredOutputs source files
        foreach ($entry in $inventory.mirroredOutputs) {
            $source = [string]$entry.source
            if ([string]::IsNullOrWhiteSpace($source)) {
                continue
            }
            $sourcePath = Join-Path $RepoRoot $source
            if (-not (Test-Path $sourcePath)) {
                $msg = "Canonical inventory: mirrored source '$source' does not exist"
                $check5Issues.Add($msg)
                Add-Issue -Check 5 -Severity error -Message $msg
            }
        }
    }
}

$checkResults['check5'] = @{
    name = 'Canonical output inventory source file existence'
    authorityOnlyChecked = if ($null -ne $inventory) { @($inventory.authorityOnly).Count } else { 0 }
    mirroredOutputsChecked = if ($null -ne $inventory) { @($inventory.mirroredOutputs).Count } else { 0 }
    missingFiles = $check5Issues.Count
    passed = ($check5Issues.Count -eq 0)
}

# =====================================================================
# CHECK 6: Boundary policy references valid files
# =====================================================================
$check6Issues = [System.Collections.Generic.List[string]]::new()

$policyContent = Get-FileContent -RelativePath 'governance/boundary-policy.json'
if ($null -eq $policyContent) {
    Add-Issue -Check 6 -Severity error -Message "Cannot read governance/boundary-policy.json"
} else {
    try {
        $policy = $policyContent | ConvertFrom-Json
    } catch {
        Add-Issue -Check 6 -Severity error -Message "Could not parse boundary-policy.json: $_"
    }

    if ($null -ne $policy) {
        if ($null -eq $policy.requiredFiles) {
            Add-Issue -Check 6 -Severity warn -Message "boundary-policy.json has no 'requiredFiles' property"
            return
        }

        foreach ($requiredFile in $policy.requiredFiles) {
            $rf = [string]$requiredFile
            if ([string]::IsNullOrWhiteSpace($rf)) {
                continue
            }
            $fullPath = Join-Path $RepoRoot $rf
            if (-not (Test-Path $fullPath)) {
                $msg = "Boundary policy: required file '$rf' does not exist"
                $check6Issues.Add($msg)
                Add-Issue -Check 6 -Severity error -Message $msg
            }
        }
    }
}

$checkResults['check6'] = @{
    name = 'Boundary policy required file existence'
    requiredFilesChecked = if ($null -ne $policy) { @($policy.requiredFiles).Count } else { 0 }
    missingFiles = $check6Issues.Count
    passed = ($check6Issues.Count -eq 0)
}

# =====================================================================
# Summary
# =====================================================================
$totalIssues = $issues.Count
$errorIssues = @($issues | Where-Object { $_.severity -eq 'error' })
$allPassed = ($errorIssues.Count -eq 0)

$summaryResult = [pscustomobject]@{
    registryAlignment = if ($allPassed) { 'PASS' } else { 'FAIL' }
    totalIssues = $totalIssues
    errorCount = $errorIssues.Count
    warningCount = ($totalIssues - $errorIssues.Count)
    checks = $checkResults
    issues = @($issues)
}

if ($Json) {
    $summaryResult | ConvertTo-Json -Depth 6
} else {
    Write-Host ""

    $checkKeys = @('check1', 'check2', 'check3', 'check4', 'check5', 'check6')
    foreach ($key in $checkKeys) {
        $chk = $checkResults[$key]
        if ($null -eq $chk) { continue }
        $statusText = if ($chk.passed) { 'PASS' } else { 'FAIL' }
        Write-Host "[$statusText] Check $key`: $($chk.name)"
    }

    Write-Host ""
    if ($issues.Count -gt 0) {
        Write-Host "Issues ($($issues.Count)):"
        foreach ($issue in $issues) {
            $sevLabel = if ($issue.severity -eq 'error') { 'ERROR' } else { 'WARN' }
            Write-Host "  [$sevLabel] Check $($issue.check): $($issue.message)"
        }
        Write-Host ""
    }

    if ($allPassed) {
        if ($totalIssues -gt 0) {
            Write-Host "Registry alignment: PASS (with $totalIssues advisory warning(s))"
        } else {
            Write-Host "Registry alignment: PASS"
        }
    } else {
        Write-Host "Registry alignment: FAIL ($($errorIssues.Count) error(s), $($totalIssues - $errorIssues.Count) warning(s))"
    }
}

# Exit code
if (-not $allPassed) {
    exit 1
}
exit 0
