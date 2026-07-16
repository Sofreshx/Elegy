param([Parameter(Mandatory = $true)][string]$Source, [string]$InstallRoot = (Join-Path $env:LOCALAPPDATA 'Elegy\Accounts'))
$ErrorActionPreference = 'Stop'
if (Test-Path -LiteralPath (Join-Path $InstallRoot 'account-center.pid')) { throw 'Stop Account Center before restoring a backup.' }
& (Join-Path $InstallRoot 'bin\elegy-accounts.exe') restore $Source
Write-Host 'Encrypted Elegy Accounts backup restored for the current Windows user.'
