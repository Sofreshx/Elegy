# Elegy Ecosystem Topology

## Purpose

This document defines the current high-level organization of the Elegy ecosystem so docs, exports, and implementation ownership stay aligned with the repo that actually exists.

The main goal is to keep Elegy reusable across Holon and non-Holon projects while staying:

- contract-first
- provider-agnostic
- framework-agnostic where possible
- honest about the currently shipped executable surfaces

## Top-level decision

`Elegy` is now a capabilities-and-tooling monorepo.

Its active design centers are:

- governed schemas, fixtures, manifests, and policy co-located with their owning plugins under `plugins/<name>/schemas/`, `plugins/<name>/fixtures/`, and `plugins/<name>/contracts/`; cross-cutting fixtures live in `shared/core/fixtures/`
- the first-party Rust workspace at the repo root, organized into `hosts/`, `plugins/`, and `shared/`

Legacy `src/`, `tests/`, solution files, and `.NET` package-family narratives are not active repo centers and should not be described as such in current docs.

Historical `Elegy-Skills`, `Elegy-CLI`, and related sibling repos should be treated as archival or transition references rather than the current implementation home.

### Repo layout

```mermaid
flowchart TD
    subgraph governed["Governed artifacts — co-located per-plugin"]
        schemas["plugins/&lt;name&gt;/schemas/\ncontract truth"]
        fixtures_plugin["plugins/&lt;name&gt;/fixtures/\ndiscovery & examples"]
        crosscuts["shared/core/fixtures/\ncross-cutting fixtures"]
        config["plugins/&lt;name&gt;/contracts/\nconfiguration artifacts"]
    end

    subgraph exec["hosts/ + plugins/ + shared/ — Rust workspace (28 crates)"]
        hosts["hosts/\n2 binary crates\n(elegy-cli, elegy-host-mcp)"]
        plugins["plugins/\n17 capability crates\n(mcp, skills, planning, ...)"]
        shared["shared/\n10 library crates\n(core, runtime, tooling, ...)"]
    end

    subgraph docs["docs/ — governance & architecture"]
        governance["governance/\nworkflow & policy"]
        architecture["architecture/\ndesign decisions"]
        adr["adr/\ndecision records"]
        specs["specs/\nimplementation specs"]
    end

    subgraph derived["artifacts/ — CI-generated only (gitignored)"]
        bundles["contracts/\nexported handoff bundles"]
        dist["distribution/\nrelease archives"]
    end

    subgraph ci[".github/workflows/ — CI orchestrator"]
        orchestrator["publish-orchestrator.yml"]
    end

    governed -->|"consumed by"| exec
    exec -->|"generates"| derived
    docs -->|"governs"| governed
    docs -->|"governs"| exec
    ci -->|"builds & publishes"| derived
```

## Repo centers

### Governed artifact roots

The durable authority in this repo is language-agnostic and lives in authored assets such as:

- per-plugin schemas under `plugins/<name>/schemas/`
- per-plugin fixtures under `plugins/<name>/fixtures/`
- cross-cutting fixtures under `shared/core/fixtures/`
- configuration artifacts under `plugins/<name>/contracts/`
- version and release policy defined in schemas
- operational policy (workflow, environment, branch enforcement modes) under `docs/governance/`
- exported downstream handoff bundles under `artifacts/contracts` (CI-generated only; `artifacts/` is gitignored)

These assets are the source of truth for downstream consumers. They should be preferred over reviving a removed source-package tree.

### Rust executable family

The first-party executable and runtime layer is organized as a Cargo workspace at the repo root with three top-level subtrees:

- **`hosts/`** — umbrella CLI and host binaries
- **`plugins/`** — capability crates, each owning its schemas, fixtures, and tests
- **`shared/`** — library crates reused across hosts and plugins

The key current crates are:

- `elegy-core` (`shared/core`) for reusable core primitives and cross-cutting fixtures
- `elegy-runtime` (`shared/runtime`) for runtime composition
- `elegy-descriptor` (`shared/descriptor`) for governed-descriptor types
- `elegy-policy` (`shared/policy`) for bounded policy enforcement
- `elegy-tooling` (`shared/tooling`) for descriptor authoring, analysis, and skill generation
- `elegy-mcp` (`plugins/mcp`) for MCP analysis and related runtime behavior over governed descriptors
- `elegy-skills` (`plugins/skills`) for governed skill-registry access and validation
- `elegy-planning` (`plugins/planning`) for durable planning authority
- `elegy-memory` (`plugins/memory`) for bounded local memory surfaces
- `elegy-documentation` (`plugins/documentation`) for documentation inspection, mapping, and export
- `elegy-configuration` (`plugins/configuration`) for governed template and profile flows
- `elegy-cli` (`hosts/cli`) for the umbrella human-facing operator surface
- `elegy-host-mcp` (`hosts/host-mcp`) for the thin stdio host

### Current shipped operator slice

The current shipped operator path is intentionally narrow.

The current shipped operator surfaces are `elegy`, `elegy-memory`, `elegy-mcp`, `elegy-planning`, `elegy-skills`, `elegy-configuration`, and `elegy-documentation`.

What the repo proves today:

- the Rust `elegy` CLI exposes `author mcp`, `analyze mcp`, umbrella `skills ...`, and lower-level `generate skills` / `plugin export codex`
- the `plugins/memory` surface is shipped as a bounded local operator surface
- the `plugins/mcp` surface is shipped as a thin dedicated wrapper over descriptor authoring and descriptor analysis
- the `plugins/planning` surface is shipped as a dedicated wrapper over durable planning authority
- the `plugins/skills` surface is shipped as a thin dedicated wrapper over governed skill-registry access and validation
- the `plugins/configuration` surface is shipped as a dedicated wrapper over governed template/profile flows
- the `plugins/documentation` surface is shipped as a dedicated wrapper over documentation inspection, mapping, and non-authoritative export
- those commands are backed by shared Rust crates led by `plugins/mcp`, `plugins/skills`, `plugins/planning`, and `shared/tooling`
- the CLI also carries validation, inspection, and stdio-host startup entrypoints
- contract bundles can be exported and consumed independently of the Rust workspace

What the repo does **not** yet prove as a completed product surface:

- built-in MCP-native self-authoring loops
- skill-driven autonomous authoring built into the runtime as a finished user surface
- broad claims that REST/OpenAPI ingestion, operation-catalog projection, or hosted MCP execution are implemented just because descriptor analysis and generation exist

Those remain targets until the repo has a documented, validated, contributor-facing implementation for them.

## Burden-of-proof rule

Capability existence is not the same thing as long-term repo-center status.

Under the current architecture reset, a shared Elegy surface should survive only if it proves one of two things:

1. it is governed authority that downstream consumers must consume consistently
2. it is a reusable Rust executable capability that multiple consumers should use without product-specific assumptions

If a capability can be represented as schemas, fixtures, manifests, compatibility metadata, policy files, or docs, prefer those artifacts over shared code.

If a capability depends on host-specific auth, persistence, UI, HTTP endpoints, DI composition, tenant policy, or app orchestration, it belongs in the consuming repo rather than in Elegy.

### Contract authority chain

How governed artifacts flow from authored truth to host consumption:

```mermaid
flowchart LR
    subgraph authored["Authored authority"]
        schemas["plugins/&lt;name&gt;/schemas/\nschema truth"]
        fixtures_auth["plugins/&lt;name&gt;/fixtures/\ngoverned examples"]
    end

    subgraph package["Plugin"]
        pkg_json[".elegy-plugin/plugin.json"]
        skills["Agent Skills\n(SKILL.md)"]
    end

    subgraph installed["Installed surface"]
        receipt["install-receipt.json\nverification evidence"]
        bin_dir["bin/\ninstalled binaries"]
    end

    subgraph host["Host consumption"]
        registry["Host tool registry\nexecution policy"]
        llm["LLM tool calling\nagent-facing schema"]
    end

    schemas --> fixtures_auth
    schemas --> pkg_json
    pkg_json --> skills
    skills --> registry
    bin_dir --> registry
    registry --> llm
```

## Dependency shape across the repo

The dependency direction should remain one-way:

1. governed artifacts and operational policy at the bottom
2. shared Rust contract consumers and policy crates above those authored assets
3. plugin capability crates above the shared layer
4. operator surfaces such as `hosts/cli` and `hosts/host-mcp` on top
5. downstream apps consuming exported bundles, explicit Rust crates, or CLI outputs

That means:
- per-plugin schemas, fixtures, and operational policy define the durable boundary
- Rust crates consume governed artifacts rather than redefining them
- operator shells remain thin over explicit runtime and tooling crates
- docs must not imply a removed source-package center just because downstream consumers may still be `.NET`

### Authority hierarchy

The five-layer dependency stack, bottom-up:

```mermaid
flowchart TB
    subgraph L0["L0 — Governed artifacts"]
        schemas["plugins/*/schemas/\nschema truth"]
        fixtures_l0["plugins/*/fixtures/\ndiscovery authority"]
        crosscuts_l0["shared/core/fixtures/\ncross-cutting fixtures"]
        policy["docs/governance/\nworkflow policy"]
    end

    subgraph L1["L1 — Shared library crates"]
        contracts["elegy-descriptor\nelegy-policy"]
        runtime["elegy-runtime\nelegy-core"]
        tooling["elegy-tooling"]
    end

    subgraph L2["L2 — Plugin capability crates"]
        memory["elegy-memory\nelegy-planning"]
        skills["elegy-skills\nelegy-configuration"]
        docs_feat["elegy-documentation\nelegy-mcp"]
        other["elegy-observe\nelegy-mermaid\nelegy-diagram\n..."]
    end

    subgraph L3["L3 — Operator surfaces"]
        cli["elegy-cli\numbrella CLI"]
        host["elegy-host-mcp\nstdio host"]
        dedicated["elegy-memory\nelegy-planning\nelegy-skills\n..."]
    end

    subgraph L4["L4 — CI & consumers"]
        ci["publish-orchestrator\nreusable-publish"]
        consumers["host consumers\ndownstream repos"]
    end

    L0 --> L1
    L1 --> L2
    L2 --> L3
    L3 --> L4
```

## Consumer guidance

For near-term integration and migration work, prefer the smallest real Elegy surface that carries the responsibility:

- use per-plugin schemas and fixtures, shared cross-cutting fixtures, and exported bundles for stable schema, fixture, and compatibility handoff
- use Rust crates under `hosts/`, `plugins/`, and `shared/` only for proven reusable executable behavior
- keep host-specific endpoints, orchestration, and prompt assembly in the consuming repo
- do not assume sibling checkout layouts, solution-level builds, or removed package-family roots

## Split policy for future repos

If a surface later proves it needs its own release cadence, contributor base, or implementation language, it can split into a dedicated repo.

That split should happen only after:

- the boundary is already stable
- at least two real consumers exist
- the split improves ownership more than it increases coordination cost

## Current practical stance

For now, the most coherent working model is:

- `Elegy` is the single active repo
- governed authority lives in co-located plugin artifacts (`plugins/<name>/schemas/`, `plugins/<name>/fixtures/`, `plugins/<name>/contracts/`) and cross-cutting `shared/core/fixtures/`, not in a removed `.NET` source tree or a centralized `contracts/` directory
- `hosts/`, `plugins/`, and `shared/` form the first-party home for reusable executable surfaces, especially CLI, MCP analysis, descriptor tooling, policy-bounded runtime composition, and host layers
- the current contributor-facing self-authoring story is the Rust CLI author/analyze/generate path over governed descriptors, exposed through both the umbrella `elegy` surface and the dedicated `plugins/mcp` / `plugins/skills` binaries
- built-in MCP or skill-driven self-authoring remains a target and should not be documented as a completed surface until the repo proves it end to end
