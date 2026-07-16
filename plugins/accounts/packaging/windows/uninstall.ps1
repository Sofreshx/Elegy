param([string]$InstallRoot = (Join-Path $env:LOCALAPPDATA 'Elegy\Accounts'))

$ErrorActionPreference = 'Stop'
$registryPath = 'HKCU:\Software\BraveSoftware\Brave-Browser\NativeMessagingHosts\com.elegy.accounts'
if (Test-Path $registryPath) { Remove-Item -LiteralPath $registryPath -Force }
Write-Host "Native Messaging registration removed."
Write-Host "Encrypted account data remains at $InstallRoot. Remove it manually only if you intend to delete all local accounts and audit history."
