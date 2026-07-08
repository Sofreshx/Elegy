#!/usr/bin/env bash
set -euo pipefail

REPO=""
TOKEN=""
VERIFY=false

while [[ $# -gt 0 ]]; do
    case "$1" in
        --repo)    REPO="$2"; shift 2 ;;
        --token)   TOKEN="$2"; shift 2 ;;
        --verify)  VERIFY=true; shift ;;
        *) echo "Unknown option: $1"; exit 1 ;;
    esac
done

if [ -z "$REPO" ] || [ -z "$TOKEN" ]; then
    echo "Usage: bash setup-release-token.sh --repo Sofreshx/elegy-checks --token <pat-value> [--verify]"
    exit 1
fi

if [[ ! "$TOKEN" =~ ^github_pat_ ]] && [[ ! "$TOKEN" =~ ^ghp_ ]]; then
    echo "WARNING: Token does not look like a GitHub PAT."
    echo "  Fine-grained PATs start with 'github_pat_'. Classic PATs start with 'ghp_'."
    echo "  Continuing anyway..."
fi

echo "Setting ELEGY_RELEASE_TOKEN on $REPO..."
echo "$TOKEN" | gh secret set ELEGY_RELEASE_TOKEN --repo "$REPO"

if [ "$VERIFY" = true ]; then
    echo "Verifying..."
    if gh secret list --repo "$REPO" | grep -q "ELEGY_RELEASE_TOKEN"; then
        echo "OK: ELEGY_RELEASE_TOKEN is set on $REPO"
    else
        echo "ERROR: ELEGY_RELEASE_TOKEN not found on $REPO"
        exit 1
    fi
else
    echo "Done. Verify with: gh secret list --repo $REPO"
fi

echo ""
echo "Next steps:"
echo "  1. Trigger the release workflow: gh workflow run release-plugin.yml --repo $REPO --ref main"
echo "  2. Check status: gh run list --repo $REPO --limit 1"
