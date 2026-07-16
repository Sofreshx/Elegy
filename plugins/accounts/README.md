# Elegy Accounts

Elegy Accounts is the local authentication and credential broker for Codex,
Holon, and other compatible agent hosts. It stores secrets under the current
Windows user, presents human authorization in Account Center, and gives agents
only revocable metadata and purpose-bound access grants.

Use this plugin when a workflow is blocked on connecting, selecting, creating,
or reauthorizing an online account. Provider-specific tools still own the
business action and its operation vocabulary; this plugin does not expose raw
credentials or a generic authenticated HTTP tool.

The Windows MVP supports GitHub device authorization and guided Cloudflare API
token onboarding. Brave discovery is optional and supplies only an explicit
provider-origin hint.

See `docs/architecture.md`, `docs/security.md`, `docs/install-windows.md`, and
`DISTRIBUTION.md` for the trust boundary, installation, and validation lanes.
