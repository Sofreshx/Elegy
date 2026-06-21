[CmdletBinding()]
param(
    [string]$OutputPath = '',
    [switch]$CreateArchive,
    [string]$ArchiveOutputPath = ''
)

$ErrorActionPreference = 'Stop'

$repoRoot = Split-Path -Parent $PSScriptRoot

$cargoArgs = @(
    'run',
    '--manifest-path', (Join-Path $repoRoot 'rust\Cargo.toml'),
    '-p', 'elegy-cli',
    '--',
    'contracts', 'export'
)

if (-not [string]::IsNullOrWhiteSpace($OutputPath)) {
    $cargoArgs += @('--output-path', $OutputPath)
}

if ($CreateArchive) {
    $cargoArgs += '--create-archive'
}

if (-not [string]::IsNullOrWhiteSpace($ArchiveOutputPath)) {
    $cargoArgs += @('--archive-output-path', $ArchiveOutputPath)
}

& cargo @cargoArgs
if ($LASTEXITCODE -ne 0) {
    exit $LASTEXITCODE
}
