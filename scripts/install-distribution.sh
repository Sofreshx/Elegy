#!/usr/bin/env bash
set -euo pipefail

# ── Elegy Distribution Installer ──────────────────────────────────────
# Downloads a single Elegy binary from GitHub Releases, verifies SHA256,
# and installs to a destination directory.
#
# Each consuming repo and external plugin can copy this script or use
# scripts/install-template.sh as a base.
#
# Usage:
#   ./install-distribution.sh -t v0.1.0 -d ./tools/elegy -s elegy-planning
#   ./install-distribution.sh -t v0.1.0 -d ./tools/elegy -s elegy-skills -f
# ─────────────────────────────────────────────────────────────────────

REPOSITORY="Sofreshx/Elegy"
TAG=""
DESTINATION=""
SURFACE=""
FORCE=false

_show_help() {
    cat <<'HELP'
Usage: install-distribution.sh [OPTIONS]

Options:
  -t, --tag TAG           Release tag (required)
  -d, --destination PATH  Install directory (required)
  -s, --surface NAME      Binary surface name (required, e.g. elegy-planning)
  -f, --force             Overwrite existing installation
  -h, --help              Show this help

Examples:
  ./install-distribution.sh -t v0.1.0 -d .elegy -s elegy-planning
  ./install-distribution.sh -t v0.1.0 -d .elegy -s elegy-skills -f
HELP
    exit 0
}

# Parse args
while [[ $# -gt 0 ]]; do
    case "$1" in
        -t|--tag) TAG="$2"; shift 2 ;;
        -d|--destination) DESTINATION="$2"; shift 2 ;;
        -s|--surface) SURFACE="$2"; shift 2 ;;
        -f|--force) FORCE=true; shift ;;
        -h|--help) _show_help ;;
        *) echo "Unknown option: $1"; exit 1 ;;
    esac
done

if [[ -z "$TAG" || -z "$DESTINATION" || -z "$SURFACE" ]]; then
    echo "ERROR: --tag, --destination, and --surface are required."
    exit 1
fi

# Detect platform target
case "$(uname -s)" in
    Linux)  TARGET="x86_64-unknown-linux-gnu" ;;
    Darwin) TARGET="aarch64-apple-darwin" ;;
    MINGW*|MSYS*|CYGWIN*) TARGET="x86_64-pc-windows-msvc" ;;
    *) echo "ERROR: unsupported platform $(uname -s)"; exit 1 ;;
esac

# Determine binary extension
EXE=""
[[ "$TARGET" == *windows* ]] && EXE=".exe"

BASE_URL="https://github.com/${REPOSITORY}/releases/download/${TAG}"
BINARY_URL="${BASE_URL}/${SURFACE}-${TARGET}${EXE}"
CHECKSUM_URL="${BASE_URL}/${SURFACE}-${TARGET}${EXE}.sha256"

DEST_BIN="${DESTINATION}/bin"

# Handle existing installation
if [[ -d "$DEST_BIN" ]]; then
    if [[ -f "${DEST_BIN}/${SURFACE}${EXE}" && "$FORCE" != true ]]; then
        echo "Already installed at ${DEST_BIN}/${SURFACE}${EXE}. Use -f to overwrite."
        exit 0
    fi
fi

# Install
echo "[INFO] Installing ${SURFACE} ${TAG} (${TARGET})..."
mkdir -p "$DEST_BIN"

# Download binary
if command -v curl &>/dev/null; then
    curl -fSL --progress-bar -o "${DEST_BIN}/${SURFACE}${EXE}" "$BINARY_URL" || {
        echo "ERROR: failed to download binary from $BINARY_URL"; exit 1;
    }
elif command -v wget &>/dev/null; then
    wget -q --show-progress -O "${DEST_BIN}/${SURFACE}${EXE}" "$BINARY_URL" || {
        echo "ERROR: failed to download binary from $BINARY_URL"; exit 1;
    }
else
    echo "ERROR: curl or wget required."; exit 1;
fi

# Verify SHA256
if command -v sha256sum &>/dev/null; then
    EXPECTED_HASH=$(curl -fSL "$CHECKSUM_URL" 2>/dev/null | awk '{print $1}')
    if [[ -n "$EXPECTED_HASH" ]]; then
        ACTUAL_HASH=$(sha256sum "${DEST_BIN}/${SURFACE}${EXE}" | awk '{print $1}')
        if [[ "$EXPECTED_HASH" != "$ACTUAL_HASH" ]]; then
            echo "ERROR: SHA256 mismatch. Expected: $EXPECTED_HASH, got: $ACTUAL_HASH"
            exit 1
        fi
        echo "[OK] SHA256 verified"
    else
        echo "[WARN] Could not fetch checksum; skipping verification"
    fi
else
    echo "[WARN] sha256sum not found; skipping verification"
fi

# Make executable
chmod +x "${DEST_BIN}/${SURFACE}${EXE}" 2>/dev/null || true

echo "[DONE] ${SURFACE} installed to ${DEST_BIN}/${SURFACE}${EXE}"
echo "       Verify: ${DEST_BIN}/${SURFACE}${EXE} --version"
