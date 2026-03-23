[CmdletBinding()]
param(
    [string]$Target = '',
    [string]$OutputDirectory = '',
    [switch]$SkipBuild
)

$ErrorActionPreference = 'Stop'

$repoRoot = Split-Path -Parent $PSScriptRoot
$rustRoot = Join-Path $repoRoot 'rust'

function Get-CliPackageVersion {
    param(
        [string]$WorkspaceRoot
    )

    Push-Location $WorkspaceRoot
    try {
        $metadataJson = & cargo metadata --format-version 1 --no-deps
        if ($LASTEXITCODE -ne 0) {
            throw 'cargo metadata failed while resolving the elegy CLI package version.'
        }
    }
    finally {
        Pop-Location
    }

    $metadata = $metadataJson | ConvertFrom-Json
    $package = $metadata.packages | Where-Object { $_.name -eq 'elegy-cli' } | Select-Object -First 1

    if ($null -eq $package -or [string]::IsNullOrWhiteSpace($package.version)) {
        throw 'Unable to resolve the elegy CLI package version from cargo metadata.'
    }

    return $package.version
}

function Get-HostTargetTriple {
    param(
        [string]$WorkspaceRoot
    )

    Push-Location $WorkspaceRoot
    try {
        $rustcVersion = & rustc -vV
        if ($LASTEXITCODE -ne 0) {
            throw 'rustc -vV failed while resolving the host target triple.'
        }
    }
    finally {
        Pop-Location
    }

    $hostLine = $rustcVersion | Where-Object { $_ -match '^host:\s+' } | Select-Object -First 1
    if ([string]::IsNullOrWhiteSpace($hostLine)) {
        throw 'Unable to locate the rust host target triple.'
    }

    return ($hostLine -replace '^host:\s+', '').Trim()
}

function Get-BinaryName {
    param(
        [string]$TargetTriple
    )

    if ($TargetTriple -match 'windows') {
        return 'elegy.exe'
    }

    return 'elegy'
}

if ([string]::IsNullOrWhiteSpace($OutputDirectory)) {
    $OutputDirectory = Join-Path $repoRoot 'artifacts\distribution'
}

$resolvedTarget = if ([string]::IsNullOrWhiteSpace($Target)) {
    Get-HostTargetTriple -WorkspaceRoot $rustRoot
}
else {
    $Target.Trim()
}

$cliVersion = Get-CliPackageVersion -WorkspaceRoot $rustRoot
$binaryName = Get-BinaryName -TargetTriple $resolvedTarget

Push-Location $rustRoot
try {
    if (-not $SkipBuild) {
        $buildArgs = @('build', '--locked', '-p', 'elegy-cli', '--bin', 'elegy', '--release')
        if (-not [string]::IsNullOrWhiteSpace($Target)) {
            $buildArgs += @('--target', $resolvedTarget)
        }

        & cargo @buildArgs
        if ($LASTEXITCODE -ne 0) {
            throw 'cargo build failed while packaging the elegy CLI archive.'
        }
    }
}
finally {
    Pop-Location
}

$binaryPath = if ([string]::IsNullOrWhiteSpace($Target)) {
    Join-Path $rustRoot "target\release\$binaryName"
}
else {
    Join-Path $rustRoot "target\$resolvedTarget\release\$binaryName"
}

if (-not (Test-Path $binaryPath)) {
    throw "Built elegy CLI binary was not found at $binaryPath"
}

New-Item -ItemType Directory -Path $OutputDirectory -Force | Out-Null

$assetBaseName = "elegy-cli-$cliVersion-$resolvedTarget"
$stagingDirectory = Join-Path $OutputDirectory $assetBaseName
$archivePath = Join-Path $OutputDirectory "$assetBaseName.zip"

if (Test-Path $stagingDirectory) {
    Remove-Item -Path $stagingDirectory -Recurse -Force
}

if (Test-Path $archivePath) {
    Remove-Item -Path $archivePath -Force
}

New-Item -ItemType Directory -Path $stagingDirectory -Force | Out-Null
Copy-Item -Path $binaryPath -Destination (Join-Path $stagingDirectory $binaryName) -Force
Compress-Archive -Path (Join-Path $stagingDirectory '*') -DestinationPath $archivePath -CompressionLevel Optimal
Remove-Item -Path $stagingDirectory -Recurse -Force

Write-Host "Packaged CLI archive: $archivePath"
Write-Host " - CLI version: $cliVersion"
Write-Host " - target: $resolvedTarget"
Write-Host " - binary: $binaryPath"