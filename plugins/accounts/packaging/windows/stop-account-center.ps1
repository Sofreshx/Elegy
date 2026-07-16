param([string]$InstallRoot = (Join-Path $env:LOCALAPPDATA 'Elegy\Accounts'))
$ErrorActionPreference = 'Stop'
$pidFile = Join-Path $InstallRoot 'account-center.pid'
if (-not (Test-Path -LiteralPath $pidFile)) { Write-Host 'Elegy Account Center is not running.'; exit 0 }
$processId = [int](Get-Content -LiteralPath $pidFile -Raw)
$process = Get-Process -Id $processId -ErrorAction SilentlyContinue
if ($process) { Stop-Process -Id $processId }
Remove-Item -LiteralPath $pidFile -Force
Write-Host 'Elegy Account Center stopped.'
