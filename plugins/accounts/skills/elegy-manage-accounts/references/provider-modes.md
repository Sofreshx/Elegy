# Provider connection modes

| Provider | Preferred | Fallback | Browser discovery |
|---|---|---|---|
| Cloudflare | Guided, resource-limited API token | None in the MVP | `dash.cloudflare.com`; validate with Cloudflare's token verification endpoint |
| GitHub | Device authorization | None in the MVP | `github.com`; validate with the authenticated-user endpoint |

Prefer provider-managed short-lived and revocable authorization. For a guided token, request only scopes/resources needed by named operations and avoid global keys when a narrower token exists.

Google, Vercel, generic OAuth, GitHub Apps, and fine-grained PATs are planned compatibility modes. They are intentionally not advertised until their complete provider-specific authorization and verification paths pass the same acceptance contract.
