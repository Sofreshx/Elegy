# Connection modes

Choose from the runtime provider pack returned by `account_discover`; never infer support from a hardcoded provider list.

| Method | Use when | Human checkpoint |
|---|---|---|
| OAuth PKCE | A public desktop client and authorization endpoint are available | provider consent, MFA, account selection |
| Device authorization | The provider supports the OAuth device grant | enter code and approve in browser |
| Scoped API token | The provider offers narrow, revocable tokens | user creates/pastes it only in Account Center |
| HTTP Basic/app password | A legacy API supports a dedicated app password | user enters username and app password only in Account Center |
| Client credentials | A machine identity is appropriate | user supplies client registration; broker exchanges tokens per use |
| Service credential | A reviewed code adapter exists | unsupported by the current declarative executor |

GitHub and Cloudflare are bundled v2 execution proof packs. Google is a v3
typed execution/lifecycle proof pack with fixture-tested PKCE, refresh, scope
validation, and revocation. It remains non-routable until a reviewed live
receipt proves those behaviors.

When a provider flow expires, use `account_attention_list`, then `account_present` or `account_resume_request`. Do not poll indefinitely or ask for secrets in chat.
