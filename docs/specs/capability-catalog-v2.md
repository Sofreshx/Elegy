---
title: Capability Catalog V2
status: active
owner: elegy-core
doc_kind: spec
---

# Capability Catalog V2

## Authority and scope

`elegy-capability-catalog/v2` is the current authority for the interface that
an Elegy capability exposes. The catalog is portable and host-neutral. It
describes invocation shape; it does not prove that the implementation is
installed, conformant, live, or routable.

The v1 contract remains readable for compatibility. New catalogs and new
capability entries use v2 terminology and rules in this document.

## One concrete interface per entry

Every capability entry declares exactly one `kind`. A kind is the concrete
interface a host invokes, not a list of possible transports:

| Kind | Meaning | Typical host surface |
| --- | --- | --- |
| `cli` | A local executable command with a governed invocation template. | `elegy-*` binary or a host subprocess lane |
| `mcp-resource` | An addressable MCP resource, normally read-only context or data. | MCP `resources/read` and resource discovery |
| `mcp-tool` | A typed MCP operation with input/output schemas and side-effect policy. | MCP `tools/call` |

An entry must not combine these kinds, encode a second primary interface in a
field, or rely on host-specific inference. If the same behavior is intentionally
available through more than one interface, publish separate capability entries
with distinct IDs and explicit provenance.

`mcp-resource` and `mcp-tool` are first-class interfaces. They are not merely
documentation or generated projections of a CLI entry. A host may still choose
the CLI lane when it is the default integration boundary or when MCP is not
available.

## Legacy compatibility kinds

The v1 kinds `mcp` and `app-binding` may be read while old packages migrate.
They are not current v2 authoring targets:

- `mcp` must be classified as either `mcp-resource` or `mcp-tool` during
  migration; do not emit new `mcp` entries.
- `app-binding` is legacy compatibility metadata. It is valid only when the
  package also has a native Codex app connection binding owned by the host.
  A portable service slug is never a Codex app ID, and an app-binding entry
  without a native binding is not a routable interface. Use `cli`,
  `mcp-resource`, or `mcp-tool` for the actual capability.

Connection requirements and host connection bindings remain separate from the
capability catalog. The host owns authentication, account state, and opaque
app IDs.

## Fallback is descriptive, not a runtime route

The v1 `fallback` field may remain in a compatibility artifact, but Elegy has
no active runtime consumer that selects or executes fallback entries. Fallback
must therefore be treated as non-authoritative guidance for migration and
documentation only. It does not establish a second kind, alter routing, or
promote readiness. Do not add new runtime behavior that branches on `fallback`.

## Evidence vocabulary

Keep interface declaration separate from evidence:

| Term | What it proves | What it does not prove |
| --- | --- | --- |
| `implemented` | Source behavior and its local tests exist. | A packaged install, external consumer, or live system works. |
| `conformance` | A governed schema, fixture, or contract consumer interprets the catalog correctly. | A real provider, host, or end-user task works. |
| `live-proof` | A clean installed, non-fixture task exercised the declared interface in its supported environment. | Production durability or broad cross-host support. |
| `routable` | Readiness is `usable` or `production` and the validated interface can be offered to an agent. | Permission to bypass host policy, approvals, or side-effect gates. |

Source tests, schema checks, fixtures, archive construction, and generated
projections can support `implemented` or `conformance`; they are not
`live-proof`. Only `usable` and `production` readiness stages are routable by
default.

## Validation rules

- `id`, `kind`, `description`, side-effect classification, and contract version
  are required for every entry.
- `cli` requires a CLI invocation template.
- `mcp-resource` requires a resource URI/template and its output contract.
- `mcp-tool` requires a tool name plus input and output contracts.
- No entry may declare more than one primary kind.
- Legacy `app-binding` requires a native Codex connection binding in the host
  projection; otherwise it is compatibility metadata only and cannot be
  routable.
- `fallback` is accepted only for v1 compatibility and is never used as an
  active runtime selection mechanism.

## Authority chain

```text
docs/specs/capability-catalog-v2.md
  -> shared/plugin-sdk Rust contract and generated schema
    -> plugin capability-catalog.json
      -> host projections (MCP, Codex, skills)
```

Host projections and skills are derived or instructional surfaces. They must
not redefine the capability kind, evidence stage, connection authority, or
routing decision.

## Validation commands

```bash
cargo test -p elegy-plugin-sdk
cargo test -p elegy-tooling
cargo run -p elegy-documentation -- check --project .
```
