# WS3 governance (formalization compatibility name)

Elegy owns the canonical WS3 governance policy, scripts, and reusable workflow posture.

`formalization` remains in several file names, script names, and artifact names as a historical compatibility label for existing callers. The current purpose of this lane is WS3 governance policy over the Rust-first, governed-artifact repo posture.

## Canonical assets

- Policy: `policies/formalization/visual-llm-enforcement-policy.json`
- Scripts:
  - `scripts/ws3/resolve-enforcement-mode.ps1`
  - `scripts/ws3/evaluate-formalization-gates.ps1`
  - `scripts/ws3/write-mode-audit-artifact.ps1`
- Workflow: `.github/workflows/ws3-formalization-governance.yml` (compatible path and display naming retained for reusable callers)

## Local validation

From the Elegy repo root:

```powershell
pwsh ./scripts/ws3/resolve-enforcement-mode.ps1 -BranchName main
pwsh ./scripts/ws3/evaluate-formalization-gates.ps1 -ViolationsPath ./artifacts/ws3/sample-violations.json
pwsh ./scripts/ws3/write-mode-audit-artifact.ps1
```

## Workflow posture

The reusable workflow is defined in `.github/workflows/ws3-formalization-governance.yml`.

- Direct runs in Elegy use the checked-out repo as both the caller workspace and governance asset source.
- Reusable runs check out the caller repository for artifacts and policy inputs, then derive the workflow source repository from `github.workflow_ref` so the canonical scripts can still run from Elegy when invoked cross-repo later.
- If `artifacts/ws3/formalization-violations.json` is missing, the workflow creates an empty violations file so governance posture and artifact generation still execute deterministically. That filename is retained as a compatibility contract for existing callers.

## Thin-caller readiness note

SAASTools should eventually call the reusable workflow rather than carry duplicate WS3 policy or script logic. When that lane is implemented, wire any repo-specific violations producer into the expected `artifacts/ws3/formalization-violations.json` compatibility contract or extend the reusable workflow input surface in a follow-up.
