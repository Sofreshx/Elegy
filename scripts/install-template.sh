#!/usr/bin/env bash
set -euo pipefail
# ── Elegy Plugin Install Template ─────────────────────────────────────
# Copy this file into your repo and adjust:
#   - REPOSITORY (your GitHub repo)
#   - SURFACE (your binary name)
# Usage: ./install-template.sh -t v0.1.0 -d ./tools/my-plugin

REPOSITORY="YOUR_ORG/YOUR_REPO"   # ← CHANGE THIS
SURFACE="YOUR_BINARY_NAME"         # ← CHANGE THIS
TAG=""
DESTINATION=""
FORCE=false

_show_help() {
    cat <<'HELP'
Usage: install.sh [OPTIONS]
Options:
  -t, --tag TAG           Release tag (required)
  -d, --destination PATH  Install directory (required)
  -f, --force             Overwrite existing installation
  -h, --help              Show this help
HELP
    exit 0
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        -t|--tag) TAG="$2"; shift 2 ;;
        -d|--destination) DESTINATION="$2"; shift 2 ;;
        -f|--force) FORCE=true; shift ;;
        -h|--help) _show_help ;;
        *) echo "Unknown option: $1"; exit 1 ;;
    esac
done

if [[ -z "$TAG" || -z "$DESTINATION" ]]; then
    echo "ERROR: --tag and --destination are required."; exit 1
fi

case "$(uname -s)" in
    Linux)  TARGET="x86_64-unknown-linux-gnu" ;;
    Darwin) TARGET="aarch64-apple-darwin" ;;
    MINGW*|MSYS*|CYGWIN*) TARGET="x86_64-pc-windows-msvc" ;;
    *) echo "ERROR: unsupported platform $(uname -s)"; exit 1 ;;
esac

EXE=""
[[ "$TARGET" == *windows* ]] && EXE=".exe"

BASE_URL="https://github.com/${REPOSITORY}/releases/download/${TAG}"
BINARY_URL="${BASE_URL}/${SURFACE}-${TARGET}${EXE}"
CHECKSUM_URL="${BASE_URL}/${SURFACE}-${TARGET}${EXE}.sha256"

DEST_BIN="${DESTINATION}/bin"

if [[ -f "${DEST_BIN}/${SURFACE}${EXE}" && "$FORCE" != true ]]; then
    echo "Already installed. Use -f to overwrite."; exit 0
fi

mkdir -p "$DEST_BIN"

if command -v curl &>/dev/null; then
    curl -fSL --progress-bar -o "${DEST_BIN}/${SURFACE}${EXE}" "$BINARY_URL"
elif command -v wget &>/dev/null; then
    wget -q --show-progress -O "${DEST_BIN}/${SURFACE}${EXE}" "$BINARY_URL"
else
    echo "ERROR: curl or wget required."; exit 1
fi

# Verify SHA256 if checksum available
if command -v sha256sum &>/dev/null; then
    EXPECTED_HASH=$(curl -fSL "$CHECKSUM_URL" 2>/dev/null | awk '{print $1}')
    if [[ -n "$EXPECTED_HASH" ]]; then
        ACTUAL_HASH=$(sha256sum "${DEST_BIN}/${SURFACE}${EXE}" | awk '{print $1}')
        [[ "$EXPECTED_HASH" == "$ACTUAL_HASH" ]] || {
            echo "ERROR: SHA256 mismatch."; exit 1;
        }
    fi
fi

chmod +x "${DEST_BIN}/${SURFACE}${EXE}" 2>/dev/null || true
echo "[DONE] ${SURFACE} installed to ${DEST_BIN}/${SURFACE}${EXE}"
