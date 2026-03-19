# WS3 enforcement scripts

Canonical WS3 governance scripts live in this folder.

See `../../docs/governance/ws3-formalization-governance.md` for policy ownership, workflow posture, and local validation guidance.

## Scripts

- `resolve-enforcement-mode.ps1`
  - Resolves enforcement mode deterministically from context inputs and policy.
  - Defaults to `../../policies/formalization/visual-llm-enforcement-policy.json`.
  - Writes UTF-8 JSON mode artifact (default: `artifacts/ws3/mode-selection.json`).

- `evaluate-formalization-gates.ps1`
  - Evaluates gate decision from selected mode and violation input.
  - `warn` mode: non-blocking warnings.
  - `strict` mode: exits non-zero when violations exist.
  - Writes UTF-8 JSON gate artifact (default: `artifacts/ws3/gate-decision.json`).

- `write-mode-audit-artifact.ps1`
  - Combines mode selection + gate decision into one audit artifact.
  - Writes UTF-8 JSON audit file (default: `artifacts/ws3/mode-audit.json`).
