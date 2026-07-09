---
title: main-protection Branch Ruleset
status: draft
owner: elegy-core
doc_kind: reference
---

# main-protection — Branch Ruleset for `main`

Status: **artifact**. Not yet applied. Apply after `reconcile/main-roro` merges CI-green.

## Why

The trunk is `main`. Merges happen through pull requests gated by CI.
This ruleset enforces that every merge to `main` passes required status checks
and preserves linear history.

## Where to apply

GitHub repo → Settings → Rules → Rulesets → New branch ruleset (not "New ruleset" for tags/organization).

## Settings checklist

| Setting | Value | Rationale |
|---|---|---|
| **Name** | `main-protection` | |
| **Enforcement status** | Active | |
| **Target branches** | Default branch (`main`) | Via "Add target" → "Default branch" |
| **Restrict deletions** | ✅ Checked | Prevents accidental branch deletion |
| **Block force pushes** | ✅ Checked | Non-fast-forward history is forbidden on trunk |
| **Require linear history** | ✅ Checked | All merges to main become fast-forward merges (rebased or squash-merged PRs). No merge bubbles from GitHub "Merge PR" button. |
| **Require a pull request before merging** | ✅ Checked | |
| └ Required approvals | **0** | Two-person team; author cannot approve their own PR. 0 approval + mandatory CI avoids solo bottleneck. Consider raising to 1 if external contributors join. |
| └ Dismiss stale approvals when new commits are pushed | ✅ Checked | |
| └ Require review from Code Owners | ❌ Unchecked | No CODEOWNERS file configured. |
| **Require status checks to pass before merging** | ✅ Checked | |
| └ Require branches to be up to date before merging | ✅ Checked | Strict — avoids merge races. |
| **Bypass list** | Empty (repository admin role retains implicit bypass for emergencies) | |

### Required status checks

Add each of these exact `context` names:

| Context | Source workflow |
|---|---|
| `Rust CI / fmt` | rust-ci.yml |
| `Rust CI / clippy` | rust-ci.yml |
| `Rust CI / test (ubuntu-latest)` | rust-ci.yml |
| `Rust CI / test (windows-latest)` | rust-ci.yml |
| `Rust CI / test (macos-latest)` | rust-ci.yml |
| `Rust CI / docs check v2 (authoritative)` | rust-ci.yml |
| `Security / cargo-deny` | security.yml |
| `Security / cargo-audit` | security.yml |
| `Security / gitleaks` | security.yml |
| `Security / codeql` | security.yml |
| `Repo Boundaries (package-boundaries compatibility name) / validate-package-boundaries` | package-boundaries.yml |
| `WS3 Governance (formalization compatibility name) / ws3-governance` | ws3-formalization-governance.yml |

Note: the context names include historical compatibility suffixes (e.g. `(package-boundaries compatibility name)`, `(formalization compatibility name)`). These match the `name:` field in the workflow YAML.

Distribution Artifacts jobs are intentionally NOT listed as required — they are heavyweight (matrix × 21) and not release-critical. They run and log but do not gate PR merge.
`Crate Verify` is also not listed as required while the crates.io gate is
disabled or advisory. Keep it as a manual smoke test unless crates.io publishing
is reactivated.

### Effect after application

- All PRs into `main` must pass the 12 listed checks before the merge button enables.
- No direct push to `main` — only reviewed, CI-green PRs.
- Force pushes to `main` are impossible.
