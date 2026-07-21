---
title: Agent Integration
status: active
owner: elegy-core
doc_kind: guide
---

# Agent Integration

Elegy is designed for AI-agent hosts that can run local subprocesses. The
canonical path is installed-plugin discovery plus dedicated `elegy-*` binaries:

1. validate the setup
2. discover the minimum needed capability
3. invoke the advertised CLI template
4. let the host enforce side-effect policy

MCP is supported as an optional projection for MCP-native clients, but it is not the primary onboarding model.

## Canonical Flow

Verify the plugin package and then invoke its dedicated binary:

```bash
elegy-plugin-packaging verify --plugin plugins/planning
elegy-planning --help
```

## Discovery Layers

Skill definitions in `plugins/<name>/skills/<skill-id>/SKILL.md` and standalone root
`<skill-id>/SKILL.md` packages are the discovery authority. Each plugin manifest
declares the skill directory packaged with that plugin. A host discovers and
routes only the plugins installed or projected into that host; Elegy does not
provide a central registry or cross-plugin resolver. Contract schemas live under
`plugins/<name>/schemas/` and cross-cutting fixtures under `shared/core/fixtures/`.

## Optional MCP Adapter

MCP-native clients can start the stdio host:

```bash
elegy-run
```

The same side-effect rule applies: tools with side effects are blocked unless the call is an explicit dry run or the host is started with side-effect execution enabled by a surrounding approval policy.

```bash
elegy-run --allow-side-effects
```

Use MCP only when it is the host's preferred protocol boundary. CLI invocation remains the default integration contract.

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
