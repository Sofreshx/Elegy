#!/usr/bin/env bash
set -euo pipefail

# ── Elegy Local MCP Install Receipt ───────────────────────────────────
# `just install-mcp` copies freshly built binaries into ~/.elegy/bin with
# a plain `cp` and no version stamp. That silence let the copy installed
# there on 2026-08-26 sit ~10 days stale against the workspace's own last
# build, with nothing to detect the gap while the elegy-run MCP host it
# shipped was broken (DESC-CONFIG-002, see elegy.resources.d/README.md).
#
# This script writes (`write`) or verifies (`check`) a small JSON receipt
# recording the sha256 of each installed binary against the freshly built
# one, plus the git SHA it was built from.
#
# Usage:
#   ./scripts/mcp-install-receipt.sh write <install_dir> <bin1> [<bin2> ...]
#   ./scripts/mcp-install-receipt.sh check <install_dir> <bin1> [<bin2> ...]
# ─────────────────────────────────────────────────────────────────────

MODE="${1:-}"
INSTALL_DIR="${2:-}"
shift 2 || true
BINARIES=("$@")

if [[ -z "$MODE" || -z "$INSTALL_DIR" || ${#BINARIES[@]} -eq 0 ]]; then
    echo "Usage: $0 <write|check> <install_dir> <binary_path> [<binary_path> ...]" >&2
    exit 2
fi

RECEIPT="${INSTALL_DIR}/install-receipt.json"

sha256_of() {
    sha256sum "$1" | cut -d' ' -f1
}

case "$MODE" in
write)
    GIT_SHA="$(git rev-parse HEAD 2>/dev/null || echo "unknown")"
    GIT_DIRTY="false"
    if ! git diff --quiet --ignore-submodules HEAD -- 2>/dev/null; then
        GIT_DIRTY="true"
    fi
    INSTALLED_AT="$(date -u +%Y-%m-%dT%H:%M:%SZ)"

    ENTRIES="["
    first=true
    for bin_path in "${BINARIES[@]}"; do
        name="$(basename "$bin_path")"
        sha="$(sha256_of "$bin_path")"
        if [[ "$first" == true ]]; then
            first=false
        else
            ENTRIES+=","
        fi
        ENTRIES+=$'\n'"    {\"name\": \"${name}\", \"sha256\": \"${sha}\"}"
    done
    ENTRIES+=$'\n  ]'

    cat > "$RECEIPT" <<JSON
{
  "schemaVersion": 1,
  "installedAt": "${INSTALLED_AT}",
  "sourceGitSha": "${GIT_SHA}",
  "sourceDirty": ${GIT_DIRTY},
  "binaries": ${ENTRIES}
}
JSON
    echo "[DONE] wrote ${RECEIPT}"
    ;;
check)
    if [[ ! -f "$RECEIPT" ]]; then
        echo "WARNING: no install receipt at ${RECEIPT}; this install was not made by 'just install-mcp'." >&2
        echo "         Run 'just install-mcp' to install a receipted build." >&2
        exit 1
    fi

    STALE=false
    for bin_path in "${BINARIES[@]}"; do
        name="$(basename "$bin_path")"
        fresh_sha="$(sha256_of "$bin_path")"
        recorded_sha="$(grep -o "\"name\": *\"${name}\", *\"sha256\": *\"[0-9a-f]\{64\}\"" "$RECEIPT" | grep -o '[0-9a-f]\{64\}' || true)"
        if [[ -z "$recorded_sha" ]]; then
            echo "WARNING: ${RECEIPT} has no entry for ${name}." >&2
            STALE=true
        elif [[ "$fresh_sha" != "$recorded_sha" ]]; then
            echo "WARNING: installed ${name} does not match the current workspace build (sha256 differs)." >&2
            echo "         Run 'just install-mcp' to update it." >&2
            STALE=true
        fi
    done

    if [[ "$STALE" == true ]]; then
        exit 1
    fi
    echo "[OK] installed binaries match the current workspace build (receipt: ${RECEIPT})"
    ;;
*)
    echo "Usage: $0 <write|check> <install_dir> <binary_path> [<binary_path> ...]" >&2
    exit 2
    ;;
esac
