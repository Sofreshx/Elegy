#!/usr/bin/env bash
# ============================================================
# install-distribution.sh — Elegy Distribution Installer (Bash)
# ============================================================
# Installs Elegy distribution assets from GitHub releases or
# a local artifacts directory, mirroring install-distribution.ps1.
# ============================================================
set -euo pipefail

# ---- Defaults ----
readonly REPOSITORY="Sofreshx/Elegy"
RELEASE_TAG=""
DESTINATION=""
CLI_SURFACES_RAW=""
WRAPPER_SURFACES_RAW=""
LOCAL_ARTIFACTS_ROOT=""
FORCE=false

# ---- Tooling (set by check_dependencies) ----
DOWNLOAD_CMD=""
DOWNLOAD_OUTPUT_CMD=""
HASH_CMD=""

# ---- Color helpers ----
_info()  { printf "\033[1;34m[INFO]\033[0m %s\n" "$*"; }
_warn()  { printf "\033[1;33m[WARN]\033[0m %s\n" "$*"; }
_error() { printf "\033[1;31m[ERROR]\033[0m %s\n" "$*" >&2; }

# ---- Help ----
_show_help() {
    cat <<'HELP'
Usage: install-distribution.sh [OPTIONS]

Install Elegy distribution assets.

Options:
  -t, --tag TAG                 Release tag to install (default: latest stable)
  -l, --local-artifacts-root PATH  Path to local artifacts (offline/dev installs)
  -d, --destination PATH        Installation target directory (required)
  -c, --cli-surfaces LIST       Comma-separated CLI surface list (default: elegy-cli)
  -w, --wrapper-surfaces LIST   Comma-separated wrapper surface list
  -f, --force                   Overwrite existing installation
  -h, --help                    Show this help message

Supported targets:
  Linux:   x86_64-unknown-linux-gnu
  macOS:   aarch64-apple-darwin

Examples:
  # Install latest release to .elegy
  ./install-distribution.sh -d .elegy

  # Install specific tag with all CLI surfaces
  ./install-distribution.sh -t v0.2.0 -d .elegy -c all

  # Install from local artifacts
  ./install-distribution.sh -l ./artifacts/distribution -d .elegy

  # Install specific CLI and wrapper surfaces
  ./install-distribution.sh -d .elegy -c elegy-cli,elegy-memory -w elegy-obsidian
HELP
    exit 0
}

# ============================================================
# Argument parsing
# ============================================================
_parse_args() {
    while [[ $# -gt 0 ]]; do
        case "$1" in
            -t|--tag)
                RELEASE_TAG="$2"
                shift 2
                ;;
            -l|--local-artifacts-root)
                LOCAL_ARTIFACTS_ROOT="$2"
                shift 2
                ;;
            -d|--destination)
                DESTINATION="$2"
                shift 2
                ;;
            -c|--cli-surfaces)
                CLI_SURFACES_RAW="$2"
                shift 2
                ;;
            -w|--wrapper-surfaces)
                WRAPPER_SURFACES_RAW="$2"
                shift 2
                ;;
            -f|--force)
                FORCE=true
                shift
                ;;
            -h|--help)
                _show_help
                ;;
            *)
                _error "Unknown option: $1"
                _show_help
                ;;
        esac
    done

    if [[ -z "$DESTINATION" ]]; then
        DESTINATION="$(pwd)/.elegy"
        _info "No destination specified, defaulting to: ${DESTINATION}"
    fi
}

# ============================================================
# Dependency check
# ============================================================
_check_deps() {
    local missing=false

    if command -v curl &>/dev/null; then
        DOWNLOAD_CMD="curl -fsSL"
        DOWNLOAD_OUTPUT_CMD="curl -fsSL -o"
    elif command -v wget &>/dev/null; then
        DOWNLOAD_CMD="wget -qO-"
        DOWNLOAD_OUTPUT_CMD="wget -q -O"
    else
        _error "Required tool not found: curl or wget"
        missing=true
    fi

    if ! command -v unzip &>/dev/null; then
        _error "Required tool not found: unzip"
        missing=true
    fi

    if command -v sha256sum &>/dev/null; then
        HASH_CMD="sha256sum"
    elif command -v shasum &>/dev/null; then
        HASH_CMD="shasum -a 256"
    else
        _error "Required tool not found: sha256sum or shasum"
        missing=true
    fi

    if ! command -v jq &>/dev/null; then
        _error "Required tool not found: jq (install via: apt install jq | brew install jq)"
        missing=true
    fi

    if $missing; then
        exit 1
    fi
}

# ============================================================
# Helpers
# ============================================================

# Convert a glob pattern to a regex pattern for jq test()
_glob_to_regex() {
    local pattern="$1"
    local regex
    regex="$(echo "$pattern" | sed 's/\./\\./g; s/\*/.*/g; s/?/./g')"
    echo "^${regex}$"
}

# Compute file SHA-256, returning only the hex hash
_file_sha256() {
    local file="$1"
    if [[ "$HASH_CMD" == "sha256sum" ]]; then
        sha256sum "$file" | awk '{print $1}'
    else
        $HASH_CMD "$file" | awk '{print $1}'
    fi
}

# Download a URL to a file
_download_file() {
    local url="$1" output="$2"
    _info "Downloading: $(basename "$output")"
    $DOWNLOAD_OUTPUT_CMD "$output" "$url"
}

# Download a URL to stdout (for JSON APIs)
_download_stdout() {
    local url="$1"
    $DOWNLOAD_CMD "$url"
}

# ============================================================
# Platform detection
# ============================================================
_detect_target() {
    local os arch
    os="$(uname -s)"
    arch="$(uname -m)"

    case "$os" in
        Linux)
            case "$arch" in
                x86_64|amd64)
                    echo "x86_64-unknown-linux-gnu"
                    ;;
                *)
                    _error "Unsupported Linux architecture: ${arch}. Published Elegy CLI assets currently support only x86_64."
                    exit 1
                    ;;
            esac
            ;;
        Darwin)
            case "$arch" in
                arm64|aarch64)
                    echo "aarch64-apple-darwin"
                    ;;
                x86_64)
                    _warn "macOS x86_64 is not a published target. Trying x86_64-apple-darwin."
                    echo "x86_64-apple-darwin"
                    ;;
                *)
                    _error "Unsupported macOS architecture: ${arch}. Published Elegy CLI assets currently support only Arm64 hosts."
                    exit 1
                    ;;
            esac
            ;;
        *)
            _error "Unsupported operating system: ${os}. Published Elegy CLI assets support Linux and macOS only."
            exit 1
            ;;
    esac
}

# ============================================================
# Surface resolution
# ============================================================
_expand_and_resolve_surfaces() {
    local raw="$1"
    shift
    local known_surfaces=("$@")

    [[ -z "$raw" ]] && return 0

    local -a result=()
    local saved_ifs="$IFS"

    IFS=','
    for entry in $raw; do
        IFS="$saved_ifs"
        # Trim whitespace
        entry="$(echo "$entry" | xargs)"
        [[ -z "$entry" ]] && { IFS=','; continue; }

        if [[ "$entry" == "all" ]]; then
            printf '%s\n' "${known_surfaces[@]}"
            IFS="$saved_ifs"
            return 0
        fi

        # Validate known
        local found=false
        for known in "${known_surfaces[@]}"; do
            if [[ "$entry" == "$known" ]]; then
                found=true
                break
            fi
        done
        if ! $found; then
            _error "Unsupported surface: ${entry}"
            exit 1
        fi

        # Deduplicate
        local dup=false
        for r in "${result[@]}"; do
            [[ "$r" == "$entry" ]] && { dup=true; break; }
        done
        $dup || result+=("$entry")
        IFS=','
    done
    IFS="$saved_ifs"

    printf '%s\n' "${result[@]}"
}

_resolve_cli_surfaces() {
    if [[ -z "$CLI_SURFACES_RAW" ]]; then
        echo "elegy-cli"
        return 0
    fi
    local -a all_cli=(
        "elegy-cli" "elegy-memory" "elegy-mcp" "elegy-planning"
        "elegy-skills" "elegy-configuration" "elegy-documentation"
    )
    _expand_and_resolve_surfaces "$CLI_SURFACES_RAW" "${all_cli[@]}"
}

_resolve_wrapper_surfaces() {
    [[ -z "$WRAPPER_SURFACES_RAW" ]] && return 0
    local -a all_wrapper=(
        "elegy-memory" "elegy-mcp" "elegy-planning" "elegy-skills"
        "elegy-configuration" "elegy-documentation" "elegy-obsidian"
    )
    _expand_and_resolve_surfaces "$WRAPPER_SURFACES_RAW" "${all_wrapper[@]}"
}

# ============================================================
# Directory initialization
# ============================================================
_init_destination() {
    local path="$1"
    if [[ -d "$path" ]]; then
        if ! $FORCE; then
            _error "Destination path already exists: ${path}. Re-run with -f/--force to replace it."
            exit 1
        fi
        _info "Removing existing destination: ${path}"
        rm -rf "$path"
    fi
    mkdir -p "$path"
}

# ============================================================
# GitHub release metadata
# ============================================================
_resolve_release() {
    local tag="$1"
    local api_url

    if [[ -z "$tag" ]]; then
        api_url="https://api.github.com/repos/${REPOSITORY}/releases/latest"
        _info "Fetching latest release metadata from GitHub API"
    else
        local encoded
        encoded="$(echo "$tag" | jq -sRr @uri)"
        api_url="https://api.github.com/repos/${REPOSITORY}/releases/tags/${encoded}"
        _info "Fetching release metadata for tag: ${tag}"
    fi

    _download_stdout "$api_url"
}

# ============================================================
# Asset resolution from GitHub release assets
# ============================================================
_resolve_release_asset_by_pattern() {
    local release_json="$1"
    local description="$2"
    shift 2
    local patterns=("$@")

    # Build jq filter: select assets where name matches any of the patterns
    local jq_filter='.assets[] | select('
    local first=true
    for p in "${patterns[@]}"; do
        local regex
        regex="$(_glob_to_regex "$p")"
        if $first; then
            jq_filter+="(.name | test(\"${regex}\"))"
            first=false
        else
            jq_filter+=" or (.name | test(\"${regex}\"))"
        fi
    done
    jq_filter+=') | {fileName: .name, sourceUri: .browser_download_url, publishedSize: .size}'

    local matches
    matches="$(echo "$release_json" | jq -c "$jq_filter")"
    local count
    count="$(echo "$matches" | grep -c . || true)"

    if [[ "$count" -eq 0 ]]; then
        _error "Unable to locate a release ${description} asset matching patterns: ${patterns[*]}"
        exit 1
    fi
    if [[ "$count" -gt 1 ]]; then
        _error "Ambiguous release ${description} assets. Patterns: ${patterns[*]}. Matches:"
        echo "$matches" | jq -r '.fileName' >&2
        exit 1
    fi

    echo "$matches"
}

_resolve_release_asset_by_name() {
    local release_json="$1"
    local file_name="$2"
    local description="$3"

    local filter=".assets[] | select(.name == \"${file_name}\") | {fileName: .name, sourceUri: .browser_download_url, publishedSize: .size}"

    local matches
    matches="$(echo "$release_json" | jq -c "$filter")"

    [[ -z "$matches" ]] && {
        _error "Unable to locate a release ${description} asset named ${file_name}"
        exit 1
    }

    local count
    count="$(echo "$matches" | grep -c . || true)"
    if [[ "$count" -gt 1 ]]; then
        _error "Ambiguous release ${description} assets named ${file_name}."
        exit 1
    fi

    echo "$matches"
}

# ============================================================
# Asset resolution from local artifacts directory
# ============================================================
_resolve_local_asset_by_pattern() {
    local artifacts_root="$1"
    local description="$2"
    shift 2
    local patterns=("$@")

    local -a candidates=()
    for p in "${patterns[@]}"; do
        while IFS= read -r -d '' f; do
            candidates+=("$f")
        done < <(find "$artifacts_root" -maxdepth 1 -type f -name "$p" -print0 2>/dev/null || true)
    done

    # Deduplicate by basename
    local -A seen
    local -a deduped=()
    for f in "${candidates[@]}"; do
        local bn
        bn="$(basename "$f")"
        [[ -n "${seen[$bn]:-}" ]] && continue
        seen[$bn]="$f"
        deduped+=("$f")
    done

    # Sort by name
    IFS=$'\n' deduped=($(sort <<<"${deduped[*]}")); unset IFS

    local count="${#deduped[@]}"
    if [[ "$count" -eq 0 ]]; then
        _error "Unable to locate a local ${description} asset in ${artifacts_root} matching patterns: ${patterns[*]}"
        exit 1
    fi
    if [[ "$count" -gt 1 ]]; then
        _error "Ambiguous local ${description} assets in ${artifacts_root} matching patterns: ${patterns[*]}. Matches:"
        for f in "${deduped[@]}"; do echo "  $(basename "$f")" >&2; done
        exit 1
    fi

    local f="${deduped[0]}"
    local bname
    bname="$(basename "$f")"
    local fsize
    fsize="$(stat -c%s "$f" 2>/dev/null || stat -f%z "$f" 2>/dev/null)"

    jq -nc --arg fn "$bname" --arg sp "$f" --argjson ps "$fsize" \
        '{fileName: $fn, sourcePath: $sp, publishedSize: $ps}'
}

_resolve_local_asset_by_name() {
    local artifacts_root="$1"
    local file_name="$2"
    local description="$3"

    local source_path="${artifacts_root}/${file_name}"

    if [[ ! -f "$source_path" ]]; then
        _error "Unable to locate a local ${description} asset named ${file_name} in ${artifacts_root}"
        exit 1
    fi
    if [[ -d "$source_path" ]]; then
        _error "Expected a file for local ${description} asset ${file_name} but found a directory at ${source_path}"
        exit 1
    fi

    local fsize
    fsize="$(stat -c%s "$source_path" 2>/dev/null || stat -f%z "$source_path" 2>/dev/null)"

    jq -nc --arg fn "$file_name" --arg sp "$source_path" --argjson ps "$fsize" \
        '{fileName: $fn, sourcePath: $sp, publishedSize: $ps}'
}

# ============================================================
# Copy file from source (local path or HTTP URI)
# ============================================================
_copy_file_from_source() {
    local dest="$1"
    local source_path="${2:-}"
    local source_uri="${3:-}"

    if [[ -n "$source_path" ]]; then
        cp -f "$source_path" "$dest"
    else
        _download_file "$source_uri" "$dest"
    fi
}

# ============================================================
# Executable filename for target
# ============================================================
_get_executable_filename() {
    local binary="$1"
    local target="$2"
    if echo "$target" | grep -qi 'windows'; then
        echo "${binary}.exe"
    else
        echo "$binary"
    fi
}

# ============================================================
# Look up an asset from the release manifest JSON
# ============================================================
_get_manifest_asset() {
    local manifest_file="$1"
    local asset_kind="$2"
    local surface="$3"
    local target="$4"
    local description="$5"

    # Build jq filter with optional surface and target matching.
    # When surface is empty, match assets where surface is null/absent/empty.
    # When target is empty, match assets where target is null/absent/empty.
    local filter='.assets[] | select(.assetKind == $ak'

    if [[ -n "$surface" ]]; then
        filter+=' and .surface == $sf'
    else
        filter+=' and (.surface == null or .surface == "" or .surface == $sf)'
    fi

    if [[ -n "$target" ]]; then
        filter+=' and .target == $tg'
    else
        filter+=' and (.target == null or .target == "" or .target == $tg)'
    fi

    filter+=')'

    local matches
    matches="$(jq -c --arg ak "$asset_kind" --arg sf "${surface:-}" --arg tg "${target:-}" \
        "$filter" "$manifest_file")"

    local count
    count="$(echo "$matches" | grep -c . || true)"

    if [[ "$count" -eq 0 ]]; then
        _error "Manifest metadata did not include a ${description} entry."
        exit 1
    fi
    if [[ "$count" -gt 1 ]]; then
        _error "Manifest metadata resolved multiple ${description} entries."
        exit 1
    fi

    echo "$matches"
}

# ============================================================
# Checksum lookup builder
# ============================================================
_build_checksum_lookup() {
    local checksums_file="$1"
    jq '[.entries[] | {key: .fileName, value: (.sha256 | ascii_downcase)}] | from_entries' "$checksums_file"
}

# ============================================================
# Archive required entries verification
# ============================================================
_assert_archive_required_entries() {
    local archive="$1"
    shift
    local required=("$@")

    [[ ${#required[@]} -eq 0 ]] && return 0

    local missing=false

    # List archive entries, normalize path separators, strip leading ./ or /
    local entries
    entries="$(unzip -lqq "$archive" 2>/dev/null | awk '{print $4}' | sed 's|\\\\|/|g; s|^[\./]*||' | sort -u)"

    for entry in "${required[@]}"; do
        if ! echo "$entries" | grep -qxF "$entry"; then
            _error "Archive ${archive} is missing required entry: ${entry}"
            missing=true
        fi
    done

    if $missing; then
        exit 1
    fi
}

# ============================================================
# Staged file verification against manifest + checksums
# ============================================================
_verify_staged_file() {
    local file_path="$1"
    local manifest_asset="$2"
    local checksum_lookup="$3"
    local published_size="$4"

    local file_name expected_size expected_hash checksum_hash

    file_name="$(echo "$manifest_asset" | jq -r '.fileName')"
    expected_size="$(echo "$manifest_asset" | jq -r '.sizeBytes')"
    expected_hash="$(echo "$manifest_asset" | jq -r '.sha256 | ascii_downcase')"

    # Cross-check with checksums
    checksum_hash="$(echo "$checksum_lookup" | jq -r --arg fn "$file_name" '.[$fn] // empty')"
    if [[ -z "$checksum_hash" ]]; then
        _error "Checksums metadata did not include an entry for ${file_name}"
        exit 1
    fi
    if [[ "$checksum_hash" != "$expected_hash" ]]; then
        _error "Manifest and checksums metadata disagreed on the SHA-256 hash for ${file_name}"
        exit 1
    fi

    # Published size check
    if [[ "$published_size" -gt 0 && "$published_size" -ne "$expected_size" ]]; then
        _error "Published size for ${file_name} was ${published_size} bytes, but the manifest expected ${expected_size} bytes"
        exit 1
    fi

    # File size check
    local file_size
    file_size="$(stat -c%s "$file_path" 2>/dev/null || stat -f%z "$file_path" 2>/dev/null)"
    if [[ "$file_size" -ne "$expected_size" ]]; then
        _error "Staged file ${file_name} was ${file_size} bytes, but the manifest expected ${expected_size} bytes"
        exit 1
    fi

    # SHA-256 check
    local actual_hash
    actual_hash="$(_file_sha256 "$file_path")"
    if [[ "$actual_hash" != "$expected_hash" ]]; then
        _error "SHA-256 mismatch for ${file_name}. Expected ${expected_hash} but found ${actual_hash}"
        exit 1
    fi

    # Return verified file info as JSON fragment
    echo "$manifest_asset" | jq '{fileName: .fileName, sizeBytes: .sizeBytes, sha256: .sha256}'
}

# ============================================================
# Surface metadata
# ============================================================
# CLI surface -> binary name mapping
declare -A _CLI_BINARY
_CLI_BINARY["elegy-cli"]="elegy"
_CLI_BINARY["elegy-memory"]="elegy-memory"
_CLI_BINARY["elegy-mcp"]="elegy-mcp"
_CLI_BINARY["elegy-planning"]="elegy-planning"
_CLI_BINARY["elegy-skills"]="elegy-skills"
_CLI_BINARY["elegy-configuration"]="elegy-configuration"
_CLI_BINARY["elegy-documentation"]="elegy-documentation"

# Wrapper surface -> metadata mapping (assetPrefix, installer, skillBridge)
declare -A _WRAPPER_META

_populate_wrapper_metadata() {
    _WRAPPER_META["elegy-memory_assetPrefix"]="elegy-memory-wrapper"
    _WRAPPER_META["elegy-memory_installer"]="install.ps1"
    _WRAPPER_META["elegy-memory_skillBridge"]="skills/elegy-memory/SKILL.md"

    _WRAPPER_META["elegy-mcp_assetPrefix"]="elegy-mcp-wrapper"
    _WRAPPER_META["elegy-mcp_installer"]="install.ps1"
    _WRAPPER_META["elegy-mcp_skillBridge"]="skills/elegy-mcp/SKILL.md"

    _WRAPPER_META["elegy-planning_assetPrefix"]="elegy-planning-wrapper"
    _WRAPPER_META["elegy-planning_installer"]="install.ps1"
    _WRAPPER_META["elegy-planning_skillBridge"]="skills/elegy-planning/SKILL.md"

    _WRAPPER_META["elegy-skills_assetPrefix"]="elegy-skills-wrapper"
    _WRAPPER_META["elegy-skills_installer"]="install.ps1"
    _WRAPPER_META["elegy-skills_skillBridge"]="skills/elegy-skills/SKILL.md"

    _WRAPPER_META["elegy-configuration_assetPrefix"]="elegy-configuration-wrapper"
    _WRAPPER_META["elegy-configuration_installer"]="install.ps1"
    _WRAPPER_META["elegy-configuration_skillBridge"]="skills/elegy-configuration/SKILL.md"

    _WRAPPER_META["elegy-documentation_assetPrefix"]="elegy-documentation-wrapper"
    _WRAPPER_META["elegy-documentation_installer"]="install.ps1"
    _WRAPPER_META["elegy-documentation_skillBridge"]="skills/elegy-documentation/SKILL.md"

    _WRAPPER_META["elegy-obsidian_assetPrefix"]="elegy-obsidian-wrapper"
    _WRAPPER_META["elegy-obsidian_installer"]="install.ps1"
    _WRAPPER_META["elegy-obsidian_skillBridge"]="skills/elegy-obsidian/SKILL.md"
}
_populate_wrapper_metadata

# ============================================================
# Main
# ============================================================
main() {
    _parse_args "$@"
    _check_deps

    # ---- Platform ----
    local resolved_target
    resolved_target="$(_detect_target)"
    _info "Detected host target: ${resolved_target}"

    # ---- Surface resolution ----
    local -a cli_surfaces wrapper_surfaces
    IFS=$'\n' read -r -d '' -a cli_surfaces < <(_resolve_cli_surfaces && printf '\0')
    IFS=$'\n' read -r -d '' -a wrapper_surfaces < <(_resolve_wrapper_surfaces && printf '\0') || true

    _info "Requested CLI surfaces: ${cli_surfaces[*]:-(none)}"
    _info "Requested wrapper surfaces: ${wrapper_surfaces[*]:-(none)}"

    # ---- Destination initialization ----
    local abs_dest
    abs_dest="$(cd "$(dirname "$DESTINATION")" && pwd)/$(basename "$DESTINATION")"
    _init_destination "$abs_dest"

    local download_dir="${abs_dest}/downloads"
    local contracts_dir="${abs_dest}/contracts"
    local bin_dir="${abs_dest}/bin"
    local wrapper_dir="${abs_dest}/wrappers"
    local legacy_cli_dir="${abs_dest}/cli"

    mkdir -p "$download_dir" "$contracts_dir" "$bin_dir"
    if [[ ${#wrapper_surfaces[@]} -gt 0 ]]; then
        mkdir -p "$wrapper_dir"
    fi

    # ---- Source resolution (GitHub release or local artifacts) ----
    local release_json=""
    local resolved_tag=""
    local resolved_local_root=""
    local is_local=false

    local manifest_source_json checksums_source_json

    if [[ -z "$LOCAL_ARTIFACTS_ROOT" ]]; then
        # GitHub release mode
        release_json="$(_resolve_release "$RELEASE_TAG")"
        resolved_tag="$(echo "$release_json" | jq -r '.tag_name // empty')"
        if [[ -z "$resolved_tag" ]]; then
            _error "Resolved GitHub release metadata did not include a tag name."
            exit 1
        fi
        _info "Resolved release tag: ${resolved_tag}"

        # Find manifest and checksums assets in the release by pattern
        manifest_source_json="$(_resolve_release_asset_by_pattern \
            "$release_json" "release manifest" "elegy-release-manifest-*.json")"
        checksums_source_json="$(_resolve_release_asset_by_pattern \
            "$release_json" "release checksums" "elegy-release-checksums-*.json")"
    else
        # Local artifacts mode
        is_local=true
        resolved_local_root="$(cd "$LOCAL_ARTIFACTS_ROOT" && pwd)"
        resolved_tag="local-artifacts"
        _info "Local artifacts root: ${resolved_local_root}"

        manifest_source_json="$(_resolve_local_asset_by_pattern \
            "$resolved_local_root" "release manifest" "elegy-release-manifest-*.json")"
        checksums_source_json="$(_resolve_local_asset_by_pattern \
            "$resolved_local_root" "release checksums" "elegy-release-checksums-*.json")"
    fi

    # ---- Download manifest & checksums ----
    local manifest_src_name checksums_src_name
    local manifest_src_uri="" manifest_src_path=""
    local checksums_src_uri="" checksums_src_path=""

    manifest_src_name="$(echo "$manifest_source_json" | jq -r '.fileName')"
    manifest_src_uri="$(echo "$manifest_source_json" | jq -r '.sourceUri // ""')"
    manifest_src_path="$(echo "$manifest_source_json" | jq -r '.sourcePath // ""')"

    checksums_src_name="$(echo "$checksums_source_json" | jq -r '.fileName')"
    checksums_src_uri="$(echo "$checksums_source_json" | jq -r '.sourceUri // ""')"
    checksums_src_path="$(echo "$checksums_source_json" | jq -r '.sourcePath // ""')"

    local manifest_file="${download_dir}/${manifest_src_name}"
    local checksums_file="${download_dir}/${checksums_src_name}"

    _copy_file_from_source "$manifest_file" "$manifest_src_path" "$manifest_src_uri"
    _copy_file_from_source "$checksums_file" "$checksums_src_path" "$checksums_src_uri"

    # ---- Validate manifest document ----
    local manifest_docType manifest_schemaVersion manifest_bundleVersion manifest_tag

    manifest_docType="$(jq -r '.documentType // ""' "$manifest_file")"
    if [[ "$manifest_docType" != "elegy-release-manifest" ]]; then
        _error "Release manifest metadata had an unexpected documentType: ${manifest_docType}"
        exit 1
    fi

    manifest_schemaVersion="$(jq -r '.schemaVersion // ""' "$manifest_file")"
    if [[ -z "$manifest_schemaVersion" ]]; then
        _error "Release manifest metadata did not include schemaVersion."
        exit 1
    fi

    manifest_bundleVersion="$(jq -r '.bundleVersion // ""' "$manifest_file")"
    if [[ -z "$manifest_bundleVersion" ]]; then
        _error "Release manifest metadata did not include bundleVersion."
        exit 1
    fi

    manifest_tag="$(jq -r '.tag // ""' "$manifest_file")"

    # ---- Validate checksums document ----
    local checksums_docType checksums_schemaVersion checksums_tag checksums_algorithm

    checksums_docType="$(jq -r '.documentType // ""' "$checksums_file")"
    if [[ "$checksums_docType" != "elegy-release-checksums" ]]; then
        _error "Release checksums metadata had an unexpected documentType: ${checksums_docType}"
        exit 1
    fi

    checksums_schemaVersion="$(jq -r '.schemaVersion // ""' "$checksums_file")"
    if [[ -z "$checksums_schemaVersion" ]]; then
        _error "Release checksums metadata did not include schemaVersion."
        exit 1
    fi

    checksums_tag="$(jq -r '.tag // ""' "$checksums_file")"
    if [[ "$manifest_tag" != "$checksums_tag" ]]; then
        _error "Release manifest and checksums metadata did not agree on the resolved tag marker."
        exit 1
    fi

    checksums_algorithm="$(jq -r '.algorithm // ""' "$checksums_file" | tr '[:upper:]' '[:lower:]')"
    if [[ "$checksums_algorithm" != "sha256" ]]; then
        _error "Unsupported checksums algorithm: ${checksums_algorithm}"
        exit 1
    fi

    # ---- Cross-validate with repository and tag (GitHub mode) ----
    if ! $is_local; then
        local manifest_repo
        manifest_repo="$(jq -r '.repository // ""' "$manifest_file" | tr '[:upper:]' '[:lower:]')"
        local expected_repo
        expected_repo="$(echo "$REPOSITORY" | tr '[:upper:]' '[:lower:]')"
        if [[ "$manifest_repo" != "$expected_repo" ]]; then
            _error "Release manifest metadata targeted repository ${manifest_repo}, but installer expected ${expected_repo}"
            exit 1
        fi
        if [[ "$manifest_tag" != "$resolved_tag" ]]; then
            _error "Release manifest metadata targeted tag ${manifest_tag}, but installer resolved tag ${resolved_tag}"
            exit 1
        fi
    else
        if [[ "$manifest_tag" != "local-artifacts" ]]; then
            _error "Local artifact installs require manifest tag marker 'local-artifacts', but found ${manifest_tag}"
            exit 1
        fi
    fi

    # ---- Build checksum lookup and verify manifest file ----
    local checksum_lookup
    checksum_lookup="$(_build_checksum_lookup "$checksums_file")"

    # Verify manifest SHA-256 against checksums
    local manifest_expected_sha
    manifest_expected_sha="$(echo "$checksum_lookup" | jq -r --arg fn "$manifest_src_name" '.[$fn] // empty')"
    if [[ -z "$manifest_expected_sha" ]]; then
        _error "Checksums metadata did not include the manifest file ${manifest_src_name}"
        exit 1
    fi

    local manifest_actual_sha
    manifest_actual_sha="$(_file_sha256 "$manifest_file")"
    if [[ "$manifest_actual_sha" != "$manifest_expected_sha" ]]; then
        _error "Release manifest SHA-256 did not match the published checksums entry for ${manifest_src_name}"
        exit 1
    fi

    # ---- Verify published targets include our host ----
    if [[ ${#cli_surfaces[@]} -gt 0 ]]; then
        local published_targets=()
        while IFS= read -r t; do
            published_targets+=("$(echo "$t" | tr '[:upper:]' '[:lower:]')")
        done < <(jq -r '.publishedTargets[] // empty' "$manifest_file")

        local target_lower
        target_lower="$(echo "$resolved_target" | tr '[:upper:]' '[:lower:]')"
        local found_target=false
        for pt in "${published_targets[@]}"; do
            [[ "$pt" == "$target_lower" ]] && { found_target=true; break; }
        done
        if ! $found_target; then
            _error "Release metadata did not publish the current host target ${resolved_target}"
            exit 1
        fi
    fi

    # ---- Verified files list (starts with manifest) ----
    local manifest_file_size
    manifest_file_size="$(stat -c%s "$manifest_file" 2>/dev/null || stat -f%z "$manifest_file" 2>/dev/null)"

    local -a verified_files=()
    verified_files+=("$(jq -nc \
        --arg fn "$manifest_src_name" \
        --argjson sz "$manifest_file_size" \
        --arg sha "$manifest_actual_sha" \
        '{fileName: $fn, sizeBytes: $sz, sha256: $sha}')")

    local -a installed_assets=()
    local -a installed_cli_reports=()
    local -a installed_wrapper_reports=()

    # ============================================================
    # Contracts bundle
    # ============================================================
    _info "Processing contracts bundle..."

    local contracts_manifest_asset
    contracts_manifest_asset="$(_get_manifest_asset \
        "$manifest_file" "contracts-bundle" "" "" "contracts bundle")"

    local contracts_src_name
    contracts_src_name="$(echo "$contracts_manifest_asset" | jq -r '.fileName')"

    local contracts_src_uri="" contracts_src_path=""
    local contracts_published_size=0

    if ! $is_local; then
        local contracts_release_asset
        contracts_release_asset="$(_resolve_release_asset_by_name \
            "$release_json" "$contracts_src_name" "contracts bundle")"
        contracts_src_uri="$(echo "$contracts_release_asset" | jq -r '.sourceUri')"
        contracts_published_size="$(echo "$contracts_release_asset" | jq -r '.publishedSize')"
    else
        local contracts_local_asset
        contracts_local_asset="$(_resolve_local_asset_by_name \
            "$resolved_local_root" "$contracts_src_name" "contracts bundle")"
        contracts_src_path="$(echo "$contracts_local_asset" | jq -r '.sourcePath')"
        contracts_published_size="$(echo "$contracts_local_asset" | jq -r '.publishedSize')"
    fi

    local contracts_archive="${download_dir}/${contracts_src_name}"
    _copy_file_from_source "$contracts_archive" "$contracts_src_path" "$contracts_src_uri"

    # Verify contracts archive
    local verified_contracts
    verified_contracts="$(_verify_staged_file \
        "$contracts_archive" "$contracts_manifest_asset" "$checksum_lookup" "$contracts_published_size")"
    verified_files+=("$verified_contracts")

    # Verify required archive entries
    local -a contracts_required_entries=()
    while IFS= read -r e; do
        contracts_required_entries+=("$e")
    done < <(echo "$contracts_manifest_asset" | jq -r '.requiredEntries[] // empty' 2>/dev/null || true)
    _assert_archive_required_entries "$contracts_archive" "${contracts_required_entries[@]}"

    # Extract contracts
    _info "Extracting contracts bundle to ${contracts_dir}"
    unzip -qo "$contracts_archive" -d "$contracts_dir"

    # Record installed asset
    local contracts_size_bytes contracts_sha256
    contracts_size_bytes="$(echo "$contracts_manifest_asset" | jq -r '.sizeBytes')"
    contracts_sha256="$(echo "$contracts_manifest_asset" | jq -r '.sha256')"
    installed_assets+=("$(jq -nc \
        --arg ak "contracts-bundle" \
        --arg fn "$contracts_src_name" \
        --arg ip "$contracts_dir" \
        --argjson re "$(echo "$contracts_manifest_asset" | jq -c '.requiredEntries // []')" \
        --argjson sz "$contracts_size_bytes" \
        --arg sha "$contracts_sha256" \
        '{assetKind: $ak, surface: null, target: null, fileName: $fn, installPath: $ip, requiredEntries: $re, sizeBytes: $sz, sha256: $sha}')")

    # ============================================================
    # CLI surfaces
    # ============================================================
    for surface in "${cli_surfaces[@]}"; do
        _info "Processing CLI surface: ${surface}"

        local cli_manifest_asset
        cli_manifest_asset="$(_get_manifest_asset \
            "$manifest_file" "cli" "$surface" "$resolved_target" \
            "${surface} CLI archive for ${resolved_target}")"

        local cli_src_name
        cli_src_name="$(echo "$cli_manifest_asset" | jq -r '.fileName')"

        local cli_src_uri="" cli_src_path=""
        local cli_published_size=0

        if ! $is_local; then
            local cli_release_asset
            cli_release_asset="$(_resolve_release_asset_by_name \
                "$release_json" "$cli_src_name" "${surface} CLI archive")"
            cli_src_uri="$(echo "$cli_release_asset" | jq -r '.sourceUri')"
            cli_published_size="$(echo "$cli_release_asset" | jq -r '.publishedSize')"
        else
            local cli_local_asset
            cli_local_asset="$(_resolve_local_asset_by_name \
                "$resolved_local_root" "$cli_src_name" "${surface} CLI archive")"
            cli_src_path="$(echo "$cli_local_asset" | jq -r '.sourcePath')"
            cli_published_size="$(echo "$cli_local_asset" | jq -r '.publishedSize')"
        fi

        local cli_archive="${download_dir}/${cli_src_name}"
        local surface_path="${bin_dir}/${surface}"
        mkdir -p "$surface_path"

        _copy_file_from_source "$cli_archive" "$cli_src_path" "$cli_src_uri"

        # Verify CLI archive
        local verified_cli
        verified_cli="$(_verify_staged_file \
            "$cli_archive" "$cli_manifest_asset" "$checksum_lookup" "$cli_published_size")"
        verified_files+=("$verified_cli")

        # Verify required archive entries
        local -a cli_required_entries=()
        while IFS= read -r e; do
            cli_required_entries+=("$e")
        done < <(echo "$cli_manifest_asset" | jq -r '.requiredEntries[] // empty' 2>/dev/null || true)
        _assert_archive_required_entries "$cli_archive" "${cli_required_entries[@]}"

        # Extract
        _info "Extracting ${surface} CLI to ${surface_path}"
        unzip -qo "$cli_archive" -d "$surface_path"

        # Verify executable exists
        local binary_name="${_CLI_BINARY[$surface]}"
        local executable_name
        executable_name="$(_get_executable_filename "$binary_name" "$resolved_target")"
        local executable_path="${surface_path}/${executable_name}"

        if [[ ! -f "$executable_path" ]]; then
            _error "Installed CLI executable was not found at ${executable_path}"
            exit 1
        fi

        # Restore executable permission
        chmod +x "$executable_path"

        # Legacy compatibility copy for elegy-cli
        if [[ "$surface" == "elegy-cli" ]]; then
            _info "Creating legacy compatibility copy in ${legacy_cli_dir}"
            mkdir -p "$legacy_cli_dir"
            cp -f "$surface_path"/* "$legacy_cli_dir"/
            local legacy_executable="${legacy_cli_dir}/${executable_name}"
            if [[ ! -f "$legacy_executable" ]]; then
                _error "Installed compatibility CLI executable was not found at ${legacy_executable}"
                exit 1
            fi
            chmod +x "$legacy_executable"
        fi

        # Record installed CLI report
        installed_cli_reports+=("$(jq -nc \
            --arg s "$surface" \
            --arg a "$cli_src_name" \
            --arg ip "$surface_path" \
            --arg ep "$executable_path" \
            '{Surface: $s, Asset: $a, InstallPath: $ip, ExecutablePath: $ep}')")

        # Record installed asset
        local cli_size_bytes cli_sha256
        cli_size_bytes="$(echo "$cli_manifest_asset" | jq -r '.sizeBytes')"
        cli_sha256="$(echo "$cli_manifest_asset" | jq -r '.sha256')"
        installed_assets+=("$(jq -nc \
            --arg ak "cli" \
            --arg s "$surface" \
            --arg tg "$resolved_target" \
            --arg fn "$cli_src_name" \
            --arg ip "$surface_path" \
            --arg ep "$executable_path" \
            --argjson re "$(echo "$cli_manifest_asset" | jq -c '.requiredEntries // []')" \
            --argjson sz "$cli_size_bytes" \
            --arg sha "$cli_sha256" \
            '{assetKind: $ak, surface: $s, target: $tg, fileName: $fn, installPath: $ip, executablePath: $ep, requiredEntries: $re, sizeBytes: $sz, sha256: $sha}')")
    done

    # ============================================================
    # Wrapper surfaces
    # ============================================================
    for surface in "${wrapper_surfaces[@]}"; do
        _info "Processing wrapper surface: ${surface}"

        local wrapper_manifest_asset
        wrapper_manifest_asset="$(_get_manifest_asset \
            "$manifest_file" "wrapper" "$surface" "" \
            "${surface} wrapper archive")"

        local wrapper_src_name
        wrapper_src_name="$(echo "$wrapper_manifest_asset" | jq -r '.fileName')"

        local wrapper_src_uri="" wrapper_src_path=""
        local wrapper_published_size=0

        if ! $is_local; then
            local wrapper_release_asset
            wrapper_release_asset="$(_resolve_release_asset_by_name \
                "$release_json" "$wrapper_src_name" "${surface} wrapper archive")"
            wrapper_src_uri="$(echo "$wrapper_release_asset" | jq -r '.sourceUri')"
            wrapper_published_size="$(echo "$wrapper_release_asset" | jq -r '.publishedSize')"
        else
            local wrapper_local_asset
            wrapper_local_asset="$(_resolve_local_asset_by_name \
                "$resolved_local_root" "$wrapper_src_name" "${surface} wrapper archive")"
            wrapper_src_path="$(echo "$wrapper_local_asset" | jq -r '.sourcePath')"
            wrapper_published_size="$(echo "$wrapper_local_asset" | jq -r '.publishedSize')"
        fi

        local wrapper_archive="${download_dir}/${wrapper_src_name}"
        local surface_path="${wrapper_dir}/${surface}"
        mkdir -p "$surface_path"

        _copy_file_from_source "$wrapper_archive" "$wrapper_src_path" "$wrapper_src_uri"

        # Verify wrapper archive
        local verified_wrapper
        verified_wrapper="$(_verify_staged_file \
            "$wrapper_archive" "$wrapper_manifest_asset" "$checksum_lookup" "$wrapper_published_size")"
        verified_files+=("$verified_wrapper")

        # Verify required archive entries
        local -a wrapper_required_entries=()
        while IFS= read -r e; do
            wrapper_required_entries+=("$e")
        done < <(echo "$wrapper_manifest_asset" | jq -r '.requiredEntries[] // empty' 2>/dev/null || true)
        _assert_archive_required_entries "$wrapper_archive" "${wrapper_required_entries[@]}"

        # Extract
        _info "Extracting ${surface} wrapper to ${surface_path}"
        unzip -qo "$wrapper_archive" -d "$surface_path"

        # Verify installer and skill bridge exist
        local installer_filename
        installer_filename="${_WRAPPER_META[${surface}_installer]:-}"
        local skill_bridge_path
        skill_bridge_path="${_WRAPPER_META[${surface}_skillBridge]:-}"

        local installer_path="${surface_path}/${installer_filename}"
        local skill_bridge_full="${surface_path}/${skill_bridge_path}"

        if [[ ! -f "$installer_path" ]]; then
            _error "Installed wrapper installer was not found at ${installer_path}"
            exit 1
        fi
        if [[ ! -f "$skill_bridge_full" ]]; then
            _error "Installed wrapper skill bridge was not found at ${skill_bridge_full}"
            exit 1
        fi

        # Record installed wrapper report
        installed_wrapper_reports+=("$(jq -nc \
            --arg s "$surface" \
            --arg a "$wrapper_src_name" \
            --arg ip "$surface_path" \
            --arg ipath "$installer_path" \
            --arg spath "$skill_bridge_full" \
            '{Surface: $s, Asset: $a, InstallPath: $ip, InstallerPath: $ipath, SkillBridgePath: $spath}')")

        # Record installed asset
        local wrapper_size_bytes wrapper_sha256
        wrapper_size_bytes="$(echo "$wrapper_manifest_asset" | jq -r '.sizeBytes')"
        wrapper_sha256="$(echo "$wrapper_manifest_asset" | jq -r '.sha256')"
        installed_assets+=("$(jq -nc \
            --arg ak "wrapper" \
            --arg s "$surface" \
            --arg fn "$wrapper_src_name" \
            --arg ip "$surface_path" \
            --arg ipath "$installer_path" \
            --arg spath "$skill_bridge_full" \
            --argjson re "$(echo "$wrapper_manifest_asset" | jq -c '.requiredEntries // []')" \
            --argjson sz "$wrapper_size_bytes" \
            --arg sha "$wrapper_sha256" \
            '{assetKind: $ak, surface: $s, target: null, fileName: $fn, installPath: $ip, installerPath: $ipath, skillBridgePath: $spath, requiredEntries: $re, sizeBytes: $sz, sha256: $sha}')")
    done

    # ============================================================
    # Write install-receipt.json
    # ============================================================
    local installed_at_utc
    installed_at_utc="$(date -u +"%Y-%m-%dT%H:%M:%SZ")"
    local verified_at_utc="$installed_at_utc"

    local source_mode
    if $is_local; then
        source_mode="local-artifacts"
    else
        source_mode="github-release"
    fi

    # Build verified files JSON array
    local verified_files_json
    verified_files_json="$(printf '%s\n' "${verified_files[@]}" | jq -s '.' )"

    # Build installed assets JSON array
    local installed_assets_json
    installed_assets_json="$(printf '%s\n' "${installed_assets[@]}" | jq -s '.' )"

    # Build source object
    local source_json
    if $is_local; then
        source_json="$(jq -nc \
            --arg mode "$source_mode" \
            --arg tag "$manifest_tag" \
            --arg root "$resolved_local_root" \
            --arg mf "$manifest_src_name" \
            --arg cs "$checksums_src_name" \
            '{mode: $mode, tag: $tag, localArtifactsRoot: $root, manifest: $mf, checksums: $cs}')"
    else
        source_json="$(jq -nc \
            --arg mode "$source_mode" \
            --arg repo "$REPOSITORY" \
            --arg tag "$manifest_tag" \
            --arg mf "$manifest_src_name" \
            --arg cs "$checksums_src_name" \
            '{mode: $mode, repository: $repo, tag: $tag, manifest: $mf, checksums: $cs}')"
    fi

    # Build request object
    local request_json
    request_json="$(jq -nc \
        --arg dest "$abs_dest" \
        --argjson cli "$(printf '%s\n' "${cli_surfaces[@]}" | jq -R . | jq -s .)" \
        --argjson wrp "$(printf '%s\n' "${wrapper_surfaces[@]}" | jq -R . | jq -s .)" \
        --argjson force "$FORCE" \
        '{destination: $dest, cliSurfaces: $cli, wrapperSurfaces: $wrp, force: $force}')"

    local receipt
    receipt="$(jq -nc \
        --arg sv "elegy-install-receipt/v1" \
        --arg dt "elegy-install-receipt" \
        --arg iat "$installed_at_utc" \
        --argjson request "$request_json" \
        --argjson source "$source_json" \
        --arg target "$resolved_target" \
        --argjson verification "$(jq -nc \
            --arg alg "sha256" \
            --arg bv "$manifest_bundleVersion" \
            --arg vat "$verified_at_utc" \
            --argjson files "$verified_files_json" \
            '{algorithm: $alg, manifestBundleVersion: $bv, verifiedAtUtc: $vat, files: $files}')" \
        --argjson assets "$installed_assets_json" \
        '{
            schemaVersion: $sv,
            documentType: $dt,
            installedAtUtc: $iat,
            request: $request,
            source: $source,
            hostTarget: $target,
            verification: $verification,
            installedAssets: $assets
        }')"

    local receipt_path="${abs_dest}/install-receipt.json"
    echo "$receipt" > "$receipt_path"
    _info "Install receipt written to: ${receipt_path}"

    # ============================================================
    # Summary
    # ============================================================
    echo ""
    _info "Installed Elegy distribution assets."
    if $is_local; then
        _info " - local artifacts root: ${resolved_local_root}"
    else
        _info " - repository: ${REPOSITORY}"
    fi
    _info " - release tag: ${resolved_tag}"
    _info " - contracts path: ${contracts_dir}"

    for report in "${installed_cli_reports[@]}"; do
        local s a ip ep
        s="$(echo "$report" | jq -r '.Surface')"
        a="$(echo "$report" | jq -r '.Asset')"
        ip="$(echo "$report" | jq -r '.InstallPath')"
        ep="$(echo "$report" | jq -r '.ExecutablePath')"
        echo " - CLI surface: ${s}"
        echo "   asset: ${a}"
        echo "   path: ${ip}"
        echo "   executable: ${ep}"
    done

    for report in "${installed_wrapper_reports[@]}"; do
        local s a ip ipath spath
        s="$(echo "$report" | jq -r '.Surface')"
        a="$(echo "$report" | jq -r '.Asset')"
        ip="$(echo "$report" | jq -r '.InstallPath')"
        ipath="$(echo "$report" | jq -r '.InstallerPath')"
        spath="$(echo "$report" | jq -r '.SkillBridgePath')"
        echo " - wrapper surface: ${s}"
        echo "   asset: ${a}"
        echo "   path: ${ip}"
        echo "   installer: ${ipath}"
        echo "   skill bridge: ${spath}"
    done

    if [[ " ${cli_surfaces[*]} " == *" elegy-cli "* ]]; then
        echo " - compatibility cli path: ${legacy_cli_dir}"
    fi

    _info "Install receipt: ${receipt_path}"
    _info "Installation complete."
}

main "$@"
