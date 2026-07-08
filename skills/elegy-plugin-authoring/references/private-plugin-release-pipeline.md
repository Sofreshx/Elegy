# Private Plugin Release Pipeline

How to ship closed-source Elegy plugins from a private repo to the public
`Sofreshx/Elegy` marketplace. Two pipelines are supported. Prefer the
token-based pipeline; use the local-assets pipeline as a fallback.

## Pipeline A — Token-based CI upload (preferred)

The private repo's GitHub Actions workflow builds per-target archives and
uploads them to a release on the public `Sofreshx/Elegy` repo. This requires
a dedicated token because the default `GITHUB_TOKEN` can only write to its
own repo.

### Step 1 — Create a fine-grained PAT

Fine-grained PATs must be created in the GitHub UI (no CLI support yet).

1. Go to https://github.com/settings/personal-access-tokens/new
2. Set **Token name** to `elegy-release-upload`
3. Set **Resource owner** to `Sofreshx`
4. Set **Expiration** to 90 days (renew before expiry)
5. Under **Repository access**, select **Only select repositories** and pick `Sofreshx/Elegy`
6. Under **Permissions**, set:
   - **Repository permissions → Contents** to **Read and write**
   - All other permissions: **No access** (default)
7. Click **Generate token**
8. Copy the token value immediately (shown only once)

### Step 2 — Set the secret on the private repo

Use the helper script from the Elegy repo:

```bash
# From the Elegy repo root
bash scripts/setup-release-token.sh --repo Sofreshx/elegy-checks --token <pat-value>
```

Or with PowerShell:

```powershell
pwsh -File scripts/setup-release-token.ps1 -Repo Sofreshx/elegy-checks -Token <pat-value>
```

Or manually:

```bash
echo "<pat-value>" | gh secret set ELEGY_RELEASE_TOKEN --repo Sofreshx/elegy-checks
```

### Step 3 — Verify the release workflow

```bash
gh workflow run release-plugin.yml --repo Sofreshx/elegy-checks --ref main
gh run list --repo Sofreshx/elegy-checks --limit 1
```

All three target jobs should pass and archives should appear at:

```
https://github.com/Sofreshx/Elegy/releases/download/main-snapshot/<plugin>-plugin-<target>.zip
```

### Step 4 — Rotate the token

Fine-grained PATs expire. Before expiry:

1. Repeat Step 1 to create a new token
2. Repeat Step 2 to update the secret on every private plugin repo
3. Optionally revoke the old token at https://github.com/settings/personal-access-tokens

## Pipeline B — Local build + manual upload (fallback)

Use this when you cannot or do not want to set up a CI token. You build
locally and upload archives to the Elegy release yourself.

### Step 1 — Build archives locally

From the private plugin repo:

```powershell
# Windows (one target at a time)
pwsh -File scripts/package.ps1 -Target x86_64-pc-windows-msvc
pwsh -File scripts/package.ps1 -Target x86_64-unknown-linux-gnu
pwsh -File scripts/package.ps1 -Target aarch64-apple-darwin
```

This produces:

```text
dist/<plugin>-plugin-<target>.zip
dist/<plugin>-plugin-<target>.zip.sha256
```

### Step 2 — Upload to the Elegy release

```bash
# Ensure the release exists
gh release create main-snapshot --repo Sofreshx/Elegy --title main-snapshot --notes "External plugin assets" --prerelease

# Upload archives (clobber replaces existing)
gh release upload main-snapshot --repo Sofreshx/Elegy --clobber dist/*.zip dist/*.sha256
```

### Step 3 — Verify

```bash
gh release view main-snapshot --repo Sofreshx/Elegy --json assets | jq '.assets[].name'
```

## Checklist — Setting up a new private plugin

When creating a new private plugin repo for the Elegy marketplace:

1. **Impl repo**: Create a private GitHub repo with the Rust crate, skills, schemas, `.elegy-plugin/plugin.json`, `.codex-plugin/plugin.json`.
2. **Packaging scripts**: Add `scripts/validate.ps1` and `scripts/package.ps1` (mirror an existing private plugin like `elegy-checks` or `elegy-client-radar`).
3. **Release workflow**: Add `.github/workflows/release-plugin.yml` (mirror an existing private plugin). The workflow references `secrets.ELEGY_RELEASE_TOKEN`.
4. **CI workflow**: Add `.github/workflows/ci.yml` for basic fmt/clippy/test on push/PR.
5. **Elegy wrapper**: Add `marketplace-wrappers/<plugin-name>/` in the public Elegy repo with `.elegy-plugin/plugin.json` (Proprietary license, `elegy.marketplace-wrapper/v1` + `codex.plugin/v1` extensions) and `README.md`.
6. **Surfaces registration**: Add an entry to `distribution/surfaces.json` in the Elegy repo (`kind: external-plugin-wrapper`, `artifactBaseUrl`, `marketplaceCategory`).
7. **Regenerate marketplace**: Run `cargo run -p elegy-tooling --bin elegy-plugin-packaging -- marketplace generate --project .` and `marketplace validate --source .` from the Elegy repo.
8. **Set the release token**: Create a fine-grained PAT (Pipeline A, Step 1) and set `ELEGY_RELEASE_TOKEN` on the private repo (Pipeline A, Step 2). Or use Pipeline B for local uploads.
9. **Trigger first release**: Run the release workflow or upload manually.
10. **Verify**: Check that archives appear in the `Sofreshx/Elegy` `main-snapshot` release and the marketplace validates.

## Security notes

- Never commit tokens to git. Always use `gh secret set` or the GitHub UI.
- Fine-grained PATs are scoped to specific repos and permissions. Prefer them over classic PATs.
- The `ELEGY_RELEASE_TOKEN` only needs `contents:write` on `Sofreshx/Elegy`. No other permissions.
- Rotate tokens before expiry. Set a calendar reminder for the expiration date.
- If a token is compromised, revoke it immediately at https://github.com/settings/personal-access-tokens and set a new one on all repos.

## Current private plugin repos

| Repo | Secret set | Pipeline |
|---|---|---|
| Sofreshx/elegy-checks | Yes (ELEGY_RELEASE_TOKEN) | A (token-based CI) |
| Sofreshx/elegy-client-radar | Pending | A (token-based CI) |
| Sofreshx/elegy-ai-radar | Pending | A (token-based CI) |
