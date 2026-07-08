param(
    [Parameter(Mandatory = $true)]
    [string]$Repo,

    [Parameter(Mandatory = $true)]
    [string]$Token,

    [switch]$Verify
)

$ErrorActionPreference = "Stop"

if ($Token -notmatch "^github_pat_" -and $Token -notmatch "^ghp_") {
    Write-Host "WARNING: Token does not look like a GitHub PAT." -ForegroundColor Yellow
    Write-Host "  Fine-grained PATs start with 'github_pat_'. Classic PATs start with 'ghp_'."
    Write-Host "  Continuing anyway..."
}

Write-Host "Setting ELEGY_RELEASE_TOKEN on $Repo..."
$Token | gh secret set ELEGY_RELEASE_TOKEN --repo $Repo

if ($Verify) {
    Write-Host "Verifying..."
    $secrets = gh secret list --repo $Repo | Select-String "ELEGY_RELEASE_TOKEN"
    if ($secrets) {
        Write-Host "OK: ELEGY_RELEASE_TOKEN is set on $Repo" -ForegroundColor Green
    } else {
        Write-Host "ERROR: ELEGY_RELEASE_TOKEN not found on $Repo" -ForegroundColor Red
        exit 1
    }
} else {
    Write-Host "Done. Verify with: gh secret list --repo $Repo"
}

Write-Host ""
Write-Host "Next steps:"
Write-Host "  1. Trigger the release workflow: gh workflow run release-plugin.yml --repo $Repo --ref main"
Write-Host "  2. Check status: gh run list --repo $Repo --limit 1"
