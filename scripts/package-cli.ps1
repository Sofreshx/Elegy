[CmdletBinding()]
param(
    [ValidateSet('elegy-cli', 'elegy-memory', 'elegy-mcp', 'elegy-skills')]
    [string]$Surface = 'elegy-cli',
    [string]$Target = '',
    [string]$OutputDirectory = '',
    [switch]$SkipBuild
)

$ErrorActionPreference = 'Stop'

$repoRoot = Split-Path -Parent $PSScriptRoot
$rustRoot = Join-Path $repoRoot 'rust'
$packageReadmePath = Join-Path $repoRoot 'PACKAGE_README.md'

function Get-DistributionSurfaceMetadata {
    param(
        [string]$SurfaceName
    )

    switch ($SurfaceName) {
        'elegy-cli' {
            return @{
                Surface = 'elegy-cli'
                Package = 'elegy-cli'
                Binary = 'elegy'
                AssetPrefix = 'elegy-cli'
            }
        }
        'elegy-memory' {
            return @{
                Surface = 'elegy-memory'
                Package = 'elegy-memory'
                Binary = 'elegy-memory'
                AssetPrefix = 'elegy-memory'
            }
        }
        'elegy-mcp' {
            return @{
                Surface = 'elegy-mcp'
                Package = 'elegy-mcp'
                Binary = 'elegy-mcp'
                AssetPrefix = 'elegy-mcp'
            }
        }
        'elegy-skills' {
            return @{
                Surface = 'elegy-skills'
                Package = 'elegy-skills'
                Binary = 'elegy-skills'
                AssetPrefix = 'elegy-skills'
            }
        }
        default {
            throw "Unsupported distribution surface: $SurfaceName"
        }
    }
}

function Get-PackageVersion {
    param(
        [string]$WorkspaceRoot,
        [string]$PackageName
    )

    Push-Location $WorkspaceRoot
    try {
        $metadataJson = & cargo metadata --format-version 1 --no-deps
        if ($LASTEXITCODE -ne 0) {
            throw "cargo metadata failed while resolving the package version for $PackageName."
        }
    }
    finally {
        Pop-Location
    }

    $metadata = $metadataJson | ConvertFrom-Json
    $package = $metadata.packages | Where-Object { $_.name -eq $PackageName } | Select-Object -First 1

    if ($null -eq $package -or [string]::IsNullOrWhiteSpace($package.version)) {
        throw "Unable to resolve the package version for $PackageName from cargo metadata."
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

function Get-BinaryFileName {
    param(
        [string]$BinaryName,
        [string]$TargetTriple
    )

    if ($TargetTriple -match 'windows') {
        return "$BinaryName.exe"
    }

    return $BinaryName
}

if ([string]::IsNullOrWhiteSpace($OutputDirectory)) {
    $OutputDirectory = Join-Path $repoRoot 'artifacts\distribution'
}

if (-not (Test-Path $packageReadmePath)) {
    throw "Missing package README source: $packageReadmePath"
}

$surfaceMetadata = Get-DistributionSurfaceMetadata -SurfaceName $Surface

$resolvedTarget = if ([string]::IsNullOrWhiteSpace($Target)) {
    Get-HostTargetTriple -WorkspaceRoot $rustRoot
}
else {
    $Target.Trim()
}

$packageVersion = Get-PackageVersion -WorkspaceRoot $rustRoot -PackageName $surfaceMetadata.Package
$binaryFileName = Get-BinaryFileName -BinaryName $surfaceMetadata.Binary -TargetTriple $resolvedTarget

Push-Location $rustRoot
try {
    if (-not $SkipBuild) {
        $buildArgs = @('build', '--locked', '-p', $surfaceMetadata.Package, '--bin', $surfaceMetadata.Binary, '--release')
        if (-not [string]::IsNullOrWhiteSpace($Target)) {
            $buildArgs += @('--target', $resolvedTarget)
        }

        & cargo @buildArgs
        if ($LASTEXITCODE -ne 0) {
            throw "cargo build failed while packaging the $($surfaceMetadata.Surface) archive."
        }
    }
}
finally {
    Pop-Location
}

$binaryPath = if ([string]::IsNullOrWhiteSpace($Target)) {
    Join-Path $rustRoot "target\release\$binaryFileName"
}
else {
    Join-Path $rustRoot "target\$resolvedTarget\release\$binaryFileName"
}

if (-not (Test-Path $binaryPath)) {
    throw "Built $($surfaceMetadata.Surface) binary was not found at $binaryPath"
}

New-Item -ItemType Directory -Path $OutputDirectory -Force | Out-Null

$assetBaseName = "$($surfaceMetadata.AssetPrefix)-$packageVersion-$resolvedTarget"
$stagingDirectory = Join-Path $OutputDirectory $assetBaseName
$archivePath = Join-Path $OutputDirectory "$assetBaseName.zip"

if (Test-Path $stagingDirectory) {
    Remove-Item -Path $stagingDirectory -Recurse -Force
}

if (Test-Path $archivePath) {
    Remove-Item -Path $archivePath -Force
}

New-Item -ItemType Directory -Path $stagingDirectory -Force | Out-Null
Copy-Item -Path $binaryPath -Destination (Join-Path $stagingDirectory $binaryFileName) -Force
Copy-Item -Path $packageReadmePath -Destination (Join-Path $stagingDirectory 'README.md') -Force
Compress-Archive -Path (Join-Path $stagingDirectory '*') -DestinationPath $archivePath -CompressionLevel Optimal
Remove-Item -Path $stagingDirectory -Recurse -Force

Write-Host "Packaged CLI archive: $archivePath"
Write-Host " - surface: $($surfaceMetadata.Surface)"
Write-Host " - package: $($surfaceMetadata.Package)"
Write-Host " - package version: $packageVersion"
Write-Host " - target: $resolvedTarget"
Write-Host " - binary: $binaryPath"
