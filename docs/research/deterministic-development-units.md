---

created: 2026-06-16
updated: 2026-06-16
category: research
status: research-baseline
doc_kind: reference
id: deterministic-development-units
summary: Reference document for building Elegy/Holon systems around governed solved units: repeatable development capabilities encoded as deterministic generators, tools, contracts, workflows, and validation gates for AI agents.
tags:
- agents
- scaffolding
- generators
- codegen
- ui
- workflows
- rust
- tools
- validation
- elegy
- holon
related:
- dynamic-cli-tooling
- reusable-ai-substrate-patterns
- openclaw-orchestration-gap-roadmap
- agent-integration
- spec-baseline

---

# Deterministic Development Units for Agentic Engineering

## Purpose

This document defines a direction for using deterministic generation, code transformation, contract generation, and workflow composition as first-class primitives for AI-assisted development.

The goal is to stop AI agents from repeatedly re-inventing solved engineering patterns. When a unit of work becomes repeatable, it should be promoted into a governed capability that agents can invoke reliably.

The document is a research baseline, not an implementation-ready spec.

---

# 1. Core Thesis

Move from:

```txt
User intent
  -> LLM explores codebase
  -> LLM guesses patterns
  -> LLM edits files manually
  -> LLM maybe runs tests
  -> inconsistent artifact
```

To:

```txt
User intent
  -> LLM classifies the work unit
  -> deterministic solved unit executes known pattern
  -> graph/provenance is updated
  -> validation gates run
  -> LLM only handles the unsolved part
```

Generators are not merely scaffolding. In this context, they are executable architecture.

A generator, codemod, contract emitter, workflow emitter, or tool wrapper can encode engineering practice as executable code. Once the same class of work has been solved repeatedly, the AI should know when to invoke the capability, not how to reproduce every file edit by reasoning from scratch.

---

# 2. Terminology

## 2.1 Solved Unit

A **Solved Unit** is a repeatable unit of development work that has been encoded into a governed, validated, agent-invokable capability.

It may be implemented by:

* a generator
* a codemod
* a template
* a CLI wrapper
* a codegen pipeline
* a contract emitter
* a workflow emitter
* a Rust `xtask`
* an Nx generator
* an OpenAPI/TypeSpec/Protobuf generator
* a Storybook/test generator
* a Holon workflow node

The term should not mean “the problem is globally solved forever.” It means:

> For this workspace, this class of work is sufficiently understood to be invoked through a governed deterministic capability instead of ad hoc agent edits.

## 2.2 Solved Unit Contract

A **Solved Unit Contract** is the manifest that defines:

* intent
* inputs
* outputs
* preconditions
* owned files/regions
* extension points
* graph mutations
* validation gates
* drift policy
* rollback policy
* agent-facing usage rules

The contract is more important than the generator backend.

## 2.3 Generator Backend

A **Generator Backend** is the concrete engine used to perform the work.

Examples:

* Nx generator
* Angular Schematics-style virtual file transform
* Plop/Hygen template
* cargo-generate template
* Rust `xtask`
* OpenAPI Generator
* TypeSpec emitter
* Style Dictionary build
* Dagger pipeline
* custom Elegy generator

Elegy should not standardize on one backend. Elegy should standardize on contracts, provenance, graph semantics, and validation.

## 2.4 Promotion

**Promotion** is the process of turning repeated work into a solved unit.

```txt
manual agent work
  -> documented skill
  -> candidate generator/tool
  -> verified solved unit
  -> reusable workflow node
```

Promotion is the core lifecycle. It is how repeated AI/developer work becomes deterministic infrastructure.

---

# 3. Naming Decision: Should We Keep “Solved Unit”?

## Recommendation

Keep **Solved Unit** as the high-level conceptual name, but avoid using it alone in implementation APIs where precision matters.

Use:

```txt
Solved Unit                  conceptual category
Solved Unit Contract         manifest
Solved Unit Registry         catalog
Solved Unit Runtime          execution layer
Generator Backend            implementation mechanism
Promotion Pipeline           lifecycle
```

## Why the name is good

“Solved Unit” communicates the right strategic idea:

* reduce repeated reasoning
* encode known patterns
* give agents reliable capabilities
* promote repeated work into reusable tools
* avoid reinventing architecture

It is broader than “generator,” “template,” or “tool.”

## Risks with the name

The word “solved” can sound too absolute. Some work units are only solved:

* for a specific repo
* for a specific framework
* for a specific team convention
* under a specific version
* with declared escape hatches

So the document should clarify:

> A solved unit is contextual and versioned. It is not a universal proof that no design work remains.

## Alternatives Considered

| Name                           | Strength                                  | Weakness                             |
| ------------------------------ | ----------------------------------------- | ------------------------------------ |
| Solved Unit                    | Strong strategic meaning; broad           | Can sound too absolute               |
| Capability Unit                | Neutral and broad                         | Less memorable                       |
| Pattern Unit                   | Good for promotion from repeated patterns | Too abstract                         |
| Governed Generator             | Precise for scaffolding                   | Too narrow; excludes workflows/tools |
| Reusable Development Primitive | Accurate                                  | Too long                             |
| Crystallized Skill             | Very good conceptually                    | Sounds less formal                   |
| Executable Pattern             | Strong                                    | Slightly narrow                      |
| Agent Capability               | Useful but generic                        | Could mean any tool                  |
| Work Unit                      | Too generic                               | Not enough governance implied        |

Final choice:

```txt
Use "Solved Unit" for concept and docs.
Use "Solved Unit Contract" for manifests.
Use domain-specific commands for CLI UX.
```

Example CLI should avoid overly abstract phrasing:

```bash
elegy-ui add-page SettingsPage
elegy-tool wrap-cli gh
elegy-plugin new rust-codegraph
elegy-workflow create-dag validation
```

Not:

```bash
elegy solved-unit execute create-ui-page
```

The abstraction should exist in the architecture, not necessarily in every user-facing command.

---

# 4. The Layered Maturity Model

| Layer | Name            | Description                                                                            |
| ----: | --------------- | -------------------------------------------------------------------------------------- |
|     0 | Raw capability  | grep, edit, shell, browser, file IO                                                    |
|     1 | Skill           | Written procedure: “how to add a route”, “how to wrap a CLI”                           |
|     2 | Generator       | Deterministic scaffold/codemod that creates or modifies files                          |
|     3 | Governed tool   | Schema-validated command with declared inputs/outputs, provenance, tests               |
|     4 | Workflow        | Composed graph/DAG/state machine of tools with gates, retries, fallback                |
|     5 | Promoted system | Versioned solved unit used across projects/workspaces with evidence and drift handling |

Maturity direction:

```txt
manual agent work
  -> skill
  -> generator
  -> governed tool
  -> reusable workflow node
  -> promoted solved unit
```

## Practical Rule

If an AI or developer has solved the same class of problem 5-10 times and the steps are mostly identical, consider promoting it into a solved unit.

Do not promote one-off work too early.

Bad:

```txt
create-special-customer-report-v2-for-europe
```

Good:

```txt
create-report
  type=customer
  region=europe
  format=pdf
```

---

# 5. Core Design Principle

The system should not say:

```txt
AI can generate everything.
```

It should say:

```txt
AI should generate less.
```

The best agentic development system is one where agents make fewer arbitrary edits because solved work has been compressed into reliable capabilities.

The agent should focus on:

* ambiguity
* product decisions
* business-specific logic
* edge cases
* repair
* integration judgment
* when to promote a pattern

The deterministic system should handle:

* folder structure
* imports
* exports
* routing registration
* config updates
* schema generation
* test scaffolding
* Storybook stories
* token compilation
* workflow wiring
* tool schemas
* installation checks
* validation gates

---

# 6. Core Lifecycle

```txt
1. Agent or developer solves a task manually.
2. Elegy records evidence: files changed, commands run, tests, graph deltas, errors.
3. Pattern repeats across tasks or projects.
4. Human reviews whether the pattern is stable enough.
5. Candidate solved unit is created.
6. Candidate is replayed against known examples.
7. Validation gates prove the generated output is correct enough.
8. Solved unit is promoted to stable.
9. Future agents invoke it deterministically.
10. Drift and failures feed back into repair or redesign.
```

The most important future command may be:

```bash
elegy promote-pattern
```

This bridges:

```txt
agent solved it once
```

to:

```txt
future agents never need to solve that part manually again
```

---

# 7. Solved Unit Contract

Every solved unit should have a manifest.

Example:

```yaml
id: elegy.solved-unit.ui.create-page.v0
kind: solved_unit
domain: ui
status: experimental

intent:
  name: create-ui-page
  description: Create a governed route/page pair with layout slots, Storybook story, tests, and graph registration.

inputs:
  route:
    type: string
    required: true
  page_name:
    type: string
    required: true
  layout:
    enum: [standard, dashboard, centered, split]
    default: standard
  data_source:
    type: optional_ref

preconditions:
  - react_ts_project_detected
  - elegy_ui_graph_exists
  - route_does_not_exist

emits:
  files:
    - src/routes/{{route}}.tsx
    - src/pages/{{page_name}}/{{page_name}}.tsx
    - src/pages/{{page_name}}/{{page_name}}.stories.tsx
    - src/pages/{{page_name}}/{{page_name}}.test.tsx
  graph_nodes:
    - route
    - page
    - story
    - test
  graph_edges:
    - route:renders:page
    - page:validated_by:test
    - page:documented_by:story

owned_regions:
  - generated imports
  - route registration
  - story boilerplate
  - test boilerplate

extension_points:
  - page body slot
  - data binding slot
  - action handlers slot

validation:
  - schema_validation
  - file_ownership_validation
  - graph_validation
  - typecheck
  - unit_tests
  - storybook_interaction_tests
  - accessibility_tests
  - elegy_codegraph_compare

drift_policy:
  mode: detect_and_report
  never_overwrite:
    - manual regions

rollback_policy:
  mode: transactional
  restore_files_on_failure: true

agent_policy:
  invocation:
    when:
      - user asks for a new page
      - route/page pattern already exists
      - no custom architecture is required
    avoid_when:
      - page requires new layout grammar
      - existing routing architecture is unclear
      - generated files would overwrite undeclared manual regions
```

## Required Manifest Fields

| Field              | Purpose                                         |
| ------------------ | ----------------------------------------------- |
| `id`               | Stable identifier with version                  |
| `kind`             | Entity type                                     |
| `domain`           | UI, tools, Rust, API, workflow, docs, etc.      |
| `status`           | draft, experimental, stable, deprecated         |
| `intent`           | What the unit does                              |
| `inputs`           | Typed parameters                                |
| `preconditions`    | What must be true before execution              |
| `emits`            | Files, graph nodes, config changes, docs, tests |
| `owned_regions`    | What the solved unit controls                   |
| `extension_points` | Where manual work is allowed                    |
| `validation`       | Post-generation gates                           |
| `drift_policy`     | What happens when emitted code diverges         |
| `rollback_policy`  | How failed generation is reverted               |
| `agent_policy`     | When agents should or should not invoke it      |

---

# 8. Generator Backend Strategy

Elegy should use a multi-backend generator architecture.

```txt
Elegy Generator Core
  - manifest parser
  - input schema validation
  - graph mutation
  - provenance
  - ownership tracking
  - drift detection
  - validation gates
  - rollback
  - promotion lifecycle

Backends
  - Nx generator backend
  - TypeScript AST/codemod backend
  - Angular Schematics-style virtual tree backend
  - Plop/Hygen-style template backend
  - Rust xtask backend
  - cargo-generate backend
  - OpenAPI Generator backend
  - TypeSpec emitter backend
  - Protobuf backend
  - GraphQL Code Generator backend
  - Style Dictionary backend
  - Dagger pipeline backend
  - Temporal workflow backend
  - MCP export backend
```

Elegy owns:

* solved unit contract
* graph model
* provenance
* validation semantics
* drift policy
* promotion lifecycle
* agent invocation rules

External tools provide execution backends.

---

# 9. Tool Selection Policy

Elegy does not standardize on a single generator engine. Elegy standardizes on the solved unit contract.

Use this decision rule:

```txt
Need monorepo/project graph awareness?
  -> Nx

Need safe transforms/migrations of existing code?
  -> Angular Schematics-style virtual filesystem or codemod backend

Need platform-level service creation with catalog/permissions?
  -> Backstage-style scaffolder

Need simple local files from templates?
  -> Plop / Hygen / Yeoman

Need Rust project/crate bootstrap?
  -> cargo-generate

Need Rust-native workspace automation?
  -> xtask

Need REST client/server/docs from contract?
  -> OpenAPI Generator

Need to author new APIs cleanly before OpenAPI?
  -> TypeSpec

Need stable binary/cross-language RPC contracts?
  -> Protocol Buffers

Need GraphQL frontend/backend typing?
  -> GraphQL Code Generator

Need design tokens compiled to code?
  -> W3C Design Tokens + Style Dictionary

Need generated UI validation?
  -> Storybook

Need generated UI behavior as state machines?
  -> Zag.js / Ark UI

Need complex app/session workflows?
  -> XState

Need generated CI/build/test workflows?
  -> Dagger

Need durable long-running workflows?
  -> Temporal

Need external agent tool exposure?
  -> MCP adapter
```

---

# 10. Area A — UI Creation

## Approach

Do not generate raw UI code as the primary abstraction.

Generate and verify a UI graph.

```txt
UI intent
  -> route/layout/page/component/data/state graph
  -> governed templates
  -> emitted React/TypeScript files
  -> Storybook/tests/accessibility/typecheck
  -> codegraph comparison
```

V0 should target React + TypeScript because:

* `elegy-codegraph` already has TypeScript extraction
* the strongest generator/testing/design-system ecosystem exists there
* Storybook, TanStack, Radix, React Aria, Zag, and Style Dictionary are mature enough for a first proof

## Solved Units

| Unit                     | Description                                       |
| ------------------------ | ------------------------------------------------- |
| `init-ui-app`            | Scaffold a governed UI app                        |
| `add-route`              | Add route node and route file                     |
| `add-layout`             | Add layout with named regions/slots               |
| `add-page`               | Create route/page pair with stories/tests         |
| `add-region`             | Define a named layout region                      |
| `add-slot`               | Define composable extension point                 |
| `add-component`          | Create component with tests/stories               |
| `add-form`               | Create typed form with validation                 |
| `add-table`              | Create data table with columns/sorting/filtering  |
| `add-empty-state`        | Add empty-state behavior                          |
| `add-loading-state`      | Add loading behavior                              |
| `add-error-state`        | Add error boundary/state                          |
| `add-data-query`         | Add TanStack Query-backed data read               |
| `add-mutation`           | Add mutation with invalidation rules              |
| `add-story`              | Add Storybook story                               |
| `add-interaction-test`   | Add Storybook interaction test                    |
| `add-accessibility-test` | Add accessibility test                            |
| `add-visual-test`        | Add visual regression baseline/check              |
| `add-theme-token`        | Add/modify token and compile outputs              |
| `verify-ui-graph`        | Compare intended UI graph to extracted code graph |
| `detect-ui-drift`        | Detect generated/manual divergence                |

## UI Contract

```txt
app
route tree
layout grammar
regions
slots
pages
components
actions
state sources
data sources
forms
queries
mutations
theme tokens
semantic tokens
variants
breakpoints
density
motion
dark/light modes
permissions
loading/empty/error behavior
stories
tests
accessibility requirements
visual baselines
```

## Recommended Tools

| Tool                     | Role                                           | Use Case                                          |
| ------------------------ | ---------------------------------------------- | ------------------------------------------------- |
| Nx generators            | Monorepo-aware UI/package generation           | Apps, libs, feature packages, component libraries |
| Nx sync generators       | Keep generated config in sync with graph state | Route/config/project sync before tasks            |
| Angular Schematics model | Safe virtual-file transforms                   | Migrations, config edits, codemods                |
| Storybook                | Component/page workshop and test surface       | Stories, interaction tests, a11y, visual checks   |
| W3C Design Tokens        | Token exchange format                          | Token contract                                    |
| Style Dictionary         | Token compiler                                 | CSS variables, TS exports, platform outputs       |
| Radix UI                 | Accessible unstyled primitives                 | Dialogs, dropdowns, popovers, menus               |
| React Aria               | Accessibility-focused primitives/hooks         | Complex accessible behavior                       |
| Zag.js                   | Finite-state UI behavior                       | Machine-modeled components                        |
| Ark UI                   | Headless components built around Zag concepts  | Cross-framework-ish primitive reference           |
| TanStack Router          | Typed routing/search params                    | Route state and type-safe navigation              |
| TanStack Query           | Server state                                   | Queries, cache, mutations                         |
| TanStack Form            | Typed forms                                    | Form state and validation                         |
| XState                   | Explicit statecharts                           | Complex UI/session workflows                      |
| Playwright               | Browser-level end-to-end checks                | Full UI behavior verification                     |

## V0 Recommendation

Build `elegy-ui` around:

```txt
React + TypeScript
Nx optional backend
Style Dictionary token pipeline
Storybook validation
TanStack Router/Query/Form
Radix or React Aria primitives
Zag/XState only where state machines are worth the overhead
elegy-codegraph verification
```

Avoid as V0 core dependency:

```txt
Plasmic
GrapesJS
Mitosis
large visual builder runtimes
multi-framework compilation
```

They are useful references, but they expand scope too early.

---

# 11. Area B — Agent Tool Creation

Agent tool creation is one of the highest leverage solved-unit domains.

A lot of tool work is repetitive:

```txt
discover capability
install dependency
define input schema
define output schema
write wrapper
normalize output
handle errors
document usage
write tests
register tool
create skill docs
expose through MCP or harness adapter
```

## Solved Units

| Unit                      | Description                                 |
| ------------------------- | ------------------------------------------- |
| `init-tool-package`       | Scaffold new tool package                   |
| `wrap-cli`                | Wrap an existing CLI binary                 |
| `wrap-http-api`           | Wrap HTTP API                               |
| `wrap-local-script`       | Wrap local script                           |
| `wrap-mcp-server`         | Wrap/export as MCP server                   |
| `create-tool-schema`      | Define input/output schema                  |
| `create-tool-test`        | Generate tests and fixtures                 |
| `create-tool-skill`       | Generate agent-facing skill docs            |
| `add-auth-flow`           | Add auth handling                           |
| `add-install-check`       | Verify binary/dependency installation       |
| `add-permission-boundary` | Declare allowed operations                  |
| `add-sandbox-policy`      | Define runtime restrictions                 |
| `add-output-normalizer`   | Convert raw output to agent-safe shape      |
| `register-tool`           | Register in Elegy/Holon                     |
| `export-mcp`              | Expose tool through MCP                     |
| `export-harness-skill`    | Emit Codex/OpenCode/Claude-style usage docs |

## Tool Contract

Every tool should declare:

```txt
tool.name
tool.description
input_schema
output_schema
side_effects
required_permissions
required_binaries
install_strategy
auth_strategy
sandbox_policy
timeout_policy
retry_policy
idempotency
observability
examples
tests
```

## Key Rule

A tool is not just a command.

A tool is:

```txt
command
+ schema
+ permission boundary
+ install check
+ auth model
+ sandbox policy
+ error normalization
+ tests
+ examples
+ agent-facing docs
```

## MCP Position

MCP should be treated as an external exposure protocol, not the internal source of truth.

Internal:

```txt
Elegy Tool Contract
```

Emit to:

```txt
MCP server
Codex/OpenCode/Claude-compatible skill docs
local CLI wrapper
Holon workflow node
JSON Schema/OpenAPI manifest
```

This gives Elegy richer semantics than a plain MCP tool definition.

---

# 12. Area C — Rust Code and Plugin Generation

Rust is a first-class target, but Rust generation should mostly use Rust-native patterns.

Nx can orchestrate Rust tasks in a larger monorepo, but it should not own the Rust architecture.

## Solved Units

| Unit                        | Description                                  |
| --------------------------- | -------------------------------------------- |
| `create-rust-crate`         | Add crate to workspace                       |
| `create-rust-plugin`        | Create Elegy/Holon plugin crate              |
| `create-rust-agent-tool`    | Create Rust-backed tool                      |
| `create-rust-workflow-node` | Create workflow node in Rust                 |
| `create-tauri-command`      | Create Tauri invoke command with TS bindings |
| `create-rust-cli`           | Create CLI binary crate                      |
| `create-domain-module`      | Domain-driven module structure               |
| `create-error-type`         | Generate typed error setup                   |
| `create-config-loader`      | Config/env loading pattern                   |
| `create-test-harness`       | Test harness with fixtures                   |
| `create-proc-macro-crate`   | Procedural macro crate                       |
| `create-codegen-crate`      | Codegen crate                                |
| `create-extractor`          | Codegraph extractor for a language/tool      |

## Recommended Tools

| Tool                                   | Role                            | Use Case                                     |
| -------------------------------------- | ------------------------------- | -------------------------------------------- |
| cargo-generate                         | Rust project template bootstrap | New plugin/crate/CLI from template           |
| xtask                                  | Workspace-native automation     | `cargo xtask scaffold`, `verify`, `generate` |
| cargo-make                             | Rust task runner                | Cross-platform task orchestration            |
| just                                   | Simple command runner           | Dev command aliases                          |
| build.rs                               | Build-time generation           | Generate bindings/assets at compile time     |
| procedural macros                      | Compile-time code generation    | Derives, attributes, DSL-like code expansion |
| uniffi / wasm-bindgen / ts-rs / specta | Binding/type generation         | Rust ↔ TS boundaries where appropriate       |

## Elegy-Specific Commands

```bash
elegy-plugin new <name> --lang rust
elegy-plugin add-tool <name> --lang rust
elegy-plugin add-workflow-node <name>
elegy-plugin add-tauri-command <name>
elegy-plugin add-extractor <language>
cargo xtask verify-generated
cargo xtask update-graphs
cargo xtask scaffold-plugin <name>
cargo xtask generate-bindings
```

## Strategy

Use:

```txt
cargo-generate for initial shape
xtask for repeated workspace-native operations
custom Rust CLI for stable public commands
procedural macros only for code-level patterns
Nx only as optional outer monorepo orchestrator
```

The Rust workspace should remain coherent without Nx.

---

# 13. Area D — API, Schema, and Contract Generation

This is a major “do not reinvent” area.

If an API boundary exists, agents should not manually duplicate:

* DTOs
* request types
* response types
* clients
* schemas
* endpoint contracts
* validation logic

Generate from a contract.

## Solved Units

| Unit                       | Description                                |
| -------------------------- | ------------------------------------------ |
| `create-api-contract`      | Define API contract                        |
| `create-rest-client`       | Generate REST client                       |
| `create-graphql-client`    | Generate GraphQL client                    |
| `create-protobuf-service`  | Generate from proto                        |
| `create-openapi-client`    | Generate OpenAPI client                    |
| `create-typespec-service`  | Create TypeSpec service model              |
| `create-smithy-service`    | Create Smithy model                        |
| `create-endpoint`          | Endpoint with types/tests                  |
| `create-dto`               | DTO with validation                        |
| `create-validation-schema` | JSON Schema/Zod/types                      |
| `create-api-test`          | Integration test for boundary              |
| `sync-api-client`          | Regenerate client from changed contract    |
| `wrap-openapi-as-tools`    | Create agent tools from OpenAPI operations |

## Recommended Tools

| Tool                   | Role                               | Use Case                               |
| ---------------------- | ---------------------------------- | -------------------------------------- |
| OpenAPI Generator      | REST client/server/docs generation | Generate clients, stubs, docs          |
| TypeSpec               | Higher-level API design language   | Source-of-truth API modeling           |
| Smithy                 | IDL/service modeling               | SDK/platform-grade APIs                |
| Protocol Buffers       | Cross-language RPC/data contracts  | gRPC, stable runtime boundaries        |
| GraphQL Code Generator | Typed GraphQL operations           | Frontend/backend GraphQL typing        |
| JSON Schema            | Data validation contracts          | Tool schemas, config schemas           |
| Zod                    | TypeScript-first validation        | TS runtime validation + inferred types |

## Agent Rule

```txt
If a contract exists, generate from the contract.
If no contract exists but the boundary is stable, create a contract first.
If the boundary is unstable, scaffold a candidate and mark it experimental.
```

## Elegy/Holon Use Cases

```txt
Holon internal API
plugin API
tool API
workflow node schema
Rust core ↔ TypeScript UI boundary
external HTTP API wrapper
generated agent tools from OpenAPI
generated UI data sources
```

---

# 14. Area E — Workflow Creation

Workflow generation is high-leverage, but workflow classes must be separated.

Different workflow models solve different problems.

## Workflow Classes

| Class                   | Use Case                            | Reference               |
| ----------------------- | ----------------------------------- | ----------------------- |
| DAG workflow            | Finite build/test/data pipelines    | Airflow-like            |
| State machine           | UI behavior, interactive processes  | XState/Zag-like         |
| Durable workflow        | Long-running external operations    | Temporal-like           |
| Agentic workflow        | Steps needing model judgment/repair | Elegy/Holon custom      |
| CI/build workflow       | Build/test/release pipelines        | Dagger/GitHub Actions   |
| Human approval workflow | Gated operations                    | Custom/Holon            |
| Event-driven workflow   | Reactive systems                    | Custom/runtime-specific |

## Solved Units

| Unit                            | Description                                  |
| ------------------------------- | -------------------------------------------- |
| `create-dag-workflow`           | DAG with tasks/dependencies                  |
| `create-state-machine`          | Statechart with transitions/guards/actions   |
| `create-durable-workflow`       | Persistent long-running workflow             |
| `create-agent-session-workflow` | Agentic session workflow with fallback gates |
| `create-ci-pipeline`            | Build/test/release pipeline                  |
| `create-validation-pipeline`    | Multi-gate validation chain                  |
| `create-human-review-gate`      | Approval checkpoint                          |
| `create-retry-policy`           | Retry/backoff configuration                  |
| `create-fallback-policy`        | Fallback/repair path                         |
| `create-workflow-node`          | Single workflow node                         |
| `create-workflow-edge`          | Typed edge                                   |
| `emit-workflow`                 | Emit to runtime target                       |

## Workflow IR

Define:

```txt
elegy.workflow.v0
```

With:

```txt
nodes
edges
inputs
outputs
state
side effects
retry policy
timeout policy
human approval gates
AI fallback gates
tool bindings
validation gates
evidence outputs
```

## Emit Targets

```txt
Holon runtime graph
Dagger pipeline
GitHub Actions workflow
Temporal workflow
local script
Markdown/spec view
agent session plan
```

## Recommended Tools

| Tool           | Role                                 | Use Case                                    |
| -------------- | ------------------------------------ | ------------------------------------------- |
| Dagger         | Programmable CI/build/test pipelines | Validation/release pipelines                |
| Temporal       | Durable workflows                    | Long-running automations, retries, recovery |
| Airflow        | DAG reference                        | Scheduled/batch/data pipelines              |
| Prefect        | Pythonic workflow orchestration      | Data/devops pipelines                       |
| XState         | Explicit statecharts                 | Complex UI/app/session state                |
| Zag.js         | UI component state machines          | Accessible component behavior               |
| GitHub Actions | CI target                            | Repository automation target                |

## Recommendation

For Holon/Elegy:

```txt
Use custom workflow IR as source of truth.
Use Dagger as a strong validation/release backend.
Use Temporal as reference/backend for durable workflows.
Use XState/Zag for explicit state machines.
Do not force all workflows into DAGs.
```

---

# 15. Area F — Project Architecture Scaffolding

Project architecture scaffolding creates coherent project structure, not just files.

## Solved Units

| Unit                  | Description                   |
| --------------------- | ----------------------------- |
| `create-app`          | Application scaffold          |
| `create-package`      | Reusable package              |
| `create-library`      | Library with tests/docs       |
| `create-module`       | Domain module                 |
| `create-feature`      | Feature slice                 |
| `create-service`      | Backend service               |
| `create-adapter`      | Adapter/anti-corruption layer |
| `create-repository`   | Data access layer             |
| `create-domain-model` | Domain model with tests       |
| `create-test-suite`   | Test harness                  |
| `create-doc-set`      | Documentation scaffold        |
| `create-adr`          | Architecture Decision Record  |
| `create-example`      | Example/demo project          |

## Recommended Tools

| Tool                 | Best For                                   | Elegy Role                         |
| -------------------- | ------------------------------------------ | ---------------------------------- |
| Nx generators        | Monorepo-aware code/project generation     | Main TS/React/package backend      |
| Nx sync generators   | Keep repo state in sync with project graph | Drift prevention/config sync       |
| Angular Schematics   | Virtual-filesystem transformations         | Design reference/codemod backend   |
| Backstage Scaffolder | Platform-level templates                   | Reference for catalogs/permissions |
| Plop                 | Micro-generators                           | Simple local file creation         |
| Hygen                | File-based templates                       | Lightweight template backend       |
| Yeoman               | Broad scaffolding ecosystem                | Reference/legacy ecosystem         |
| cargo-generate       | Rust project templates                     | Rust bootstrap backend             |
| xtask                | Rust-native automation                     | Rust workspace backend             |

## Recommendation

Do not make Backstage or Nx the universal foundation.

Use:

```txt
Nx for TypeScript monorepos
cargo-generate + xtask for Rust
custom Elegy contracts above both
```

---

# 16. Area G — Documentation and Spec Generation

Documentation is also a good solved-unit target.

Agents repeatedly create:

* ADRs
* specs
* readmes
* migration notes
* changelogs
* skill docs
* tool docs
* workflow docs
* validation reports

These should not be improvised every time.

## Solved Units

| Unit                       | Description                                        |
| -------------------------- | -------------------------------------------------- |
| `create-adr`               | Architecture Decision Record                       |
| `create-spec`              | Feature/system spec                                |
| `create-skill-doc`         | Agent-facing skill instructions                    |
| `create-tool-doc`          | Tool usage documentation                           |
| `create-workflow-doc`      | Workflow explanation                               |
| `create-validation-report` | Verification summary                               |
| `create-change-note`       | Change summary                                     |
| `sync-doc-index`           | Update doc index                                   |
| `link-related-docs`        | Add related-document metadata                      |
| `promote-research-to-spec` | Convert research baseline into implementation spec |

## Recommended Tools

| Tool/Pattern                | Role                         |
| --------------------------- | ---------------------------- |
| Markdown + YAML frontmatter | Durable, agent-readable docs |
| Obsidian-compatible links   | Human navigation             |
| JSON/YAML schemas           | Validate metadata            |
| markdownlint / prettier     | Formatting                   |
| custom Elegy doc generator  | Governed doc creation        |

## Recommendation

Keep docs as Markdown, but enforce structure through solved units.

Example:

```bash
elegy-doc create-adr generator-backend-strategy
elegy-doc create-spec elegy-ui-graph-v0
elegy-doc sync-index
```

---

# 17. Area H — Reverse Engineering Existing Projects

Reverse engineering is valuable but risky.

It should produce candidate contracts, not directly mutate code.

## Solved Units

| Unit                               | Description                             |
| ---------------------------------- | --------------------------------------- |
| `extract-project-graph`            | Extract code structure                  |
| `infer-ui-graph`                   | Infer UI graph from components/routes   |
| `infer-api-contract`               | Infer API contract from code/docs       |
| `infer-tool-contract`              | Infer tool boundaries                   |
| `infer-workflow-graph`             | Infer workflow from traces/config       |
| `infer-template-from-example`      | Detect recurring file/code pattern      |
| `promote-example-to-template`      | Create candidate solved unit            |
| `verify-template-against-examples` | Replay candidate against known examples |
| `detect-drift`                     | Detect divergence from generated intent |

## Pipeline

```txt
existing code
  -> codegraph extraction
  -> recurring pattern detection
  -> candidate solved unit
  -> human review
  -> manifest/template generation
  -> replay against examples
  -> promote to experimental
  -> validate in real work
  -> promote to stable
```

## Caution

Do not let reverse engineering automatically create stable solved units.

Reverse-engineered output should start as:

```txt
status: candidate
```

or:

```txt
status: experimental
```

because inferred patterns can overfit.

---

# 18. Validation and Drift Detection

A generator without verification is just a fancy copy-paste tool.

Every solved unit needs:

```txt
pre-generation validation
transactional generation
post-generation verification
drift detection
provenance
repair policy
```

## Required Gates

| Gate                      | Scope                             |
| ------------------------- | --------------------------------- |
| Schema validation         | Inputs/outputs                    |
| Precondition validation   | Project state before generation   |
| File ownership validation | Generated vs manual regions       |
| Graph validation          | Intended graph consistency        |
| Typecheck                 | Type safety                       |
| Unit tests                | Local behavior                    |
| Integration tests         | Boundary behavior                 |
| Storybook tests           | UI component behavior             |
| Accessibility checks      | A11y                              |
| Visual checks             | Rendering regressions             |
| Codegraph comparison      | Intended vs actual code structure |
| Install checks            | Tool dependencies                 |
| Permission checks         | Tool boundaries                   |
| Sandbox checks            | Runtime safety                    |
| Drift checks              | Manual divergence                 |

## Commands

```bash
elegy verify
elegy verify --against graph
elegy diff-intent
elegy drift
elegy repair-plan
elegy promote-template
elegy replay-template
```

## Drift Policy Modes

```txt
detect_only
detect_and_report
repair_generated_regions
fail_on_drift
allow_declared_escape_hatches
```

## Key Rule

Manual edits are allowed only in declared extension points or escape hatches.

Generated regions should be clearly marked or structurally tracked.

---

# 19. Agent Usage Model

Agents should classify work before invoking solved units.

Example:

```txt
User:
  "Add a settings page with account preferences."

Agent classification:
  - UI route/page? yes
  - Existing layout? dashboard
  - Existing template? generic settings page
  - Data source? account preferences API
  - Existing contract? yes/no
  - Solved units available? add-route + add-page + add-form + add-query

Agent executes:
  1. elegy-ui add-route /settings
  2. elegy-ui add-page SettingsPage --layout dashboard
  3. elegy-ui add-form AccountPreferences
  4. elegy-ui add-data-query accountPreferences
  5. elegy-ui verify

Agent manually implements:
  - field-specific business logic
  - unusual edge cases
  - missing contract details
```

Split:

```txt
Generator handles convention.
Agent handles ambiguity.
Human handles new abstraction design.
```

## Agent Should Invoke Solved Units When

```txt
the requested work matches an existing solved unit
preconditions are satisfied
the generated pattern is stable
manual custom design is not the main task
validation gates are available
```

## Agent Should Avoid Solved Units When

```txt
the abstraction itself is being designed
the existing architecture is inconsistent
the generator would overwrite unknown manual work
the work is one-off
the domain model is unclear
no validation path exists
```

---

# 20. Capability Matrix

| Area                 | Solved Unit Examples                        | Best References / Backends                             | Elegy Output                   |
| -------------------- | ------------------------------------------- | ------------------------------------------------------ | ------------------------------ |
| UI                   | page, route, layout, form, component        | Nx, Storybook, Radix, Zag, Style Dictionary            | UI graph + files + tests       |
| Agent tools          | CLI wrapper, API wrapper, schema, skill     | MCP, JSON Schema, Zod                                  | tool contract + wrapper + docs |
| Rust plugins         | crate, plugin, Tauri command, workflow node | cargo-generate, xtask, proc macros                     | Rust code + manifests + tests  |
| APIs                 | clients, DTOs, server stubs, schemas        | OpenAPI Generator, TypeSpec, Protobuf, GraphQL Codegen | generated clients/contracts    |
| Workflows            | DAG, state machine, durable workflow        | Dagger, Temporal, XState, Airflow                      | workflow IR + runtime emitter  |
| Project architecture | app, package, module, feature               | Nx, Schematics, Backstage, cargo-generate              | project graph + scaffold       |
| Documentation        | ADR, spec, skill, tool doc                  | Markdown, YAML frontmatter, custom generators          | governed docs                  |
| Reverse engineering  | infer graph/template/contract               | codegraph, static analysis, LLM-assisted inference     | candidate solved unit          |

---

# 21. Candidate Tooling Reference

## 21.1 Code Generation and Scaffolding

| Tool                 | Primary Use                                | Best Situation                              | Avoid When                                  |
| -------------------- | ------------------------------------------ | ------------------------------------------- | ------------------------------------------- |
| Nx generators        | Monorepo-aware generators                  | TS/React apps/libs/features/packages        | Rust-native internals or non-Nx repos       |
| Nx sync generators   | Keep files synced from graph/project state | Generated configs, graph-derived files      | Heavy logic or expensive generation         |
| Angular Schematics   | Virtual filesystem transforms              | Safe migrations/codemods                    | Non-Angular direct dependency unless needed |
| Backstage Scaffolder | Platform scaffolding                       | Service creation, catalog, permissions      | Small local generators                      |
| Plop                 | Micro-generators                           | Simple local boilerplate                    | Governed graph/provenance needs             |
| Hygen                | File-based local generation                | Small file templates                        | Complex project transformations             |
| Yeoman               | Broad scaffolding                          | Legacy/generic scaffolds                    | Modern governed monorepo workflows          |
| cargo-generate       | Rust project templates                     | New Rust crate/plugin/CLI                   | Repeated fine-grained mutations             |
| xtask                | Rust-native automation                     | Workspace-specific scaffold/verify/generate | Non-Rust projects                           |

## 21.2 UI, State, and Design Systems

| Tool              | Primary Use               | Best Situation                              |
| ----------------- | ------------------------- | ------------------------------------------- |
| Style Dictionary  | Token compilation         | Design token outputs to CSS/JS/etc.         |
| W3C Design Tokens | Token exchange contract   | Standard token representation               |
| Storybook         | UI workshop/testing       | Generated stories, a11y, interaction tests  |
| Radix UI          | Accessible primitives     | Unstyled React components                   |
| React Aria        | Accessible behavior/hooks | Complex accessible UI                       |
| Zag.js            | State-machine UI behavior | Generated components with explicit behavior |
| Ark UI            | Headless components       | Zag-based component systems                 |
| TanStack Router   | Typed routing             | Route/search-param state                    |
| TanStack Query    | Server state              | Queries/mutations/cache                     |
| TanStack Form     | Typed forms               | Generated form state                        |
| XState            | Statecharts               | Complex app/session workflows               |
| Playwright        | E2E browser tests         | Full UI validation                          |

## 21.3 API and Contracts

| Tool                   | Primary Use              | Best Situation                    |
| ---------------------- | ------------------------ | --------------------------------- |
| OpenAPI Generator      | REST client/server/docs  | Existing OpenAPI contract         |
| TypeSpec               | API-first modeling       | New APIs before OpenAPI exists    |
| Smithy                 | Service/SDK modeling     | Platform-grade service ecosystems |
| Protocol Buffers       | Cross-language RPC/types | gRPC/stable runtime contracts     |
| GraphQL Code Generator | Typed GraphQL code       | GraphQL schema + operations       |
| JSON Schema            | Runtime validation       | Tool/config/data schemas          |
| Zod                    | TS validation            | TypeScript-first tools/manifests  |

## 21.4 Workflow and Infrastructure

| Tool           | Primary Use                     | Best Situation                         |
| -------------- | ------------------------------- | -------------------------------------- |
| Dagger         | Programmable CI/CD              | Generated validation/release pipelines |
| Temporal       | Durable workflows               | Long-running operations with recovery  |
| Airflow        | Scheduled DAGs                  | Data/batch pipelines                   |
| Prefect        | Pythonic workflow orchestration | Data/devops automation                 |
| GitHub Actions | CI automation                   | Repo-level CI target                   |
| XState         | State machines                  | UI/session state                       |
| MCP            | Agent tool exposure             | External protocol adapter              |

## 21.5 Visual and Multi-Framework References

| Tool          | Role                        | Recommendation                 |
| ------------- | --------------------------- | ------------------------------ |
| Plasmic       | Visual builder              | Reference only for V0          |
| GrapesJS      | Visual/editor builder       | Reference only for V0          |
| Mitosis       | Multi-framework compilation | Defer until React V0 works     |
| Tokens Studio | Figma token sync            | Optional import path, not core |

---

# 22. Build Tracks

## Track 1 — `elegy-ui`

Scope:

```txt
React + TypeScript
routes
pages
layouts
components
tokens
Storybook
TanStack Router/Query/Form
codegraph verification
drift detection
```

Initial commands:

```bash
elegy-ui init
elegy-ui add-route settings
elegy-ui add-page SettingsPage
elegy-ui add-component AccountCard
elegy-ui add-form PreferencesForm
elegy-ui add-data-query accountPreferences
elegy-ui graph
elegy-ui verify
elegy-ui drift
```

Do not start with a visual builder. Start with graph + templates + validation.

## Track 2 — `elegy-toolkit`

Scope:

```txt
tool schema
CLI wrapper
install checks
auth notes
skill docs
tests
MCP export
Holon node export
```

Initial commands:

```bash
elegy-tool init
elegy-tool wrap-cli gh
elegy-tool wrap-script ./scripts/codegraph.ts
elegy-tool add-schema
elegy-tool add-skill
elegy-tool test
elegy-tool export-mcp
```

## Track 3 — `elegy-plugin-gen`

Scope:

```txt
Rust plugin scaffold
TypeScript plugin scaffold
workflow node scaffold
tool registration
test harness
docs
example project
```

Initial commands:

```bash
elegy-plugin new codegraph-rust
elegy-plugin add-tool extract-symbols
elegy-plugin add-workflow-node verify-codegraph
elegy-plugin add-test-harness
elegy-plugin verify
```

## Track 4 — `elegy-workflow`

Scope:

```txt
workflow IR
DAG/state-machine/durable distinctions
tool bindings
validation gates
AI fallback gates
human approval gates
emitters
```

Initial commands:

```bash
elegy-workflow init
elegy-workflow create validation-pipeline
elegy-workflow add-node typecheck
elegy-workflow add-node storybook-test
elegy-workflow add-edge typecheck storybook-test
elegy-workflow emit dagger
elegy-workflow verify
```

## Track 5 — `elegy-promote`

Scope:

```txt
pattern capture
candidate template generation
replay against examples
human review
promotion
versioning
deprecation
```

Initial commands:

```bash
elegy promote-pattern
elegy template candidate-from-example
elegy template replay
elegy template promote
elegy solved-unit list
elegy solved-unit deprecate
```

---

# 23. North-Star Architecture

```txt
Elegy Core
  |
  +-- Solved Unit Registry
  |     - manifests
  |     - versions
  |     - statuses
  |     - schemas
  |     - compatibility
  |     - usage evidence
  |
  +-- Generator Runtime
  |     - templates
  |     - codemods
  |     - graph mutation
  |     - transactions
  |     - rollback
  |
  +-- Graph Layer
  |     - intended graph
  |     - emitted graph
  |     - codegraph comparison
  |     - provenance
  |     - ownership
  |
  +-- Validation Layer
  |     - typecheck
  |     - tests
  |     - Storybook
  |     - accessibility
  |     - lint
  |     - security checks
  |     - custom gates
  |
  +-- Agent Interface
  |     - skill docs
  |     - tool manifests
  |     - MCP export
  |     - harness adapters
  |     - invocation policy
  |
  +-- Workflow Layer
  |     - DAGs
  |     - state machines
  |     - durable workflows
  |     - AI fallback gates
  |     - human approval gates
  |
  +-- Promotion Layer
        - evidence capture
        - candidate inference
        - replay
        - review
        - promotion
        - deprecation
```

Supported subprojects:

```txt
elegy-ui
elegy-toolkit
elegy-plugin-gen
elegy-workflow
elegy-codegen
elegy-doc
elegy-reverse
elegy-promote
```

These should share one contract model instead of becoming unrelated CLIs.

---

# 24. Implementation Direction

## Phase 1 — Manual Stable Units

Define 5-10 high-value solved units manually.

Recommended initial solved units:

```txt
elegy-ui.add-page
elegy-ui.add-component
elegy-ui.add-form
elegy-tool.wrap-cli
elegy-tool.create-skill
elegy-plugin.new-rust-plugin
elegy-plugin.add-tool
elegy-doc.create-adr
elegy-workflow.create-validation-pipeline
elegy-api.create-rest-client
```

## Phase 2 — Contract and Validation

Add:

```txt
manifest schema
input validation
output tracking
owned regions
extension points
graph mutations
verification commands
```

## Phase 3 — Agent Invocation

Expose solved units to agents through:

```txt
CLI commands
skill docs
tool manifests
MCP adapter where useful
harness-specific adapters
```

## Phase 4 — Drift and Repair

Add:

```txt
elegy drift
elegy diff-intent
elegy repair-plan
elegy verify --against graph
```

## Phase 5 — Promotion

Add:

```txt
example capture
candidate solved unit creation
template replay
human review
promotion/deprecation lifecycle
```

---

# 25. Main Risks

## Over-Generation

Risk:

```txt
creating generators for one-off cases
```

Mitigation:

```txt
require repeated evidence before promotion
support parameters instead of hyper-specific commands
keep status as experimental until replayed
```

## Template Composition Conflicts

Risk:

```txt
multiple generators own the same file/region
```

Mitigation:

```txt
owned regions
extension points
graph-level ownership
transactional generation
drift detection
```

## External Data Complexity

Risk:

```txt
API shape, auth, permissions, pagination, cache invalidation, realtime updates, partial failures
```

Mitigation:

```txt
explicit data contract
generated loading/empty/error states
cache invalidation policy
permission metadata
contract-first clients
```

## Agent Misuse

Risk:

```txt
agent invokes generator when design is still ambiguous
```

Mitigation:

```txt
agent invocation policy
preconditions
confidence thresholds
human review gates
```

## Multi-Framework Scope Explosion

Risk:

```txt
trying to support React, Vue, Svelte, Angular, Tauri, mobile, and visual builders too early
```

Mitigation:

```txt
React + TypeScript first
framework-neutral graph later
avoid Mitosis/Plasmic/GrapesJS as core V0 dependencies
```

## False Confidence from Reverse Engineering

Risk:

```txt
inferred pattern is overfit or wrong
```

Mitigation:

```txt
candidate status
human review
replay against multiple examples
never directly mutate code from inference
```

---

# 26. Final Position

The direction is not “use Nx everywhere.”

The direction is:

```txt
make repeated development work explicit,
contracted,
generated,
validated,
drift-aware,
and agent-invokable.
```

Nx is one excellent backend for TypeScript/React monorepos.

Rust should use cargo-generate and xtask.

APIs should use TypeSpec/OpenAPI/Protobuf/GraphQL codegen where applicable.

UI should use graph-governed templates, Storybook validation, design tokens, and accessible primitives.

Tools should use an internal Elegy Tool Contract and optionally emit MCP.

Workflows should use an Elegy workflow IR and emit to Dagger, Temporal, GitHub Actions, or Holon runtime depending on class.

The core product is not a generator.

The core product is:

```txt
a lifecycle for promoting repeated agent/developer work into governed deterministic capabilities.
```

That is the durable abstraction.
