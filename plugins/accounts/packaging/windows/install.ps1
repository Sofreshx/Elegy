param(
  [Parameter(Mandatory = $true)]
  [ValidatePattern('^[a-p]{32}$')]
  [string]$ExtensionId,
  [string]$InstallRoot = (Join-Path $env:LOCALAPPDATA 'Elegy\Accounts'),
  [switch]$SkipRegistry
)

$ErrorActionPreference = 'Stop'
$pluginRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..\..\..')).Path
$sourceExe = Join-Path $pluginRoot 'bin\elegy-accounts.exe'
if (-not (Test-Path -LiteralPath $sourceExe)) {
  $sourceExe = Join-Path $repoRoot 'target\release\elegy-accounts.exe'
}
if (-not (Test-Path -LiteralPath $sourceExe)) {
  throw 'Missing elegy-accounts.exe. Install from a packed plugin or run cargo build --release -p elegy-accounts.'
}
$extensionSource = Join-Path $pluginRoot 'browser\brave'
$accountCenterSource = Join-Path $pluginRoot 'ui\account-center'
$providerSource = Join-Path $pluginRoot 'providers'
$binDir = Join-Path $InstallRoot 'bin'
$extensionDir = Join-Path $InstallRoot 'brave-extension'
$uiDir = Join-Path $InstallRoot 'account-center'
$providerDir = Join-Path $InstallRoot 'providers'
$hostManifest = Join-Path $InstallRoot 'com.elegy.accounts.json'

New-Item -ItemType Directory -Force -Path $binDir, $extensionDir, $uiDir, $providerDir | Out-Null
Copy-Item -LiteralPath $sourceExe -Destination (Join-Path $binDir 'elegy-accounts.exe') -Force
Copy-Item -LiteralPath $sourceExe -Destination (Join-Path $binDir 'elegy-accounts-native-host.exe') -Force
Copy-Item -Path (Join-Path $extensionSource '*') -Destination $extensionDir -Recurse -Force
Copy-Item -Path (Join-Path $accountCenterSource '*') -Destination $uiDir -Recurse -Force
Copy-Item -Path (Join-Path $providerSource '*.json') -Destination $providerDir -Force
Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'start-account-center.ps1'), (Join-Path $PSScriptRoot 'stop-account-center.ps1'), (Join-Path $PSScriptRoot 'backup.ps1'), (Join-Path $PSScriptRoot 'restore.ps1') -Destination $InstallRoot -Force

$template = Get-Content -LiteralPath (Join-Path $PSScriptRoot 'com.elegy.accounts.json.template') -Raw
$nativeHost = (Join-Path $binDir 'elegy-accounts-native-host.exe').Replace('\', '\\')
$manifest = $template.Replace('@@NATIVE_HOST_EXE@@', $nativeHost).Replace('@@EXTENSION_ID@@', $ExtensionId)
[System.IO.File]::WriteAllText($hostManifest, $manifest, (New-Object System.Text.UTF8Encoding($false)))

if (-not $SkipRegistry) {
  $registryPath = 'HKCU:\Software\BraveSoftware\Brave-Browser\NativeMessagingHosts\com.elegy.accounts'
  New-Item -Path $registryPath -Force | Out-Null
  Set-Item -Path $registryPath -Value $hostManifest
}

Write-Host "Elegy Accounts installed locally."
Write-Host "Load the extension from: $extensionDir"
