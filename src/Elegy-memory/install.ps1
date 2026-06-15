[CmdletBinding()]
param(
    [string]$Destination = '',
    [string]$Tag = '',
    [string]$Repository = 'Sofreshx/Elegy',
    [string]$LocalArtifactsRoot = '',
    [switch]$Force,
    [switch]$AddToPath,
    [switch]$NoCommandShims
)

$ErrorActionPreference = 'Stop'

$bundledInstaller = Join-Path $PSScriptRoot 'scripts/install-distribution.ps1'
$repoInstaller = Join-Path (Split-Path -Parent (Split-Path -Parent $PSScriptRoot)) 'scripts/install-distribution.ps1'
$installerPath = if (Test-Path $bundledInstaller) {
    $bundledInstaller
}
elseif (Test-Path $repoInstaller) {
    $repoInstaller
}
else {
    throw 'Unable to locate scripts/install-distribution.ps1 for the Elegy-memory wrapper surface.'
}

$invokeArgs = @{
    Repository = $Repository
    CliSurfaces = @('elegy-memory')
    WrapperSurfaces = @('elegy-memory')
}

if (-not [string]::IsNullOrWhiteSpace($Destination)) {
    $invokeArgs.Destination = $Destination
}

if (-not [string]::IsNullOrWhiteSpace($Tag)) {
    $invokeArgs.Tag = $Tag
}

if (-not [string]::IsNullOrWhiteSpace($LocalArtifactsRoot)) {
    $invokeArgs.LocalArtifactsRoot = $LocalArtifactsRoot
}

if ($Force) {
    $invokeArgs.Force = $true
}

if ($AddToPath) {
    $invokeArgs.AddToPath = $true
}

if ($NoCommandShims) {
    $invokeArgs.NoCommandShims = $true
}

& $installerPath @invokeArgs