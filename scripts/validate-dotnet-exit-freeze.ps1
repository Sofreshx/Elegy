Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$repoRoot = Split-Path -Parent $PSScriptRoot
$remainingCsprojPaths = @(
    Get-ChildItem -Path (Join-Path $repoRoot 'src'), (Join-Path $repoRoot 'tests') -Recurse -Filter '*.csproj' -File |
        ForEach-Object { [System.IO.Path]::GetRelativePath($repoRoot, $_.FullName).Replace('\\', '/') } |
        Sort-Object
)

if ($remainingCsprojPaths.Count -gt 0) {
    $message = @(
        'Zero-dotnet baseline guard violated.',
        'No legacy .NET project surfaces may remain anywhere under src/ or tests/.',
        'Remaining project paths:'
    ) + ($remainingCsprojPaths | ForEach-Object { "- $_" })
    throw ($message -join [Environment]::NewLine)
}

Write-Host 'Zero-dotnet baseline guard checks passed.'
Write-Host 'No legacy .NET project surfaces remain under src/ or tests/.'