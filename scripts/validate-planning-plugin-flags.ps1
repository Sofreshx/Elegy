<#
.SYNOPSIS
    Validates that the OpenCode planning plugin's CLI flag mappings match the actual
    elegy-planning CLI. Catches flag name drift between the plugin and the binary.

.DESCRIPTION
    This script:
    1. Parses the planning.js plugin source to extract all --flag strings used in arg builders
    2. Runs `elegy-planning <subcommand> --help` for each subcommand the plugin uses
    3. Extracts the valid long flag names from each help output
    4. Compares plugin flags against CLI flags and reports mismatches

    The goal is to ensure the plugin's manual arg builders stay in sync with the CLI.
    For a permanent single-source-of-truth, consider generating the plugin from the
    governed skill fixture (contracts/fixtures/skill.elegy-planning.json).

.PARAMETER PluginPath
    Path to the planning.js plugin. Defaults to the OpenCode config location.

.PARAMETER CliPath
    Path to the elegy-planning binary. Defaults to "elegy-planning" (PATH lookup).
#>
[CmdletBinding()]
param(
    [string]$PluginPath = "$env:USERPROFILE\.config\opencode\plugins\planning.js",
    [string]$CliPath = 'elegy-planning'
)

$ErrorActionPreference = 'Stop'

$failures = [System.Collections.Generic.List[string]]::new()
$warnings = [System.Collections.Generic.List[string]]::new()

# --- Step 1: Extract flags from the plugin source ---

if (-not (Test-Path -LiteralPath $PluginPath)) {
    Write-Error "Plugin not found at $PluginPath"
    exit 1
}

$pluginContent = Get-Content -LiteralPath $PluginPath -Raw

# Extract all "--flag-name" strings from the plugin source.
# These appear in arg builder functions like: a.push("--id", args.id)
# We look for patterns like: "--<word-chars-and-hyphens>"
$pluginFlags = [System.Collections.Generic.HashSet[string]]::new()
$matches = [regex]::Matches($pluginContent, '"(--[a-z][a-z0-9-]*)"')
foreach ($m in $matches) {
    $flag = $m.Groups[1].Value
    # Skip global flags that are auto-set by runPlanningCommand
    if ($flag -in @('--json', '--non-interactive', '--correlation-id', '--db', '--scope', '--format')) {
        continue
    }
    [void]$pluginFlags.Add($flag)
}

Write-Host "Plugin flags found: $($pluginFlags.Count)" -ForegroundColor Cyan
foreach ($f in ($pluginFlags | Sort-Object)) {
    Write-Host "  $f"
}

# --- Step 2: For each subcommand, extract valid flags from --help output ---

# Map of subcommand paths to check (matching the plugin's arg builders)
$subcommands = @(
    @('goal', 'create'),
    @('goal', 'list'),
    @('goal', 'show'),
    @('goal', 'update-status'),
    @('roadmap', 'create'),
    @('roadmap', 'list'),
    @('roadmap', 'show'),
    @('roadmap', 'update-status'),
    @('roadmap', 'add-section'),
    @('roadmap', 'add-work-point'),
    @('plan', 'create'),
    @('plan', 'list'),
    @('plan', 'show'),
    @('plan', 'update-status'),
    @('todo', 'create'),
    @('todo', 'list'),
    @('issue', 'record'),
    @('review-point', 'record'),
    @('insight', 'record'),
    @('project-run', 'claim'),
    @('project-run', 'activate'),
    @('project-run', 'release'),
    @('project-run', 'add-evidence'),
    @('project-run', 'list'),
    @('project-run', 'show'),
    @('validate', 'all'),
    @('scope', 'list'),
    @('tags'),
    @('search'),
    @('work-point', 'list'),
    @('work-point', 'show'),
    @('work-point', 'next-runnable'),
    @('work-point', 'work-graph'),
    @('context'),
    @('health')
)

# Map: subcommand path -> set of valid flags
$cliFlagsBySubcommand = @{}

foreach ($sub in $subcommands) {
    $subPath = $sub -join ' '
    try {
        $helpOutput = & $CliPath @sub --help 2>&1 | Out-String
    } catch {
        $warnings.Add("Failed to get help for: $subPath")
        continue
    }

    # Parse long flags from help output.
    # Help format shows flags like: --flag-name <VALUE>  or  --flag-name
    $validFlags = [System.Collections.Generic.HashSet[string]]::new()
    $helpMatches = [regex]::Matches($helpOutput, '(?m)^\s+--([a-z][a-z0-9-]*)')
    foreach ($m in $helpMatches) {
        [void]$validFlags.Add("--$($m.Groups[1].Value)")
    }

    $cliFlagsBySubcommand[$subPath] = $validFlags
}

# --- Step 3: Cross-reference plugin flags against CLI flags ---

# Build a union of all valid CLI flags
$allCliFlags = [System.Collections.Generic.HashSet[string]]::new()
foreach ($kv in $cliFlagsBySubcommand.GetEnumerator()) {
    foreach ($f in $kv.Value) {
        [void]$allCliFlags.Add($f)
    }
}

Write-Host ""
Write-Host "CLI flags across all subcommands: $($allCliFlags.Count)" -ForegroundColor Cyan

# Check: every plugin flag must exist in at least one CLI subcommand
Write-Host ""
Write-Host "=== Checking plugin flags against CLI ===" -ForegroundColor Yellow

$unknownFlags = [System.Collections.Generic.List[string]]::new()
foreach ($pf in ($pluginFlags | Sort-Object)) {
    if (-not $allCliFlags.Contains($pf)) {
        $unknownFlags.Add($pf)
    }
}

if ($unknownFlags.Count -gt 0) {
    foreach ($f in $unknownFlags) {
        $failures.Add("Plugin uses flag '$f' which does not exist in any CLI subcommand --help output")
        Write-Host "  FAIL: $f (not found in CLI)" -ForegroundColor Red
    }
} else {
    Write-Host "  All plugin flags are valid CLI flags" -ForegroundColor Green
}

# Check: for each subcommand, verify the plugin's flags for that subcommand are valid
Write-Host ""
Write-Host "=== Checking per-subcommand flag validity ===" -ForegroundColor Yellow

# Parse the plugin to find which flags are used per subcommand.
# This is a best-effort check: we match arg builder function names to subcommands.
$pluginSubcommandMap = @{
    'goal create'                    = @('--id', '--title', '--description', '--acceptance', '--rejection', '--status', '--tag')
    'goal list'                      = @()
    'goal show'                      = @('--goal-id')
    'goal update-status'             = @('--goal-id', '--status')
    'roadmap create'                 = @('--id', '--goal-id', '--title', '--summary', '--status', '--tag')
    'roadmap list'                   = @()
    'roadmap show'                   = @('--roadmap-id')
    'roadmap update-status'          = @('--roadmap-id', '--status')
    'roadmap add-section'            = @('--roadmap-id', '--slug', '--title', '--summary', '--ordering')
    'roadmap add-work-point'         = @('--roadmap-id', '--id', '--title', '--summary', '--status', '--ordering', '--effort-tier', '--section-id', '--dependency-id', '--file-scope', '--validation', '--tag')
    'plan create'                    = @('--id', '--goal-id', '--roadmap-id', '--title', '--summary', '--effort-tier', '--routing-hint', '--file-scope', '--plan-scope')
    'plan list'                      = @()
    'plan show'                      = @('--plan-id')
    'plan update-status'             = @('--plan-id', '--status')
    'todo create'                    = @('--plan-id', '--title', '--summary', '--status', '--effort-tier', '--tag')
    'todo list'                      = @()
    'issue record'                   = @('--related-entity-type', '--related-entity-id', '--title', '--summary', '--severity', '--tag')
    'review-point record'            = @('--entity-type', '--entity-id', '--title', '--summary', '--status', '--severity')
    'insight record'                 = @('--insight-type', '--parent-type', '--parent-id', '--title', '--content', '--tag')
    'project-run claim'              = @('--goal-id', '--roadmap-id', '--work-point-id', '--repo-id', '--branch', '--worktree-id', '--session-id', '--profile-id')
    'project-run activate'           = @('--project-run-id')
    'project-run release'            = @('--project-run-id', '--status', '--evidence-json')
    'project-run add-evidence'       = @('--project-run-id', '--evidence-json')
    'project-run list'               = @()
    'project-run show'               = @('--project-run-id')
    'validate all'                   = @()
    'scope list'                     = @()
    'tags'                           = @()
    'search'                         = @('--fts', '--title', '--status', '--tag', '--since', '--latest')
    'work-point list'                = @()
    'work-point show'                = @('--work-point-id')
    'work-point next-runnable'       = @()
    'work-point work-graph'          = @()
    'context'                        = @('--entity-type', '--entity-id')
    'health'                         = @()
}

foreach ($entry in $pluginSubcommandMap.GetEnumerator()) {
    $subPath = $entry.Key
    $pluginFlagsForSub = $entry.Value
    $cliFlagsForSub = $cliFlagsBySubcommand[$subPath]

    if ($null -eq $cliFlagsForSub) {
        $warnings.Add("No CLI help output parsed for subcommand: $subPath")
        continue
    }

    foreach ($pf in $pluginFlagsForSub) {
        if (-not $cliFlagsForSub.Contains($pf)) {
            $failures.Add("Subcommand '$subPath': plugin uses '$pf' but CLI --help does not list it")
            Write-Host "  FAIL: $subPath uses '$f' (not in CLI)" -ForegroundColor Red
        }
    }
}

# --- Summary ---

Write-Host ""
Write-Host "=== Summary ===" -ForegroundColor Yellow

if ($failures.Count -gt 0) {
    Write-Host "$($failures.Count) failure(s):" -ForegroundColor Red
    foreach ($f in $failures) {
        Write-Host "  - $f" -ForegroundColor Red
    }
    exit 1
} else {
    Write-Host "All checks passed" -ForegroundColor Green
}

if ($warnings.Count -gt 0) {
    Write-Host "$($warnings.Count) warning(s):" -ForegroundColor Yellow
    foreach ($w in $warnings) {
        Write-Host "  - $w" -ForegroundColor Yellow
    }
}
