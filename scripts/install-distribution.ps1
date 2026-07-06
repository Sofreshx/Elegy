#!/usr/bin/env pwsh
# install-distribution.ps1 - thin PowerShell shim that delegates to install-distribution.sh
# via bash. The canonical installer logic lives in install-distribution.sh; this file exists
# only to preserve a native-pwsh entry point for Windows users. Requires bash in PATH
# (Git Bash, WSL, or Windows 10+ bash interop).
#
# Usage: pwsh ./scripts/install-distribution.ps1 -Tag v1.3.2 -Destination ./tools/elegy -Surface elegy-planning -Force
#
# All arguments are forwarded to the bash script unchanged. To add new install options,
# edit install-distribution.sh; this shim is a stable facade only.

[CmdletBinding()]
param(
    [string]$Tag,
    [string]$Destination,
    [Alias('CliSurface')]
    [string]$Surface,
    [switch]$Force,
    [Parameter(ValueFromRemainingArguments = $true)]
    [string[]]$ForwardArgs
)

$ErrorActionPreference = 'Stop'

$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$bashScript = Join-Path $scriptDir 'install-distribution.sh'

if (-not (Test-Path -LiteralPath $bashScript)) {
    throw "Canonical installer not found: $bashScript"
}

$bashCommand = Get-Command bash -ErrorAction SilentlyContinue
if (-not $bashCommand) {
    throw "bash not found in PATH. Install Git Bash, WSL, or Windows 10+ bash interop, then retry. The canonical installer at $bashScript is the only implementation; this shim does not duplicate its logic."
}
$bashArgs = @()
if ($Tag) {
    $bashArgs += @('--tag', $Tag)
}
if ($Destination) {
    $bashArgs += @('--destination', $Destination)
}
if ($Surface) {
    $bashArgs += @('--surface', $Surface)
}
if ($Force.IsPresent) {
    $bashArgs += '--force'
}
if ($ForwardArgs) {
    $bashArgs += $ForwardArgs
}

$bashScriptPath = $bashScript
if ($bashScriptPath -match '^([A-Za-z]):\\(.*)$') {
    $drive = $Matches[1].ToLowerInvariant()
    $rest = $Matches[2] -replace '\\', '/'
    if ($bashCommand.Source -like '*\system32\bash.exe') {
        $bashScriptPath = "/mnt/$drive/$rest"
    } else {
        $bashScriptPath = "/$drive/$rest"
    }
}

& bash $bashScriptPath @bashArgs
exit $LASTEXITCODE
