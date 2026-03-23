# Research: Dynamic CLI Tooling

## Question

Can dynamically created CLI tools work for skills when no better alternative exists, can they be safe enough to justify use, and when should Elegy use them?

## Short answer

Yes, but only as a bounded fallback.

Dynamic CLI tooling can work when it generates or executes tightly constrained wrappers around stable local commands. It should not be treated as the default execution model, and it should not allow arbitrary shell generation.

## When it can work

Dynamic CLI tooling is viable when all of the following are true:

- there is no better integration surface such as a library, package, file contract, HTTP API, or MCP endpoint
- the target command is stable, deterministic, and already accepted as part of the local operating environment
- the invocation can be described through explicit structured inputs rather than free-form shell text
- execution can be bounded by timeouts, output limits, cwd restrictions, and policy checks
- the result can be captured in a predictable format that the caller can validate

In practice, this means dynamic CLI tools are best treated as manifest-driven wrappers over known commands rather than as fully open command synthesis.

## Safety posture

Dynamic CLI tooling is only acceptable when it is fail-closed.

Minimum guardrails:

- no shell interpolation by default
- invoke explicit executables directly with typed argument lists
- require an allowlist or policy-approved command family
- restrict working directory and environment inheritance
- apply timeouts, output size limits, and exit-code handling
- capture structured audit records of what was invoked
- support dry-run or inspection before live execution
- prefer sandboxed or least-privilege execution where practical

Unsafe patterns:

- generating arbitrary shell scripts from prompts
- running destructive commands without an explicit allowlist and approval path
- depending on interactive CLIs that block or mutate state unpredictably
- accepting unknown installation state or undeclared system dependencies
- treating command text as authoritative instead of treating a manifest or typed request as authoritative

## When to use it

Use dynamic CLI tooling only when all of the following hold:

- the task is useful enough to justify an execution fallback
- the command family is locally available and operationally understood
- the action is not better expressed as an MCP tool, library integration, or governed file transformation
- the execution can be modeled through explicit policy-bounded arguments and outputs

Good candidates:

- local inspection tools with stable output
- repo-scoped developer tooling with deterministic flags
- policy-bounded file transforms or validators
- wrappers around established local operator commands when no library surface exists

Bad candidates:

- destructive infrastructure commands
- secret-heavy or credential-heavy workflows
- long-running interactive flows
- commands with highly unstable output contracts
- anything that is effectively a remote shell by another name

## Where it belongs in Elegy

Dynamic CLI tooling should not become a revived language-specific authority layer in Elegy.

Recommended placement:

- governed manifest or descriptor contracts only if cross-runtime interoperability actually requires them
- executable generation, inspection, and invocation in Rust tooling or dedicated Rust CLIs when a bounded surface exists; keep `elegy` as the general/compatibility CLI
- product-specific approval UX, transport wrapping, or local policy overlays in the consuming repo

This matches the broader burden-of-proof reset:

- governed artifacts under `contracts/`, `governance/`, `schemas/`, and `policies/` remain the authority for contracts and canonical semantics
- Rust in Elegy owns self-contained shared executable capabilities, with dedicated in-repo CLIs preferred for bounded paths
- app-local runtime integration stays local to the consumer

## Suggested phased path

1. Research and document the safety model first.
2. If needed, define a narrow manifest contract for dynamic CLI tool specs.
3. Add inspect-only and dry-run flows before any live execution path.
4. Gate live execution behind explicit policy and audit.
5. Keep the feature opt-in until the safety and operator ergonomics are proven.

## Decision

Dynamic CLI tooling is a valid fallback capability for skills, but it should remain:

- non-default
- Rust-first
- manifest-driven rather than prompt-driven
- policy-bounded
- auditable

If Elegy adds this feature, it should be because no better stable alternative exists, not because CLI execution is the easiest thing to generate.