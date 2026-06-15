<#
.SYNOPSIS
Configures the Elegy repo to use .githooks/ for Git hooks.

.DESCRIPTION
Sets git config core.hooksPath to .githooks (relative to repo root).
Verifies the .githooks/ directory exists with pre-commit and pre-push scripts.
#>

$ErrorActionPreference = 'Stop'

param(
    [string]$RepoRoot = (Split-Path -Parent $PSScriptRoot)
)

$hooksDir = Join-Path $RepoRoot ".githooks"
$preCommit = Join-Path $hooksDir "pre-commit"
$prePush = Join-Path $hooksDir "pre-push"

# Verify hooks directory exists
if (-not (Test-Path $hooksDir)) {
    Write-Error ".githooks directory not found at: $hooksDir"
    exit 1
}

# Verify hook scripts exist
if (-not (Test-Path $preCommit)) {
    Write-Error "pre-commit hook not found at: $preCommit"
    exit 1
}
if (-not (Test-Path $prePush)) {
    Write-Error "pre-push hook not found at: $prePush"
    exit 1
}

# Set core.hooksPath
Push-Location $RepoRoot
try {
    git config core.hooksPath .githooks
    Write-Host "core.hooksPath set to '.githooks'"

    $currentPath = git config core.hooksPath
    Write-Host "Verified: core.hooksPath = '$currentPath'"

    Write-Host ""
    Write-Host "Git hooks configured successfully!"
    Write-Host "  pre-commit: registry-alignment validation"
    Write-Host "  pre-push: registry-alignment + cargo fmt + cargo clippy"
} finally {
    Pop-Location
}
