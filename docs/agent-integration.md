# Agent Integration

Elegy is designed for AI-agent hosts that can run local subprocesses. The canonical integration path is skills plus CLI:

1. validate the setup
2. load the host manifest
3. discover the minimum needed capability
4. invoke the advertised CLI template
5. let the host enforce policy for side effects

MCP is supported as an optional projection for MCP-native clients, but it is not the primary onboarding model.

## Canonical Host Flow

Validate the local setup:

```bash
elegy agent check --json
```

Load the integration manifest:

```bash
elegy agent manifest --json
```

Search with progressive disclosure:

```bash
elegy agent discover --query "repo status" --json
elegy agent discover --query "repo status" --detail --json
```

Invoke the discovered command:

```bash
elegy <command> --json
```

## Discovery Layers

Use `elegy agent ...` for host onboarding and discovery.

Use raw `elegy skills ...` when developing Elegy itself or inspecting the full built-in registry:

```bash
elegy skills list --json
elegy skills search --query "diagram" --json
elegy skills describe --skill-id diagram --json
```

Skill definitions in `plugins/<name>/skills/<skill-id>/SKILL.md` and standalone root
`<skill-id>/SKILL.md` packages are the discovery authority. Contract schemas live
under `plugins/<name>/schemas/` and cross-cutting fixtures under `shared/core/fixtures/`.

## Optional MCP Adapter

MCP-native clients can start the stdio host:

```bash
elegy run
```

The same side-effect rule applies: tools with side effects are blocked unless the call is an explicit dry run or the host is started with side-effect execution enabled by a surrounding approval policy.

```bash
elegy run --allow-side-effects
```

Use MCP only when it is the host's preferred protocol boundary. CLI invocation remains the default integration contract.

## Release Assets

Tagged releases include dedicated CLI archives for each runtime surface:

- `elegy-planning` binary
- `elegy-skills` binary
- `elegy-memory` binary
- `elegy-mcp` binary
- `elegy-configuration` binary
- `elegy-documentation` binary

Plugin-packaged surfaces (`elegy-planning`, `elegy-skills`, `elegy-memory`, `elegy-mcp`,
`elegy-documentation`, `elegy-observe`, `elegy-desktop`) ship as `.plugin.zip` archives
containing manifest, skills, and binary. Non-plugin surfaces ship as standalone binaries.

See [Distribution](distribution.md) for the full list of asset families, targets, and install commands.

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
