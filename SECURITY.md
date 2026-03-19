# Security Policy

## Supported state

`Elegy` is the main monorepo for the project's shared formalization packages, governed contract artifacts, and first-party Rust runtime family.

The repository is still in a **bootstrap and consolidation stage**.

At this stage:

- the `.NET` packages under `src/` remain the authority for governed contracts, schemas, fixtures, and canonical skill/MCP formalization
- the Rust workspace under `rust/` is the active home for behavior-heavy runtime, host, transport, and adapter logic
- interfaces may still tighten as the former sibling runtime repos are verified and closed out
- security-related docs describe the current implementation boundary and reporting process, not a claim of comprehensive platform hardening

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

- package-boundary and architecture validation for the formalization families
- deterministic exported contract artifacts for downstream consumers
- Rust workspace lint/test validation in CI
- dependency-policy review through `cargo-deny` and advisory review through `cargo-audit`
- secret scanning and CodeQL analysis at the monorepo level
- conservative runtime policy in the Rust runtime family, including bounded filesystem roots, allowlisted outbound HTTP targets, timeout and size limits, and rejection of credential-bearing HTTP URLs

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