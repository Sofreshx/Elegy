# Distribution and downstream consumption

Elegy is intended to be consumed through versioned packages and versioned exported artifacts, not through brittle sibling-repository workspace references.

The active authority cutover moves governed schemas, fixtures, manifests, and support metadata into `contracts/`, with version policy under `governance/version-policy.json`. Remaining `.NET` packaging guidance is transitional.

## .NET packages

The `.NET` libraries under `src/` are prepared for NuGet-style distribution. The initial internal/prerelease-ready distribution model is GitHub Packages.

- Feed URL: `https://nuget.pkg.github.com/Sofreshx/index.json`
- Transitional package version source of truth: `governance/version-policy.json` (`manifestPackage.version`)
- Release workflow: `.github/workflows/publish-distribution.yml`
- Validation/artifact workflow: `.github/workflows/distribution-artifacts.yml`

### Authenticate locally

Outside GitHub Actions, use a GitHub token with at least `read:packages`.

Example `NuGet.config`:

```xml
<?xml version="1.0" encoding="utf-8"?>
<configuration>
  <packageSources>
    <clear />
    <add key="nuget.org" value="https://api.nuget.org/v3/index.json" />
    <add key="github-elegy" value="https://nuget.pkg.github.com/Sofreshx/index.json" />
  </packageSources>
  <packageSourceCredentials>
    <github-elegy>
      <add key="Username" value="%GITHUB_USER%" />
      <add key="ClearTextPassword" value="%GITHUB_TOKEN%" />
    </github-elegy>
  </packageSourceCredentials>
</configuration>
```

### Add a package

```bash
dotnet add package Elegy.Formalization.Contracts --version 0.2.0
dotnet add package Elegy.Formalization.Mcp --version 0.2.0
```

In GitHub Actions, prefer `GITHUB_TOKEN` against the repository-owner feed URL and avoid storing a second package secret unless a cross-repository permission boundary requires it.

## Contract bundles and non-NuGet assets

Contract schemas, fixtures, compatibility metadata, and parity fixtures are exported with:

```powershell
pwsh ./scripts/export-contracts.ps1 -CreateArchive
```

Outputs:

- expanded directory: `artifacts/contracts`
- versioned archive: `artifacts/distribution/elegy-contracts-<bundleVersion>.zip`

The archive is intended for downstream consumers that do not restore NuGet packages directly, including Node.js tooling, Rust consumers, and integration environments that only need the governed contract bundle.

Current governed workflow artifacts in that bundle include both the portable workflow contract and the canonical workflow graph contract:

- `canonical-workflow.schema.json`
- `canonical-workflow-graph.schema.json`
- `fixtures/canonical-workflow.minimal.json`
- `fixtures/canonical-workflow-graph.minimal.json`

## Downstream guidance

- Prefer `PackageReference` and versioned feed consumption for `.NET` dependencies.
- Prefer release assets or workflow artifacts for schema/fixture bundles.
- Do not hard-code sibling checkout paths or assume a shared parent workspace layout.
- Treat `artifacts/contracts` and release bundle contents as the supported machine-readable handoff surface.

## Maintainer flow

1. Update bundle and manifest package metadata/version in `governance/version-policy.json`.
2. Run `pwsh ./scripts/export-contracts.ps1 -CreateArchive`.
3. Run `dotnet pack` for affected source packages.
4. Publish through the GitHub Actions workflows when ready.
