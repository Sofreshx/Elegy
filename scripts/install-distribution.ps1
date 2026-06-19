#!/usr/bin/env pwsh
# install-distribution.ps1 - thin PowerShell shim that delegates to install-distribution.sh
# via bash. The canonical installer logic lives in install-distribution.sh; this file exists
# only to preserve a native-pwsh entry point for Windows users. Requires bash in PATH
# (Git Bash, WSL, or Windows 10+ bash interop).
#
# Usage: pwsh ./scripts/install-distribution.ps1 -Tag v1.3.2 -Destination ./tools/elegy -CliSurfaces elegy-cli -Force
#
# All arguments are forwarded to the bash script unchanged. To add new install options,
# edit install-distribution.sh; this shim is a stable facade only.

[CmdletBinding()]
param(
    [Parameter(ValueFromRemainingArguments = $true)]
    [string[]]$ForwardArgs
)

$ErrorActionPreference = 'Stop'

$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$bashScript = Join-Path $scriptDir 'install-distribution.sh'

if (-not (Test-Path -LiteralPath $bashScript)) {
    throw "Canonical installer not found: $bashScript"
}

if (-not (Get-Command bash -ErrorAction SilentlyContinue)) {
    throw "bash not found in PATH. Install Git Bash, WSL, or Windows 10+ bash interop, then retry. The canonical installer at $bashScript is the only implementation; this shim does not duplicate its logic."
}

& bash $bashScript @ForwardArgs
exit $LASTEXITCODE
