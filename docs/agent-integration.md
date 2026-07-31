---
title: Agent Integration
status: active
owner: elegy-core
doc_kind: guide
---

# Agent Integration

Elegy is designed for AI-agent hosts that can run local subprocesses or speak
MCP. The canonical path is the typed catalog of an installed adapter plugin;
each capability declares one first-class interface: `cli`, `mcp-resource`, or
`mcp-tool`.

The CLI lane remains the default integration boundary when a local subprocess
is sufficient. MCP resources and MCP tools are direct interfaces for
MCP-native hosts, not fallback metadata:

1. validate the setup
2. discover the minimum needed capability
3. invoke the advertised CLI template
4. let the host enforce side-effect policy

MCP is a first-class protocol interface for entries declared as
`mcp-resource` or `mcp-tool`. It is optional only when a host does not speak
MCP; the CLI remains the default onboarding lane for `cli` entries.

## Canonical Flow

Verify the plugin package and then invoke its dedicated binary:

```bash
elegy-plugin-packaging verify --plugin plugins/accounts
elegy-accounts --help
```

## Discovery Layers

The adapter's `capabilityCatalog` is executable discovery authority. Bundled
skills are optional workflow guidance, never proof that behavior exists.
Standalone skills use the target host's normal skill installation lane and do
not enter the Elegy plugin marketplace. A host discovers and routes only
installed, readiness-qualified adapters; Elegy does not provide a cross-plugin
runtime resolver.

## MCP interfaces

MCP-native clients can start the stdio host:

```bash
elegy-run
```

The host serves declared MCP resources and tools. The same side-effect rule
applies: side-effecting tools are blocked unless the call is an explicit dry
run or the host is started with side-effect execution enabled by a surrounding
approval policy.

```bash
elegy-run --allow-side-effects
```

Use MCP when the capability declares an MCP resource or tool and the host
supports that protocol. CLI invocation remains the default integration contract
for capabilities declared as `cli`. The legacy v1 `fallback` field is not
runtime routing, and `app-binding` is only meaningful with a native Codex app
connection binding.

## Release Assets

Tagged releases include dedicated binaries for each runtime surface.

- `elegy-planning` binary
- `elegy-memory` binary
- `elegy-mcp` binary
- `elegy-configuration` binary
- `elegy-documentation` binary

Plugin-packaged binary surfaces (`elegy-planning`, `elegy-memory`,
`elegy-mcp`, `elegy-documentation`, `elegy-observe`, `elegy-desktop`) ship as
`<surface>-plugin-<target>.zip` archives containing manifest, skills, and
binary. Skill-only plugin packages ship as `<surface>-plugin-any.zip` archives.
Non-plugin surfaces ship as standalone binaries.

See [Distribution](distribution.md) for the release index and install lanes.

## Example Profile

Generic local host (`docs/examples/agent-profile.generic.json`):

```json
{
  "schemaVersion": "agent-capability-profile/v1",
  "profileId": "generic-local-agent",
  "includeSkills": ["repo", "data", "web"],
  "excludeCapabilities": [],
  "alwaysIncludeRouter": true
}
```
