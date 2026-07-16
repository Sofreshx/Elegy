param([Parameter(Mandatory = $true)][string]$Destination, [string]$InstallRoot = (Join-Path $env:LOCALAPPDATA 'Elegy\Accounts'))
$ErrorActionPreference = 'Stop'
& (Join-Path $InstallRoot 'bin\elegy-accounts.exe') backup $Destination
Write-Host "Encrypted Elegy Accounts backup created at $Destination"
