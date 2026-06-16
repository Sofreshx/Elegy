param(
    [Parameter(Mandatory = $true)]
    [string] $Manifest,

    [string] $Elegy = "elegy"
)

$ErrorActionPreference = "Stop"

if (-not (Test-Path -LiteralPath $Manifest)) {
    throw "Generator manifest not found: $Manifest"
}

& $Elegy generator validate $Manifest --json
if ($LASTEXITCODE -ne 0) {
    throw "Generator manifest validation failed for $Manifest"
}
