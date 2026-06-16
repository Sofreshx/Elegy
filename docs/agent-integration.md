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

The skill definitions in `contracts/fixtures/skill.*.json` remain
the discovery authority. The contract schemas under `contracts/schemas/` remain
the durable authority.

## Portable Plugin Packages

`elegy-plugin-package/v1` is the portable package metadata contract for hosts
that want one governed package surface over multiple components. A package can
bundle or reference `skill` definitions, instruction skill files, MCP projection
metadata, docs, and local configuration template/profile components for
deterministic `elegy-configuration` loading.

The package contract is not a runtime. It must not contain host workspace ids,
approval state, secret refs, runtime sessions, adapter handles, or local trust
decisions. Hosts such as Holon import the portable package, then apply local
policy, readiness, approvals, secrets, evidence, and execution rules.

`SKILL.md` files, including the repo-local `.agents/skills/**` and
`.github/skills/**` mirrors, MCP descriptors, wrapper folders, and generated
discovery indexes remain derived or adapter surfaces. The governed package and
skill schemas under `contracts/schemas/` remain the authority roots.

Portable packages may also be projected into host-specific plugin folders
through `elegy generate codex-plugin` (Codex) or future host projection
targets, but those generated outputs remain derived adapter surfaces rather
than authority.

Local `elegy-plugin-package/v1` files may also be consumed directly by
`elegy-configuration` or the umbrella `elegy configuration` commands for
package-backed deterministic configuration apply/verify flows. The package file
remains metadata plus packaged assets, not a runtime.

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

## Host Projection Metadata

governed skill definitions may include a `hostProjection` block that gives
runtime hosts explicit metadata for mapping capabilities to their own tool
surfaces:

```json
{
  "hostProjection": {
    "cliName": "elegy-planning",
    "outputContractId": "elegy-planning-v1",
    "defaultSideEffectClass": "disk_write",
    "capabilityProjections": [
      {
        "capabilityId": "planning-goal-create",
        "functionName": "planning_goal_create",
        "sideEffectClass": "disk_write",
        "isDeterministic": false
      }
    ]
  }
}
```

Fields:

- `cliName`: the dedicated CLI binary name for subprocess invocation.
- `outputContractId`: versioned output contract family identifier for host
  validation (e.g. `elegy-planning-v1`, `elegy-skills-v1`).
- `defaultSideEffectClass`: the skill-level side-effect class. Individual
  capabilities may override with `none`, `read_only`, `disk_read`,
  `disk_write`, `network_outbound`, `process_spawn`, or `desktop_ui`.
- `capabilityProjections[]`: per-capability function-calling metadata with
  stable `functionName`, optional `sideEffectClass` override, and
  `isDeterministic` flag.

Hosts use this metadata to register runtime tools, apply side-effect policy,
and validate output contracts without parsing CLI templates.

`hostProjection` is part of the typed `elegy-contracts` model. The
`SkillDefinitionV2` Rust type exposes it as `host_projection: Option<SkillHostProjection>`
with typed child structs `SkillHostCapabilityProjection` and the
`HostSideEffectClass` enum, so Rust consumers can read the metadata directly
without re-parsing JSON. `validate_skill_definition_v2` enforces non-empty
`cliName` and `outputContractId`, validates that every
`capabilityProjections[].capabilityId` references an existing capability on
the same skill, and rejects duplicate `functionName` and `capabilityId`
collisions.

## Runtime Tools (Host Integration)

Elegy tools become host runtime tools backed by receipt-installed `elegy-*`
binaries. Holon is one consumer example. The integration pattern:

1. **Discover**: the host reads `hostProjection` from the governed skill
   definition to learn the CLI name, output contract, and side-effect classes.
2. **Register**: the host registers each `capabilityProjection` as a callable
   runtime tool with the advertised `functionName` and input/output schemas.
   Host-oriented package fixtures additionally carry the same projections in
   `components.capabilityProjections` so package-level consumers do not have to
   dereference `skillDefinitions[].definitionRef` first.
3. **Invoke**: the host constructs the CLI invocation from
   `implementation.arguments` (shell-free argv), substitutes parameters, and
   runs the subprocess.
4. **Validate**: the host checks the JSON envelope status and validates against
   the `outputContractId` schema.

Provider function calling is only an allowlisted projection of these runtime
tools. The runtime tool (backed by the installed binary) is the source of
execution authority.

## Release Assets and Install Receipts

Tagged releases include dedicated CLI archives and wrapper archives for each
runtime surface:

- `elegy-planning` binary and `elegy-planning-wrapper` archive
- `elegy-skills` binary and `elegy-skills-wrapper` archive
- `elegy-memory` binary and `elegy-memory-wrapper` archive
- `elegy-mcp` binary and `elegy-mcp-wrapper` archive
- `elegy-configuration` binary and `elegy-configuration-wrapper` archive
- `elegy-documentation` binary and `elegy-documentation-wrapper` archive

Install receipts include executable paths for both dedicated binaries and
umbrella wrappers. Holon and other hosts can consume these receipts to locate
the installed binaries for subprocess invocation.

The governed plugin package fixtures
(`elegy-plugin-package.elegy-planning.json` and
`elegy-plugin-package.elegy-skills.json`) carry self-sufficient
`capabilityProjections` for direct host consumption, alongside the
`hostProjection` block on the underlying skill definition. To ship in a
released contract bundle, every package fixture must be listed under its
schema entry in `contracts/manifests/compatibility-manifest.json` and
mirrored in `governance/canonical-output-inventory.json`; otherwise the
`export_contract_bundle` exporter omits it from the directory output and zip
archive.

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
