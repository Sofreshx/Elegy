# Account Center fidelity ledger

## Evidence

- Concept: `docs/design/account-center-concept.png` at 1536 × 1024.
- Final captures: Playwright native viewport screenshots generated as `artifacts/account-center-desktop.png` (1536 × 1024) and `artifacts/account-center-mobile.png` (Pixel 7 viewport).
- Interaction verification: in-app Browser inspection at 1536 × 1024 for standalone, `?embed=1`, and `?connect=cloudflare&discovered=brave`; Playwright covers desktop and mobile, axe accessibility, dialogs, overflow, embed mode, and the discovery handoff.

## Comparison

| Design point | Concept | Implementation result |
|---|---|---|
| Shell | Fixed 272px navy rail and white workspace | Faithful at desktop; rail becomes the specified compact top bar on mobile |
| Information architecture | Header, attention strip, ruled inventory, persistent detail pane | Faithful; all four regions retain hierarchy and spacing |
| Inventory | One selected table row with pale-blue fill and blue left rule | Faithful; mobile intentionally hides secondary columns without horizontal overflow |
| Access decision | Explicit Review/Deny and time-bounded approval modal | Faithful; modal adds the security-critical opaque-lease explanation |
| Embedded surface | Same account components without product navigation | Faithful; `?embed=1` removes only the rail and keeps broker behavior/components |
| Responsive detail | Full-width mobile route/sheet with a close action | Faithful; selected accounts open a fixed full-width detail sheet below the top bar |
| Visual language | White canvas, navy, cobalt, ruled borders, Lucide outline icons | Faithful; status/destructive colors are darker than the concept to pass WCAG AA |

## Copy differences

- Concept: “read-only DNS access.” Runtime: the exact named operation, for example `dns.list access`, so approval text cannot hide or generalize the requested scope.
- The approval dialog adds client ID, purpose, duration, and “opaque, revocable lease—not the credential.” This is deliberate security context.
- Provider connection copy explicitly states that passwords and browser cookies are never imported.

## Remaining intentional deviations

- Provider marks use accessible initials rather than unofficial recreations of provider trademarks.
- Account rows and request text are broker-driven in production; concept identities and timestamps remain development fixtures only.
- The implementation uses a confirmation modal over the concept view during the captured decision state; the underlying layout remains pixel-structurally faithful.

The final UI is faithful to the approved concept while making the minimum security, accessibility, and dynamic-data changes required by the product contract.
