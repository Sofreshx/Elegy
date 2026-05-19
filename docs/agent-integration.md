# Agent Integration

Elegy is designed for AI-agent hosts that can run local subprocesses. The
canonical integration path is skills plus CLI:

1. validate the setup
2. load the host manifest
3. discover the minimum needed capability
4. invoke the advertised CLI template
5. let the host enforce policy for side effects

MCP is supported as an optional projection for MCP-native clients, but it is not
the primary onboarding model.

## Canonical Host Flow

Validate the local setup:

```bash
elegy agent check --json
```

Load the integration manifest:

```bash
elegy agent manifest --json --profile ./tools/elegy-profile.json
```

Search with progressive disclosure:

```bash
elegy agent discover --query "repo status" --json --profile ./tools/elegy-profile.json
elegy agent discover --query "repo status" --detail --json --profile ./tools/elegy-profile.json
```

Use `data.results[].capabilities[].implementation.arguments` from detail output
as the CLI invocation template. Hosts should substitute the advertised
parameters, preserve JSON output, and apply policy before side-effecting
execution.

## Template Substitution

Host integrations should invoke subprocess capabilities without a shell when
possible: use `implementation.executableName` as the process name and
`implementation.arguments` as the argv template.

Rules:

- Substitute `${name}` placeholders only from parameters declared in
  `input.parameters` for that capability.
- Omit optional arguments when no value or declared default is present.
- Booleans are represented by flag presence when the template advertises a flag;
  omit the flag when the value is false.
- Arrays repeat the owning flag/value pattern only where the capability
  documents that behavior.
- Path parameters are passed as argv values without shell expansion.
- When `input.stdinFormat` is present, send the payload on stdin in that format.
- Always preserve `--json` for machine-mode parsing.
- Profile selection is not approval. Side-effecting calls still need host policy
  approval before execution.

## Capability Profiles

Profiles are host-owned allowlists. They let a host expose only the subset of
Elegy it wants.

```json
{
  "schemaVersion": "agent-capability-profile/v1",
  "profileId": "generic-agent-host",
  "includeSkills": ["repo", "data"],
  "includeCapabilities": ["memory-search"],
  "excludeCapabilities": [],
  "alwaysIncludeRouter": true
}
```

Rules:

- `includeSkills` enables all active capabilities under those skills.
- `includeCapabilities` enables individual capabilities.
- `excludeCapabilities` wins over includes.
- `alwaysIncludeRouter` keeps progressive discovery available by default.
- Profile selection means visible and invokable subject to policy; it does not
  approve side effects.

The governed schema lives at
`contracts/schemas/agent-capability-profile.schema.json`.

## Discovery Layers

Use `elegy agent ...` for host onboarding and profile-filtered discovery.

Use raw `elegy skills ...` when developing Elegy itself or inspecting the full
built-in registry:

```bash
elegy skills list --json
elegy skills search --query "diagram" --json
elegy skills describe --skill-id diagram --json
```

The skill definitions in `contracts/fixtures/skill-definition-v2.*.json` remain
the discovery authority. The contract schemas under `contracts/schemas/` remain
the durable authority.

## Portable Plugin Packages

`elegy-plugin-package/v1` is a portable package metadata contract for hosts that
want one governed package surface over multiple components. A package can bundle
or reference `skill-definition-v2` definitions, instruction skill files, MCP
projection metadata, docs, and assets.

The package contract is not a runtime. It must not contain host workspace ids,
approval state, secret refs, runtime sessions, adapter handles, or local trust
decisions. Hosts such as Holon import the portable package, then apply local
policy, readiness, approvals, secrets, evidence, and execution rules.

`SKILL.md` files, MCP descriptors, wrapper folders, and generated discovery
indexes remain derived or adapter surfaces. The governed package and skill
schemas under `contracts/schemas/` remain the authority roots.

## Optional MCP Adapter

MCP-native clients can start the stdio host:

```bash
elegy run --profile ./tools/elegy-profile.json
```

The active profile filters the MCP tool list and direct tool calls. The same
side-effect rule still applies: tools with side effects are blocked unless the
call is an explicit dry run or the host is started with side-effect execution
enabled by a surrounding approval policy.

```bash
elegy run --profile ./tools/elegy-profile.json --allow-side-effects
```

Use MCP only when it is the host's preferred protocol boundary. CLI templates
remain the default integration contract.

## Example Profiles

Generic local host (`docs/examples/agent-profile.generic.json`):

```json
{
  "schemaVersion": "agent-capability-profile/v1",
  "profileId": "generic-local-agent",
  "includeSkills": ["repo", "data", "web"],
  "includeCapabilities": ["memory-search"],
  "excludeCapabilities": [],
  "alwaysIncludeRouter": true
}
```

Holon-style desktop host example (`docs/examples/agent-profile.holon-example.json`):

```json
{
  "schemaVersion": "agent-capability-profile/v1",
  "profileId": "holon-desktop-example",
  "includeSkills": ["observe", "repo", "data"],
  "includeCapabilities": ["desktop-click", "desktop-type", "desktop-key"],
  "excludeCapabilities": ["notify-webhook"],
  "alwaysIncludeRouter": true
}
```

Holon is only an example consumer. Elegy should stay reusable for any host that
needs governed agent tool discovery and CLI execution.
