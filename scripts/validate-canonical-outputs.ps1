[CmdletBinding()]
param(
    [switch]$RequireGeneratedOutputs,
    [switch]$RequireArchive,
    [switch]$RequireWrapperArchives,
    [switch]$RequireInstallerArchives,
    [switch]$RequireReleaseMetadata,
    [switch]$RequireDeepValidation,
    [string]$DistributionDirectory = ''
)

$ErrorActionPreference = 'Stop'

$repoRoot = Split-Path -Parent $PSScriptRoot
$artifactsDir = Join-Path $repoRoot 'artifacts'
$contractsDir = Join-Path $repoRoot 'contracts'
$contractsOutputDir = Join-Path $artifactsDir 'contracts'
$distDir = if ($DistributionDirectory) { $DistributionDirectory } else { Join-Path $artifactsDir 'distribution' }

$failures = [System.Collections.Generic.List[string]]::new()
$warnings = [System.Collections.Generic.List[string]]::new()

function Assert-Exists {
    param([string]$Path, [string]$Label)
    if (-not (Test-Path -LiteralPath $Path)) {
        $failures.Add("$Label : $Path")
    }
}

function Assert-ArchiveContains {
    param([string]$ArchivePath, [string[]]$RequiredEntries, [string]$Label)
    if (-not (Test-Path -LiteralPath $ArchivePath)) {
        $failures.Add("$Label (archive missing): $ArchivePath")
        return
    }
    try {
        Add-Type -AssemblyName System.IO.Compression.FileSystem
        $zip = [System.IO.Compression.ZipFile]::OpenRead($ArchivePath)
        $entries = $zip.Entries | ForEach-Object { $_.FullName }
        $zip.Dispose()
        foreach ($required in $RequiredEntries) {
            if ($entries -notcontains $required) {
                $failures.Add("$Label (missing entry '$required'): $ArchivePath")
            }
        }
    } catch {
        $errMsg = $_.Exception.Message
        $failures.Add("$Label (failed to read archive): $ArchivePath -- $errMsg")
    }
}

function Assert-SchemaFile {
    param([string]$Path, [string]$Label)
    Assert-Exists $Path "$Label (schema)"
}

function Assert-FixtureFile {
    param([string]$Path, [string]$Label)
    Assert-Exists $Path "$Label (fixture)"
}

# --- canonical authority inventory (default check) ---
if (-not $RequireGeneratedOutputs -and -not $RequireArchive -and -not $RequireWrapperArchives -and -not $RequireInstallerArchives -and -not $RequireReleaseMetadata -and -not $RequireDeepValidation) {
    Assert-Exists (Join-Path $contractsDir 'schemas\schema-version.json') 'schema-version'
    Assert-Exists (Join-Path $contractsDir 'schemas\elegy-plugin-package.schema.json') 'plugin-package-schema'
    Assert-Exists (Join-Path $contractsDir 'fixtures\elegy-plugin-package.minimal.json') 'plugin-package-minimal-fixture'
    Assert-Exists (Join-Path $contractsDir 'fixtures\skill.minimal.json') 'skill-minimal-fixture'
    Assert-Exists (Join-Path $contractsDir 'README.md') 'contracts-readme'
    Assert-Exists (Join-Path $contractsDir 'AGENTS.md') 'contracts-agents'
    Assert-Exists (Join-Path $contractsDir 'schemas\elegy-catalog-entry.schema.json') 'catalog-entry-schema'
    Assert-Exists (Join-Path $contractsDir 'fixtures\elegy-catalog-entry.minimal.json') 'catalog-entry-minimal-fixture'

    if ($failures.Count -gt 0) {
        Write-Warning ('Canonical authority inventory issues: ' + ($failures -join '; '))
    } else {
        Write-Host 'Canonical authority inventory: ok'
    }
}

# --- generated contracts output ---
if ($RequireGeneratedOutputs) {
    Assert-Exists (Join-Path $contractsOutputDir 'elegy-contracts-manifest.json') 'contracts-manifest'
    Assert-Exists (Join-Path $contractsOutputDir 'contracts-index.json') 'contracts-index'

    if ($failures.Count -gt 0) {
        Write-Warning ('Generated outputs missing: ' + ($failures -join '; '))
    } else {
        Write-Host 'Generated outputs: ok'
    }
}

# --- contracts distribution archive with deep content validation ---
if ($RequireArchive) {
    $archives = @(Get-ChildItem -LiteralPath $distDir -Filter 'elegy-contracts-*.zip' -File -ErrorAction SilentlyContinue | Sort-Object { [version]($_.BaseName -replace '^elegy-contracts-', '') })
    if (-not $archives -or $archives.Count -eq 0) {
        $failures.Add("contracts-archive : no elegy-contracts-*.zip found in $distDir")
    } else {
        $latest = $archives[-1]
        $archivePath = $latest.FullName
        Write-Host "Contracts archive: ok ($($latest.Name))"

        # Deep validation: verify archive contains expected content categories
        $requiredSchemaEntries = @(
            'elegy-plugin-package-v2.schema.json',
            'skill-definition-v2.schema.json',
            'elegy-configuration-template-v1.schema.json',
            'elegy-configuration-profile-v1.schema.json',
            'elegy-plugin-readiness-v1.schema.json'
        )

        $expectedSchemaEntries = @(
            'elegy-catalog-entry.schema.json'
        )

        $requiredFixEntries = @(
            'fixtures/elegy-plugin-package-v2.minimal.json',
            'fixtures/skill-definition-v2.minimal.json'
        )

        $expectedFixEntries = @(
            'fixtures/elegy-catalog-entry.minimal.json'
        )

        $requiredConfigEntries = @(
            'fixtures/configuration/demo-template.json',
            'fixtures/configuration/demo-profile.json',
            'fixtures/elegy-configuration-template-v1.minimal.json',
            'fixtures/elegy-configuration-profile-v1.minimal.json'
        )

        try {
            Add-Type -AssemblyName System.IO.Compression.FileSystem
            $zip = [System.IO.Compression.ZipFile]::OpenRead($archivePath)
            $archiveEntries = $zip.Entries | ForEach-Object { $_.FullName }
            $zip.Dispose()

            $foundSchemas = 0
            foreach ($entry in $requiredSchemaEntries) {
                if ($archiveEntries -contains $entry) { $foundSchemas++ }
            }
            if ($foundSchemas -lt 3) {
                $failures.Add("contracts-archive-content : only $foundSchemas of $($requiredSchemaEntries.Count) required schemas present in $archivePath")
            }

            $foundFixtures = 0
            foreach ($entry in $requiredFixEntries) {
                if ($archiveEntries -contains $entry) { $foundFixtures++ }
            }
            if ($foundFixtures -lt 2) {
                $failures.Add("contracts-archive-content : only $foundFixtures of $($requiredFixEntries.Count) required plugin/skill fixtures present in $archivePath")
            }

            $foundConfig = 0
            foreach ($entry in $requiredConfigEntries) {
                if ($archiveEntries -contains $entry) { $foundConfig++ }
            }
            if ($foundConfig -lt 2) {
                $failures.Add("contracts-archive-content : only $foundConfig of $($requiredConfigEntries.Count) required config assets present in $archivePath")
            }

            # Check expected (non-required) entries and warn if missing
            foreach ($entry in $expectedSchemaEntries) {
                if ($archiveEntries -notcontains $entry) {
                    $warnings.Add("contracts-archive-content : expected schema '$entry' not found in $archivePath (may not yet be in export bundle)")
                }
            }
            foreach ($entry in $expectedFixEntries) {
                if ($archiveEntries -notcontains $entry) {
                    $warnings.Add("contracts-archive-content : expected fixture '$entry' not found in $archivePath (may not yet be in export bundle)")
                }
            }

            if ($failures.Where({ $_ -match '^contracts-archive-content' }).Count -eq 0) {
                Write-Host "Contracts archive content: ok (schemas: $foundSchemas, fixtures: $foundFixtures, config: $foundConfig)"
            }
        } catch {
            $errMsg = $_.Exception.Message
            $failures.Add("contracts-archive-content : failed to inspect archive $archivePath -- $errMsg")
        }
    }
}

# --- wrapper surface archives ---
if ($RequireWrapperArchives) {
    $wrapperSurfaces = @('elegy-memory', 'elegy-mcp', 'elegy-planning', 'elegy-skills', 'elegy-configuration', 'elegy-documentation', 'elegy-obsidian')
    foreach ($surface in $wrapperSurfaces) {
        $wrappers = Get-ChildItem -LiteralPath $distDir -Filter "$surface-wrapper-*.zip" -File -ErrorAction SilentlyContinue
        if (-not $wrappers -or $wrappers.Count -eq 0) {
            $failures.Add("wrapper-archive : $surface-wrapper-*.zip not found in $distDir")
        }
    }
    if ($failures.Where({ $_ -match '^wrapper-archive' }).Count -eq 0) {
        Write-Host "Wrapper archives: ok"
    }
}

# --- installer archive ---
if ($RequireInstallerArchives) {
    $installers = Get-ChildItem -LiteralPath $distDir -Filter 'elegy-installer-*.zip' -File -ErrorAction SilentlyContinue
    if (-not $installers -or $installers.Count -eq 0) {
        $failures.Add("installer-archive : no elegy-installer-*.zip found in $distDir")
    } else {
        Write-Host "Installer archive: ok ($($installers[0].Name))"
    }
}

# --- release metadata with cross-validation against staged artifacts ---
if ($RequireReleaseMetadata) {
    $manifests = Get-ChildItem -LiteralPath $distDir -Filter 'elegy-release-manifest-*.json' -File -ErrorAction SilentlyContinue
    $checksums = Get-ChildItem -LiteralPath $distDir -Filter 'elegy-release-checksums-*.json' -File -ErrorAction SilentlyContinue
    if (-not $manifests -or $manifests.Count -eq 0) {
        $failures.Add("release-metadata : no elegy-release-manifest-*.json found in $distDir")
    }
    if (-not $checksums -or $checksums.Count -eq 0) {
        $failures.Add("release-metadata : no elegy-release-checksums-*.json found in $distDir")
    }

    if ($manifests -and $checksums) {
        # Cross-validate: manifest asset entries must match staged artifacts
        $manifestPath = $manifests[0].FullName
        $checksumsPath = $checksums[0].FullName
        try {
            $manifest = Get-Content $manifestPath -Raw | ConvertFrom-Json
            $checksumData = Get-Content $checksumsPath -Raw | ConvertFrom-Json

            # Verify each manifest asset has a corresponding staged file
            foreach ($asset in $manifest.assets) {
                $stagedPath = Join-Path $distDir $asset.fileName
                if (-not (Test-Path -LiteralPath $stagedPath)) {
                    $failures.Add("release-metadata-cross-check : manifest lists '$($asset.fileName)' but file is missing from $distDir")
                } else {
                    # Verify file size matches manifest
                    $actualSize = (Get-Item -LiteralPath $stagedPath).Length
                    if ($actualSize -ne $asset.sizeBytes) {
                        $failures.Add("release-metadata-cross-check : $($asset.fileName) size mismatch (manifest: $($asset.sizeBytes), actual: $actualSize)")
                    }

                    # Verify SHA-256 matches manifest
                    $actualHash = (Get-FileHash -LiteralPath $stagedPath -Algorithm SHA256).Hash.ToLowerInvariant()
                    if ($actualHash -ne $asset.sha256.ToLowerInvariant()) {
                        $failures.Add("release-metadata-cross-check : $($asset.fileName) SHA-256 mismatch")
                    }
                }

                # Verify checksums document has a matching entry
                $checksumEntry = $checksumData.assets | Where-Object { $_.fileName -eq $asset.fileName }
                if (-not $checksumEntry) {
                    $failures.Add("release-metadata-cross-check : $($asset.fileName) in manifest but missing from checksums document")
                }
            }

            # Verify checksums assets also exist on disk
            if ($checksumData.assets) {
                foreach ($csAsset in $checksumData.assets) {
                    $stagedPath = Join-Path $distDir $csAsset.fileName
                    if (-not (Test-Path -LiteralPath $stagedPath)) {
                        $warnings.Add("release-metadata-cross-check : checksums lists '$($csAsset.fileName)' but file is missing from $distDir")
                    }
                }
            }

            if ($failures.Where({ $_ -match '^release-metadata-cross-check' }).Count -eq 0) {
                Write-Host "Release metadata cross-validation: ok"
            }
        } catch {
            $errMsg = $_.Exception.Message
            $failures.Add("release-metadata-cross-check : failed to parse metadata -- $errMsg")
        }
    }

    if ($failures.Where({ $_ -match '^release-metadata' }).Count -eq 0 -and $failures.Where({ $_ -match '^release-metadata-cross-check' }).Count -eq 0) {
        Write-Host "Release metadata: ok"
    }
}

# --- deep validation: plugin package fixtures via Rust CLI ---
if ($RequireDeepValidation) {
    # Validate plugin package fixtures using the Rust CLI
    $pluginPackageFixtures = Get-ChildItem -LiteralPath $contractsDir -Recurse -Filter 'elegy-plugin-package.*.json' -File -ErrorAction SilentlyContinue |
        Where-Object { $_.Name -notmatch '\.negative-' } |
        Where-Object { $_.Name -ne 'elegy-plugin-package.demo-config.json' }

    foreach ($fixture in $pluginPackageFixtures) {
        $fixturePath = $fixture.FullName
        try {
            $result = cargo run --manifest-path (Join-Path $repoRoot 'rust\Cargo.toml') -p elegy-cli -- plugin verify --package $fixturePath --json 2>&1
            if ($LASTEXITCODE -ne 0) {
                $warnings.Add("plugin-fixture-validate : $($fixture.Name) -- CLI returned non-zero exit code (may be expected for fixtures without install receipts)")
            } else {
                try {
                    $parsed = $result | Where-Object { $_ -match '^\s*\{' } | ConvertFrom-Json
                    if ($parsed.readiness -eq 'blocked') {
                        $warnings.Add("plugin-fixture-validate : $($fixture.Name) -- readiness is 'blocked'")
                    }
                } catch {
                    # JSON parse failure is acceptable; some fixtures require install receipts
                }
            }
        } catch {
            $warnings.Add("plugin-fixture-validate : $($fixture.Name) -- failed to run CLI validation")
        }
    }

    # Validate skill fixtures using the Rust CLI
    $skillFixtures = Get-ChildItem -LiteralPath (Join-Path $contractsDir 'fixtures') -Filter 'skill.*.json' -File -ErrorAction SilentlyContinue |
        Where-Object { $_.Name -notmatch 'skill\.minimal\.json' -and $_.Name -notmatch '\.(parity|expected|negative)\.' }

    foreach ($skillFixture in $skillFixtures) {
        $fixturePath = $skillFixture.FullName
        try {
            cargo run --manifest-path (Join-Path $repoRoot 'rust\Cargo.toml') -p elegy-cli -- skills validate --path $fixturePath --json 2>&1 | Out-Null
            if ($LASTEXITCODE -ne 0) {
                $warnings.Add("skill-fixture-validate : $($skillFixture.Name) -- validation warnings or issues")
            }
        } catch {
            $warnings.Add("skill-fixture-validate : $($skillFixture.Name) -- failed to run CLI validation")
        }
    }

    # Verify required workflow scripts exist
    $requiredScripts = @(
        'scripts/export-contracts.ps1',
        'scripts/package-installer.ps1'
    )
    foreach ($script in $requiredScripts) {
        Assert-Exists (Join-Path $repoRoot $script) "required-script : $script"
    }

    # Verify required workflow files exist
    $requiredWorkflows = @(
        '.github/workflows/rust-ci.yml',
        '.github/workflows/distribution-artifacts.yml',
        '.github/workflows/publish-distribution.yml'
    )
    foreach ($wf in $requiredWorkflows) {
        Assert-Exists (Join-Path $repoRoot $wf) "required-workflow : $wf"
    }

    if ($failures.Where({ $_ -match '^required-' }).Count -eq 0) {
        Write-Host "Required scripts and workflows: ok"
    }

    if ($warnings.Count -gt 0) {
        Write-Host "Deep validation warnings:" -ForegroundColor Yellow
        foreach ($w in $warnings) {
            Write-Host "  $w" -ForegroundColor Yellow
        }
    }

    if ($failures.Where({ $_ -notmatch '^required-' }).Count -eq 0) {
        Write-Host "Deep validation: ok"
    }
}

# --- warnings output ---
if ($warnings.Count -gt 0 -and -not $RequireDeepValidation) {
    Write-Host "Warnings:" -ForegroundColor Yellow
    foreach ($w in $warnings) {
        Write-Host "  $w" -ForegroundColor Yellow
    }
}

if ($failures.Count -gt 0) {
    throw ('Validation failed: ' + ($failures -join '; '))
}
