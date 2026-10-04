# Developing Elegy Memory

Follow the repository [authority chain](../../AGENTS.md). This file is the
Memory routing entrypoint; it does not define a second behavior contract.

## Start with the task

Read [CONTRIBUTING.md](CONTRIBUTING.md) for setup, a scratch-database example,
and the **change → contract → source → tests** map. Read only the relevant
row and its owning documents before editing.

- Behavior or scope: [feature matrix](docs/architecture/mvp-scope.md) and
  [memory model](docs/architecture/memory-model.md).
- Persistence: [storage schema](docs/architecture/storage-schema.md) and
  [migration framework](docs/architecture/migration-framework.md).
- API extension: [interface map](docs/architecture/traits-and-interfaces.md).
- Recall binding or feedback: [contextual recall spec](../../docs/specs/memory-contextual-recall-v1/spec.md).
- Qualification: [qualification guide](docs/qualification.md); run
  `cargo run --locked -p elegy-memory -- eval status --json` and
  `cargo run --locked -p elegy-memory -- eval next --json` from the repo root.

## Before finishing

Use the [validation matrix](CONTRIBUTING.md#validate-the-change). Preserve the
public crate re-exports, CLI envelopes and governed fixtures when reorganizing
code. Keep contract changes aligned with their Rust implementation and tests.
Source tests do not establish installed-host or live-provider readiness.

For transport, authentication or host policy, follow
[`hosts/memory-mcp/AGENTS.md`](../../hosts/memory-mcp/AGENTS.md).
