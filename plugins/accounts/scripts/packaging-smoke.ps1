$ErrorActionPreference = 'Stop'
$repo = Resolve-Path (Join-Path $PSScriptRoot '..')
$root = Join-Path $env:TEMP ("ElegyAccountsSmoke-" + [guid]::NewGuid().ToString('N'))
$previousLocalAppData = $env:LOCALAPPDATA
$previousAccountCenterPort = $env:ELEGY_ACCOUNT_CENTER_PORT
$process = $null
try {
  & (Join-Path $repo 'packaging\windows\install.ps1') -ExtensionId 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa' -InstallRoot $root -SkipRegistry
  $required = @(
    'bin\elegy-accounts.exe', 'bin\elegy-accounts-native-host.exe', 'account-center\index.html',
    'brave-extension\manifest.json', 'com.elegy.accounts.json', 'start-account-center.ps1',
    'stop-account-center.ps1', 'backup.ps1', 'restore.ps1', 'providers\github.json',
    'providers\cloudflare.json', 'providers\google.json'
  )
  foreach ($relative in $required) { if (-not (Test-Path -LiteralPath (Join-Path $root $relative))) { throw "Missing installed file: $relative" } }
  $native = Get-Content -LiteralPath (Join-Path $root 'com.elegy.accounts.json') -Raw | ConvertFrom-Json
  if ($native.allowed_origins.Count -ne 1 -or $native.allowed_origins[0] -ne 'chrome-extension://aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa/') { throw 'Native host origin allowlist is invalid.' }

  $env:LOCALAPPDATA = Join-Path $root 'local-data'
  $env:ELEGY_ACCOUNT_CENTER_DIST = Join-Path $root 'account-center'
  $portProbe = [System.Net.Sockets.TcpListener]::new([System.Net.IPAddress]::Loopback, 0)
  $portProbe.Start()
  $smokePort = ([System.Net.IPEndPoint]$portProbe.LocalEndpoint).Port
  $portProbe.Stop()
  $env:ELEGY_ACCOUNT_CENTER_PORT = $smokePort
  $process = Start-Process -FilePath (Join-Path $root 'bin\elegy-accounts.exe') -ArgumentList 'broker' -WindowStyle Hidden -PassThru
  $healthy = $false
  foreach ($attempt in 1..30) {
    try {
      $response = Invoke-RestMethod -Uri ("http://127.0.0.1:$smokePort/api/state") -TimeoutSec 1
      if ($null -ne $response.accounts -and $response.providers.Count -ge 3) { $healthy = $true; break }
    } catch { Start-Sleep -Milliseconds 100 }
  }
  if (-not $healthy) { throw 'Installed Account Center did not become healthy.' }

  $artifacts = Join-Path $repo 'artifacts\acceptance'
  New-Item -ItemType Directory -Force -Path $artifacts | Out-Null
  $report = @{ passed = $true; installedFiles = $required; localEndpoint = "127.0.0.1:$smokePort"; registrySkipped = $true } | ConvertTo-Json -Depth 4
  [System.IO.File]::WriteAllText((Join-Path $artifacts 'packaging-smoke.json'), $report, (New-Object System.Text.UTF8Encoding($false)))
} finally {
  if ($process -and -not $process.HasExited) { Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue }
  $env:LOCALAPPDATA = $previousLocalAppData
  if ($null -eq $previousAccountCenterPort) { Remove-Item Env:ELEGY_ACCOUNT_CENTER_PORT -ErrorAction SilentlyContinue }
  else { $env:ELEGY_ACCOUNT_CENTER_PORT = $previousAccountCenterPort }
  if (Test-Path -LiteralPath $root) { Remove-Item -LiteralPath $root -Recurse -Force }
}
