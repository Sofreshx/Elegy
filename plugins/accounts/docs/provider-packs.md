# Provider packs

Provider packs are the extension boundary for Elegy Accounts. The broker, Account Center, Brave bridge, and agent tools load these JSON files at runtime; they contain no compiled provider switch statements.

## When to add a pack

Add a pack when a downstream tool needs an account-backed operation and the provider uses one of the supported declarative methods: OAuth 2.0 authorization code with PKCE, OAuth device authorization, scoped API token, HTTP Basic/app password, or OAuth client credentials. `service_credential` is reserved for a reviewed code adapter and is not executable in the current release.

Put local packs in the installed `providers` directory or set `ELEGY_ACCOUNTS_PROVIDER_DIR` to a directory containing JSON packs. Restart the local broker after changing packs.

## Contract

Each file uses `elegy-account-provider/v1` and declares:

- stable lowercase `id`, display metadata, publisher, and pack version;
- browser origins used only for discovery hints;
- one or more auth profiles with endpoints, audience, identity verification, client registration, scopes, and optional credential fields;
- named operations mapped to the scopes required by provider tools.

All remote URLs must be HTTPS. Loopback HTTP is accepted only for deterministic tests and the local callback. Identity selectors are JSON Pointers and at least one must resolve before a credential can be stored. `required` assertions must all match.

```json
{
  "schema_version": "elegy-account-provider/v1",
  "id": "example-mail",
  "display_name": "Example Mail",
  "version": "1.0.0",
  "publisher": "example",
  "browser_origins": ["https://accounts.example.com"],
  "auth_profiles": [{
    "id": "desktop",
    "method": "oauth_pkce",
    "issuer": "https://accounts.example.com",
    "audience": "https://api.example.com",
    "authorization_url": "https://accounts.example.com/oauth/authorize",
    "token_url": "https://accounts.example.com/oauth/token",
    "identity": { "url": "https://api.example.com/me", "selectors": ["/email", "/id"] },
    "client": { "mode": "environment", "client_id_env": "EXAMPLE_CLIENT_ID" },
    "scopes": ["mail.read"]
  }],
  "operations": { "mail.messages.read": ["mail.read"] }
}
```

For manual credentials, declare `credential_fields` with `id`, `label`, `secret`, and optional browser `autocomplete`. Field values go directly from Account Center to the loopback broker, are verified, then encrypted with the OS-bound vault key. They are never returned to agents or stored by the extension.

## Trust and review

Bundled packs are trusted with the plugin release. Local third-party packs are configuration, not executable code, but still control where credentials are sent. Review their publisher, endpoints, identity assertions, scopes, and operation map before installation. A future signed-pack registry can add distribution trust without changing this contract.

## Proof requirements

Every pack must pass the generic conformance suite with a deterministic fake provider. Before marking a real pack ready, perform a read-only live proof: connect, verify identity, restart and resume if applicable, approve a least-privilege lease, execute one safe read through the broker, revoke, scan artifacts for canaries, and clean up. Provider-specific proof names are evidence only; they do not belong in generic broker or UI branches.
