#!/usr/bin/env bash
set -euo pipefail

TARGET="${TARGET:-x86_64-unknown-linux-gnu}"
DIST_DIR="${DIST_DIR:-dist/codex/${TARGET}}"
IMAGE="${IMAGE:-ubuntu:24.04}"

if ! command -v docker >/dev/null 2>&1; then
  echo "docker is required for container Codex verification" >&2
  exit 1
fi

if [[ ! -f "${DIST_DIR}/.agents/plugins/marketplace.json" ]]; then
  echo "${DIST_DIR}/.agents/plugins/marketplace.json is missing" >&2
  echo "Generate it with: cargo run -p elegy-tooling --bin elegy-plugin-packaging -- marketplace export-codex --source . --target ${TARGET} --output ${DIST_DIR}" >&2
  exit 1
fi

docker run --rm \
  -e CODEX_NON_INTERACTIVE=1 \
  -e OPENAI_API_KEY="${OPENAI_API_KEY:-}" \
  -v "$(pwd)/${DIST_DIR}:/workspace/dist/codex/${TARGET}:ro" \
  "${IMAGE}" \
  bash -lc "
    set -euo pipefail
    export DEBIAN_FRONTEND=noninteractive
    apt-get update >/dev/null
    apt-get install -y curl ca-certificates jq >/dev/null
    export CODEX_HOME=/tmp/codex-home
    export HOME=/tmp/codex-home-user
    mkdir -p \"\$CODEX_HOME\" \"\$HOME\"
    curl -fsSL https://chatgpt.com/codex/install.sh | CODEX_NON_INTERACTIVE=1 sh
    export PATH=\"\$HOME/.local/bin:\$PATH\"
    codex --version
    codex plugin marketplace add /workspace/dist/codex/${TARGET} --json | tee /tmp/marketplace-add.json
    jq -e '.marketplaceName == \"elegy\" or .name == \"elegy\"' /tmp/marketplace-add.json >/dev/null
    codex plugin marketplace list --json
    codex plugin list --marketplace elegy --available --json | tee /tmp/available.json
    jq -e '.. | objects | select(.name? == \"elegy-planning\")' /tmp/available.json >/dev/null
    codex plugin add elegy-planning@elegy --json | tee /tmp/plugin-add.json
    jq -e '.. | objects | select((.name? == \"elegy-planning\") and ((.version? // \"\") | contains(\"+codex.\")))' /tmp/plugin-add.json >/dev/null
    codex plugin list --marketplace elegy --json | tee /tmp/installed.json
    jq -e '.. | objects | select((.name? == \"elegy-planning\") and ((.installed? == true) or (.enabled? == true) or (.status? == \"installed\") or (.status? == \"enabled\")))' /tmp/installed.json >/dev/null
  "
