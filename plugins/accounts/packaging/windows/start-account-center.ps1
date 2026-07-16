param([string]$InstallRoot = (Join-Path $env:LOCALAPPDATA 'Elegy\Accounts'))
$ErrorActionPreference = 'Stop'
$exe = Join-Path $InstallRoot 'bin\elegy-accounts.exe'
$ui = Join-Path $InstallRoot 'account-center'
$pidFile = Join-Path $InstallRoot 'account-center.pid'
if (-not (Test-Path -LiteralPath $exe)) { throw "Elegy Accounts is not installed at $InstallRoot" }
$env:ELEGY_ACCOUNT_CENTER_DIST = $ui
$process = Start-Process -FilePath $exe -ArgumentList 'broker' -WindowStyle Hidden -PassThru
Set-Content -LiteralPath $pidFile -Value $process.Id -Encoding ascii
Start-Process 'http://127.0.0.1:43119/'
Write-Host "Elegy Account Center started locally (PID $($process.Id))."
