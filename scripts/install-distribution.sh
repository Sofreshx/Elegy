#!/usr/bin/env bash
set -euo pipefail

# ── Elegy Distribution Installer ──────────────────────────────────────
# Downloads an Elegy surface from GitHub Releases, verifies SHA256,
# and installs it to a destination directory. Plugin-packaged surfaces
# prefer portable plugin archives and fall back to legacy flat binaries.
#
# Each consuming repo and external plugin can copy this script or use
# scripts/install-template.sh as a base.
#
# Usage:
#   ./install-distribution.sh -t v0.1.0 -d ./tools/elegy -s elegy-planning
#   ./install-distribution.sh -t v0.1.0 -d ./tools/elegy -s elegy-memory -f
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
  ./install-distribution.sh -t v0.1.0 -d .elegy -s elegy-memory -f
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
PLUGIN_URL="${BASE_URL}/${SURFACE}-plugin-${TARGET}.zip"
PLUGIN_CHECKSUM_URL="${PLUGIN_URL}.sha256"
LEGACY_BINARY_URL="${BASE_URL}/${SURFACE}-${TARGET}${EXE}"
LEGACY_CHECKSUM_URL="${LEGACY_BINARY_URL}.sha256"

DEST_BIN="${DESTINATION}/bin"
DEST_BUNDLE="${DESTINATION}/bundle/plugins/${SURFACE}"
TMP_DIR=""

cleanup() {
    if [[ -n "$TMP_DIR" && -d "$TMP_DIR" ]]; then
        rm -rf "$TMP_DIR"
    fi
}

trap cleanup EXIT

download_file() {
    local url="$1"
    local output="$2"
    if command -v curl &>/dev/null; then
        curl -fSL --progress-bar -o "$output" "$url"
    elif command -v wget &>/dev/null; then
        wget -q --show-progress -O "$output" "$url"
    else
        echo "ERROR: curl or wget required."
        exit 1
    fi
}

remote_exists() {
    local url="$1"
    if command -v curl &>/dev/null; then
        curl -fsIL "$url" >/dev/null 2>&1
    elif command -v wget &>/dev/null; then
        wget --spider -q "$url" >/dev/null 2>&1
    else
        return 1
    fi
}

download_text() {
    local url="$1"
    if command -v curl &>/dev/null; then
        curl -fSL "$url" 2>/dev/null || true
    elif command -v wget &>/dev/null; then
        wget -q -O - "$url" 2>/dev/null || true
    fi
}

verify_sha256() {
    local file_path="$1"
    local checksum_url="$2"
    if ! command -v sha256sum &>/dev/null; then
        echo "[WARN] sha256sum not found; skipping verification"
        return 0
    fi
    local expected_hash
    expected_hash="$(download_text "$checksum_url" | awk '{print $1}')"
    if [[ -z "$expected_hash" ]]; then
        echo "[WARN] Could not fetch checksum; skipping verification"
        return 0
    fi
    local actual_hash
    actual_hash="$(sha256sum "$file_path" | awk '{print $1}')"
    if [[ "$expected_hash" != "$actual_hash" ]]; then
        echo "ERROR: SHA256 mismatch. Expected: $expected_hash, got: $actual_hash"
        exit 1
    fi
    echo "[OK] SHA256 verified"
}

extract_zip() {
    local archive_path="$1"
    local output_dir="$2"
    mkdir -p "$output_dir"
    if command -v python &>/dev/null; then
        ARCHIVE_PATH="$archive_path" OUTPUT_DIR="$output_dir" python - <<'PY'
import os
from pathlib import Path
import zipfile

archive_path = Path(os.environ["ARCHIVE_PATH"])
output_dir = Path(os.environ["OUTPUT_DIR"]).resolve()

with zipfile.ZipFile(archive_path) as zf:
    for member in zf.infolist():
        target = (output_dir / member.filename).resolve()
        if not str(target).startswith(str(output_dir)):
            raise SystemExit(f"Unsafe archive entry: {member.filename}")
    zf.extractall(output_dir)
PY
    elif command -v unzip &>/dev/null; then
        unzip -q "$archive_path" -d "$output_dir"
    elif command -v pwsh &>/dev/null; then
        pwsh -NoProfile -Command "Expand-Archive -LiteralPath '$archive_path' -DestinationPath '$output_dir' -Force"
    else
        echo "ERROR: python, unzip, or pwsh required to extract plugin archives."
        exit 1
    fi
}

mkdir -p "$DEST_BIN"

if remote_exists "$PLUGIN_URL"; then
    if [[ -e "$DEST_BUNDLE" && "$FORCE" != true ]]; then
        echo "Already installed at ${DEST_BUNDLE}. Use -f to overwrite."
        exit 0
    fi

    echo "[INFO] Installing plugin-packaged surface ${SURFACE} ${TAG} (${TARGET})..."
    TMP_DIR="$(mktemp -d)"
    ARCHIVE_PATH="${TMP_DIR}/${SURFACE}-plugin-${TARGET}.zip"
    EXTRACT_DIR="${TMP_DIR}/extract"

    download_file "$PLUGIN_URL" "$ARCHIVE_PATH" || {
        echo "ERROR: failed to download plugin archive from $PLUGIN_URL"; exit 1;
    }
    verify_sha256 "$ARCHIVE_PATH" "$PLUGIN_CHECKSUM_URL"
    extract_zip "$ARCHIVE_PATH" "$EXTRACT_DIR"

    PLUGIN_BIN="${EXTRACT_DIR}/bin/${SURFACE}${EXE}"
    if [[ ! -f "$PLUGIN_BIN" ]]; then
        echo "ERROR: plugin archive is missing bin/${SURFACE}${EXE}" >&2
        exit 1
    fi

    if [[ "$FORCE" == true ]]; then
        rm -rf "$DEST_BUNDLE"
        rm -f "${DEST_BIN}/${SURFACE}${EXE}"
    fi

    mkdir -p "$(dirname "$DEST_BUNDLE")" "$DEST_BUNDLE"
    cp -R "${EXTRACT_DIR}/." "$DEST_BUNDLE/"
    cp "$PLUGIN_BIN" "${DEST_BIN}/${SURFACE}${EXE}"
    chmod +x "${DEST_BIN}/${SURFACE}${EXE}" 2>/dev/null || true

    echo "[DONE] ${SURFACE} installed to ${DEST_BIN}/${SURFACE}${EXE}"
    echo "       Bundle: ${DEST_BUNDLE}"
    echo "       Verify: ${DEST_BIN}/${SURFACE}${EXE} --version"
    exit 0
fi

if [[ -f "${DEST_BIN}/${SURFACE}${EXE}" && "$FORCE" != true ]]; then
    echo "Already installed at ${DEST_BIN}/${SURFACE}${EXE}. Use -f to overwrite."
    exit 0
fi

echo "[INFO] Installing legacy flat-binary surface ${SURFACE} ${TAG} (${TARGET})..."
download_file "$LEGACY_BINARY_URL" "${DEST_BIN}/${SURFACE}${EXE}" || {
    echo "ERROR: failed to download binary from $LEGACY_BINARY_URL"; exit 1;
}
verify_sha256 "${DEST_BIN}/${SURFACE}${EXE}" "$LEGACY_CHECKSUM_URL"
chmod +x "${DEST_BIN}/${SURFACE}${EXE}" 2>/dev/null || true

echo "[DONE] ${SURFACE} installed to ${DEST_BIN}/${SURFACE}${EXE}"
echo "       Verify: ${DEST_BIN}/${SURFACE}${EXE} --version"
