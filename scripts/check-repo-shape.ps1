param(
    [string]$Project = ".",
    [switch]$Json,
    [switch]$FailOnIssues
)

$ErrorActionPreference = "Stop"

$root = (Resolve-Path -LiteralPath $Project).Path
$issues = New-Object System.Collections.Generic.List[object]

function Add-Issue {
    param(
        [string]$Code,
        [string]$Path,
        [string]$Message,
        [string]$Severity = "warning"
    )

    $issues.Add([ordered]@{
        code = $Code
        path = $Path.Replace("\", "/")
        severity = $Severity
        message = $Message
    })
}

function Test-RelativePath {
    param([string]$Path)
    Test-Path -LiteralPath (Join-Path $root $Path)
}

function Get-TrackedFiles {
    $gitOutput = & git -C $root ls-files
    if ($LASTEXITCODE -ne 0) {
        throw "git ls-files failed"
    }

    $gitOutput | Where-Object {
        $_ -and (Test-Path -LiteralPath (Join-Path $root $_))
    }
}

$trackedFiles = @(Get-TrackedFiles)

foreach ($file in $trackedFiles) {
    if ($file -match '(\.db|\.sqlite|\.sqlite3|\.db-wal|\.db-shm|\.db-journal)$') {
        Add-Issue "artifacts.local_database" $file "Tracked local database or SQLite state file."
    }

    if ($file -match 'Users.*AppData.*Temp|^C[:\\/]') {
        Add-Issue "artifacts.literal_local_path" $file "Tracked file path looks like a local absolute-path artifact."
    }
}

$windowsUsersPathPattern = 'C:' + '[/\\]' + 'Users'
$appDataPathPattern = 'AppData' + '[/\\]' + 'Local'
$macUsersPathPattern = '/' + 'Users' + '/[^/\s]+'
$linuxHomePathPattern = '/' + 'home' + '/[^/\s]+'
$localPathPattern = @(
    $windowsUsersPathPattern,
    $appDataPathPattern,
    $macUsersPathPattern,
    $linuxHomePathPattern
) -join '|'
foreach ($file in $trackedFiles) {
    $fullPath = Join-Path $root $file
    $item = Get-Item -LiteralPath $fullPath -ErrorAction SilentlyContinue
    if (-not $item -or $item.PSIsContainer) {
        continue
    }
    if ($item.Length -gt 1048576) {
        continue
    }

    $text = Get-Content -LiteralPath $fullPath -Raw -ErrorAction SilentlyContinue
    $match = [regex]::Match($text, $localPathPattern)
    if ($match.Success) {
        Add-Issue "content.local_path" $file "Tracked file contains a local absolute path token: $($match.Value)"
    }
}

if (Test-RelativePath ".cargo/config.toml") {
    Add-Issue "cargo.active_config" ".cargo/config.toml" "Active Cargo config should stay local; commit .cargo/config.example.toml instead."
}

if (Test-RelativePath "cargo/config.toml") {
    Add-Issue "cargo.nonstandard_config" "cargo/config.toml" "Non-dot cargo/config.toml is not Cargo's standard project config path."
}

if (Test-RelativePath "package.json") {
    $packageJson = Get-Content -LiteralPath (Join-Path $root "package.json") -Raw | ConvertFrom-Json
    if (-not $packageJson.scripts -and -not $packageJson.dependencies -and -not $packageJson.devDependencies) {
        Add-Issue "node.empty_root_package" "package.json" "Root package.json has no scripts or dependencies."
    }
}

$surfacesPath = Join-Path $root "distribution/surfaces.json"
if (Test-Path -LiteralPath $surfacesPath) {
    $surfacesCatalog = Get-Content -LiteralPath $surfacesPath -Raw | ConvertFrom-Json
    if ($surfacesCatalog.schemaVersion -ne "elegy-surfaces/v3") {
        Add-Issue "surfaces.schema_version" "distribution/surfaces.json" "Surface catalog must declare schemaVersion elegy-surfaces/v3."
    }
    $surfaces = $surfacesCatalog.surfaces
    $allowedSurfaceKinds = @("bundled-plugin", "cli", "host-adapter", "skill-package", "external-plugin-wrapper")
    foreach ($surface in $surfaces) {
        if (-not $surface.kind -or $allowedSurfaceKinds -notcontains $surface.kind) {
            Add-Issue "surfaces.invalid_kind" "distribution/surfaces.json" "Surface '$($surface.name)' must use one of: $($allowedSurfaceKinds -join ', ')."
        }
        $allowedSurfaceClasses = @("adapter-plugin", "tool", "skill", "host-adapter", "host-extension")
        if (-not $surface.surfaceClass -or $allowedSurfaceClasses -notcontains $surface.surfaceClass) {
            Add-Issue "surfaces.invalid_class" "distribution/surfaces.json" "Surface '$($surface.name)' must declare a surfaceClass from: $($allowedSurfaceClasses -join ', ')."
        }
        $allowedLifecycles = @("active", "rework", "deprecated", "blocked")
        if (-not $surface.lifecycle -or $allowedLifecycles -notcontains $surface.lifecycle) {
            Add-Issue "surfaces.invalid_lifecycle" "distribution/surfaces.json" "Surface '$($surface.name)' must declare a lifecycle from: $($allowedLifecycles -join ', ')."
        }
        if ($surface.packaging -eq "plugin" -and ($surface.surfaceClass -ne "adapter-plugin" -or $surface.lifecycle -ne "active")) {
            Add-Issue "surfaces.invalid_plugin_boundary" "distribution/surfaces.json" "Only active adapter-plugin surfaces may set packaging to plugin."
        }

        if ($surface.pluginRoot -and -not (Test-RelativePath $surface.pluginRoot)) {
            Add-Issue "surfaces.missing_plugin_root" $surface.pluginRoot "distribution/surfaces.json references a missing pluginRoot."
        }

        if ($surface.crateRoot -and -not (Test-RelativePath $surface.crateRoot)) {
            Add-Issue "surfaces.missing_crate_root" $surface.crateRoot "distribution/surfaces.json references a missing crateRoot."
        }

        if ($surface.skillRoot -and -not (Test-RelativePath $surface.skillRoot)) {
            Add-Issue "surfaces.missing_skill_root" $surface.skillRoot "distribution/surfaces.json references a missing skillRoot."
        }

        switch ($surface.kind) {
            "bundled-plugin" {
                if (-not $surface.pluginRoot -or -not $surface.pluginRoot.StartsWith("plugins/")) {
                    Add-Issue "surfaces.bundled_plugin_root" "distribution/surfaces.json" "Bundled plugin '$($surface.name)' must use pluginRoot under plugins/."
                }
            }
            "skill-package" {
                if (-not $surface.skillRoot -or -not $surface.skillRoot.StartsWith("skills/elegy-")) {
                    Add-Issue "surfaces.skill_package_root" "distribution/surfaces.json" "Skill package '$($surface.name)' must use skillRoot under skills/elegy-*."
                }
            }
            "external-plugin-wrapper" {
                if (-not $surface.pluginRoot -or -not $surface.pluginRoot.StartsWith("marketplace-wrappers/")) {
                    Add-Issue "surfaces.wrapper_root" "distribution/surfaces.json" "External wrapper '$($surface.name)' must use pluginRoot under marketplace-wrappers/."
                }
            }
            "host-adapter" {
                if (-not $surface.crateRoot -or -not $surface.crateRoot.StartsWith("hosts/")) {
                    Add-Issue "surfaces.host_adapter_root" "distribution/surfaces.json" "Host adapter '$($surface.name)' must declare crateRoot under hosts/."
                }
            }
        }
    }

    $surfacePackages = New-Object System.Collections.Generic.HashSet[string]
    $surfaceRoots = New-Object System.Collections.Generic.HashSet[string]
    foreach ($surface in $surfaces) {
        if ($surface.package) {
            [void]$surfacePackages.Add([string]$surface.package)
        }
        else {
            [void]$surfacePackages.Add([string]$surface.name)
        }
        if ($surface.crateRoot) {
            [void]$surfaceRoots.Add(([string]$surface.crateRoot).Replace("\", "/"))
        }
        if ($surface.pluginRoot -and ([string]$surface.kind -in @("bundled-plugin", "cli"))) {
            [void]$surfaceRoots.Add(([string]$surface.pluginRoot).Replace("\", "/"))
        }
    }

    $metadataRaw = & cargo metadata --format-version 1 --no-deps 2>$null
    if ($LASTEXITCODE -eq 0) {
        $metadata = $metadataRaw | ConvertFrom-Json
        foreach ($package in $metadata.packages) {
            $manifestPath = [string]$package.manifest_path
            $packageDir = Split-Path -Parent $manifestPath
            $relativePackageDir = [System.IO.Path]::GetRelativePath($root, $packageDir).Replace("\", "/")
            if ($relativePackageDir -match '^(plugins|tools|hosts)/') {
                $packageName = [string]$package.name
                $coveredByPluginRoot = $false
                foreach ($surface in $surfaces) {
                    if ($surface.kind -ne "bundled-plugin" -or -not $surface.pluginRoot) { continue }
                    $pluginRoot = ([string]$surface.pluginRoot).Replace("\", "/").TrimEnd("/")
                    if ($relativePackageDir -eq $pluginRoot -or $relativePackageDir.StartsWith("$pluginRoot/")) {
                        $coveredByPluginRoot = $true
                        break
                    }
                }
                if (-not $surfacePackages.Contains($packageName) -and -not $coveredByPluginRoot) {
                    Add-Issue "surfaces.missing_package" $relativePackageDir "Rust package '$packageName' under plugins/, tools/, or hosts/ is missing from distribution/surfaces.json."
                }
                if (-not $surfaceRoots.Contains($relativePackageDir) -and -not $coveredByPluginRoot) {
                    Add-Issue "surfaces.missing_crate_root_entry" $relativePackageDir "Rust package '$packageName' should be represented by crateRoot or pluginRoot in distribution/surfaces.json."
                }
            }
        }
    }
    else {
        Add-Issue "surfaces.metadata_unavailable" "Cargo.toml" "Could not run cargo metadata to verify workspace/catalog consistency."
    }
}

if ($surfaces) {
    foreach ($surface in $surfaces) {
        if ($surface.packaging -ne "plugin") { continue }
        $manifest = Join-Path $root "$($surface.pluginRoot)/.elegy-plugin/plugin.json"
        if (-not (Test-Path -LiteralPath $manifest)) {
            Add-Issue "plugins.missing_manifest" $surface.pluginRoot "Active adapter plugin '$($surface.name)' is missing .elegy-plugin/plugin.json."
            continue
        }
        $pluginManifest = Get-Content -LiteralPath $manifest -Raw | ConvertFrom-Json
        $capabilityCatalog = $pluginManifest.elegy.capabilityCatalog
        if (-not $capabilityCatalog) {
            # Keep accepting the legacy top-level location when it contains a
            # catalog object, while treating the v3 nested Elegy field as
            # authoritative.
            $legacyCatalog = $pluginManifest.capabilityCatalog
            if ($legacyCatalog -is [pscustomobject] -and $legacyCatalog.PSObject.Properties.Count -gt 0) {
                $capabilityCatalog = $legacyCatalog
            }
        }
        if (-not ($capabilityCatalog -is [pscustomobject]) -or $capabilityCatalog.PSObject.Properties.Count -eq 0) {
            Add-Issue "plugins.missing_capability_boundary" $surface.pluginRoot "Active adapter plugin '$($surface.name)' must declare capabilityCatalog."
        }
    }
}

$mode = if ($FailOnIssues) { "blocking" } else { "report" }
$issueArray = $issues.ToArray()

$report = [pscustomobject]@{
    schemaVersion = "elegy-repo-shape-report/v1"
    project = $root
    generatedAt = (Get-Date).ToUniversalTime().ToString("o")
    mode = $mode
    issueCount = $issues.Count
    issues = $issueArray
}

if ($Json) {
    $report | ConvertTo-Json -Depth 8
}
else {
    "Repo shape report: $($issues.Count) issue(s)"
    foreach ($issue in $issues) {
        "- [$($issue.severity)] $($issue.code): $($issue.path) - $($issue.message)"
    }
}

if ($FailOnIssues -and $issues.Count -gt 0) {
    exit 1
}
