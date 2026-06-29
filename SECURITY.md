# Security Policy

## Supported state

`Elegy` is the main monorepo for governed contract artifacts and the first-party Rust executable family that consumes them.

The repository is still in a **bootstrap and consolidation stage** after the legacy cleanup.

At this stage:

- `plugins/<name>/schemas/` and `plugins/<name>/fixtures/` hold the authority root for governed schemas, fixtures, manifests, support metadata, versioning, and policy
- `docs/governance/` holds the canonical operational policy assets (e.g. workflow/environment/branch enforcement mode selection)
- exported bundles under `artifacts/contracts` are the supported machine-readable handoff surface for downstream consumers
- the first-party Rust workspace at the repo root (`hosts/`, `plugins/`, `shared/`) is the active home for CLI, tooling, host, runtime, and adapter behavior that consumes governed artifacts
- the current contributor-facing self-authoring slice is the Rust CLI path for `author mcp`, `analyze mcp`, and `generate skills`, backed by `shared/tooling`
- this policy describes the repository boundary and reporting process; it does not claim a finished hardening story or a built-in MCP-native or skill-driven self-authoring product surface

## Reporting a vulnerability

Please do **not** open a public issue for a suspected vulnerability until maintainers have had a chance to assess it privately.

Preferred reporting path:

1. Use GitHub's private vulnerability reporting flow for this repository if it is enabled.
2. If private reporting is not available, contact the maintainers privately through GitHub before opening a public issue.

When reporting, include:

- a clear description of the issue
- affected packages, crates, workflows, or scripts
- reproduction steps if available
- expected impact
- any proof-of-concept artifacts needed to understand the report

Please avoid sending real secrets, production credentials, or sensitive customer data.

## Response expectations

During bootstrap, response times are best-effort. Maintainers will try to:

- acknowledge receipt promptly
- confirm whether the report is in scope
- communicate remediation plans when a valid issue is confirmed

## Current repository security posture

The current implemented posture includes repository and runtime safeguards such as:

- canonical output and boundary validation through `elegy contracts validate` and the conformance tests in `shared/core/`
- distribution bundle export and archive validation in `.github/workflows/distribution-artifacts.yml`
- Rust workspace formatting, linting, and test validation in `.github/workflows/rust-ci.yml`
- dependency review through `cargo-deny` and advisory review through `cargo-audit` in `.github/workflows/security.yml`
- repository secret scanning through `gitleaks` and Rust-oriented static analysis through CodeQL in `.github/workflows/security.yml`

## Important limitation

The planned security and audit posture is **not general malware detection**.

It is also **not**:

- endpoint protection
- a full DLP system
- a substitute for code review, dependency review, or host hardening
- proof that a descriptor, adapter, or upstream system is fully secure

The project may eventually flag malicious-looking or exfiltration-friendly patterns when they overlap with explicit audit or policy rules, but it must not claim to detect arbitrary malware or all malicious behavior.

## Out-of-scope reports

The following are generally out of scope unless they demonstrate a concrete Elegy issue:

- theoretical concerns with no plausible exploit path
- vulnerabilities only present in unsupported forks or local modifications
- reports that require disabled safeguards or unsupported future features
- general malware-detection expectations unrelated to the repository's explicit audit or policy rules

## Coordinated disclosure

Please allow maintainers reasonable time to investigate and fix validated issues before public disclosure.