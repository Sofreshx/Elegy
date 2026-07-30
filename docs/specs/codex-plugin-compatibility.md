---
title: Codex Plugin Compatibility
status: active
owner: Elegy
lastVerified: 2026-07-30
---

# Codex plugin compatibility

`elegy-plugin/v3` targets deterministic Codex package projection. Current
evidence proves validated structural preservation for the fixture shapes in
this repository. It does not yet prove complete Codex package/runtime parity.

## Projection rule

The v3 top level preserves Codex-native metadata, skills, MCP servers, apps,
hooks, interface/UI metadata, assets, and future unknown native fields. Known
multi-shape fields receive structural validation; unknown fields are preserved
but are not claimed to be understood. Elegy governance is isolated under
`elegy`.

Codex export serializes v3 and removes only:

- `schemaVersion`;
- `elegy`.

Changing, renaming, normalizing, or silently omitting any other field is a
lossy export and therefore an error. Import preserves accepted inline or path
forms and unknown native fields.

## Dated parity matrix

| Package/runtime area | v3 behavior | Evidence status |
|---|---|---|
| name, version, description, author, homepage, repository, license, keywords | Serialized unchanged | fixture-proven structural |
| skills | Non-empty path/list/inline structural shapes preserved | fixture-proven structural |
| MCP servers | Path or inline server-map shape preserved; package verification resolves path descriptors and auth declarations | fixture-proven structural |
| apps | Non-empty path/list/inline structural shapes preserved | fixture-proven structural |
| hooks | Non-empty path/list/inline structural shapes preserved | fixture-proven structural |
| interface/UI metadata | Typed metadata and referenced assets preserved | fixture-proven structural |
| assets | Declared paths are copied by pack/export | source-tested structural |
| unknown native fields | Manifest value is preserved; semantics and referenced files are not inferred | opaque, not parity evidence |
| expected MCP auth | `elegy.mcpAuthentication`, removed only for Codex after representability validation | governed |
| capability/readiness/connections/classification | `elegy` namespace; never emitted as native Codex fields | governed |
| marketplace install/auth policies | `elegy-marketplace/v2`; `NOT_AVAILABLE` is enforced and non-local sources remain descriptor-only | source-tested governance |
| clean Codex install, enable/disable, startup, apps/hooks execution, UI behavior | Requires a reviewed clean Codex receipt | not yet evidenced |
| OpenAI hosted review, org administration, universal directory, hosted backend | Not implemented by Elegy | out of scope |

## Authentication

Each inline MCP server has an expected mode:

- `none`: local/explicitly unauthenticated boundary;
- `mcp-oauth`: the remote server and identity provider own OAuth, while Codex
  performs login and holds tokens;
- `bearer-env`: the host supplies the named environment variable declared in
  the Elegy authentication expectation.

The manifest must not contain credentials, secret-bearing headers, tokens,
client secrets, or private OAuth endpoints. MCP-native tool security schemes,
annotations, output schemas, and `_meta` remain MCP fields and are not
translated into an Elegy permission system.

## Other hosts

Claude and OpenCode export rejects unsupported apps, hooks, UI, connection
bindings, authentication, or unknown fields. `--allow-lossy` is a maintainer
inspection override: it emits `projection-report.json`, lists every loss, and
marks the result non-routable.

## Legacy

v1/v2 inputs may be inspected or installed for backward compatibility. They
cannot be exported, published, newly generated, or enter default discovery.
Their historical extension translation is not Codex parity.

The release CLI enforces that prohibition. Public v1 archive helpers remain in
the Rust SDK only for legacy migration and fixture coverage; they are not
publication entrypoints and their output cannot enter v2 marketplace
generation or default discovery.

## Validation

```powershell
cargo test -p elegy-plugin-sdk
cargo run -p elegy-plugin-sdk --bin elegy-plugin-schemas -- --check
cargo test -p elegy-tooling
```

The generated JSON schema enforces the v3 envelope and supported top-level
Codex field shapes. Publication still requires the canonical Rust validator
and verifier for authentication semantics, governed-file consistency, and
package-path existence; schema acceptance alone is never readiness evidence.
