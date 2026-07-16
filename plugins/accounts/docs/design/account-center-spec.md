# Account Center visual specification

Source: `account-center-concept.png` (1536 × 1024). This is the implementation contract for the MVP shell.

## Allowed first-viewport copy

Elegy Accounts; Accounts; Requests; Activity; Local only; All data stays on this device; Accounts & access; Connect your online accounts and control what local AI agents can do on your behalf.; Connect account; Codex is waiting for read-only DNS access; Review; Deny; Your accounts; Account; Verified identity; Connection; Agent access; Cloudflare; GitHub; Healthy; Connected with a scoped token; Connected with device authorization; These local AI agents have been granted access to this account.; Codex; Browser; Can read zones, DNS records, and settings.; Can read account profile and zones.; No write access.; Review access; Revoke account.

## Tokens

- True-white workspace `#ffffff`; navigation `#030d21` to `#07152b` with no visible gradient in implementation; primary text `#111827`; secondary `#40506a`.
- Accent `#2563eb`; selected background `#eef5ff`; borders `#dbe3ee`; success `#15803d`; attention `#d97706`; destructive `#b91c1c` (darkened from the concept to pass WCAG AA).
- Inter/Geist-like system sans; heading 36/44 650, section 20/28 650, body 16/24 400, controls 15/22 520, captions 13/18 400.
- Spacing scale 4, 8, 12, 16, 20, 24, 32, 40. Radius 8px controls, 10px framed regions. No elevation except focus/overlay states.
- Motion: 140ms ease-out selection/focus; detail pane may slide/fade 8px. Respect reduced motion.

## Geometry and containers

- Desktop rail 272px; content begins at 310px. Header and attention strip span the main list column while a 440px detail pane stays visible on the right.
- Account inventory is one ruled table/list, not a grid of cards. Selected row has pale-blue fill and a 3px cobalt left rail.
- The detail pane is the only strong bordered panel. Dividers organize verified identity, connection, grants, and actions.
- Mobile collapses the rail into a compact top bar; selecting an account opens the detail region as a full-width route/sheet with a clear Back action.

## Icon inventory

Use Lucide outline icons at 20–24px, 1.75px stroke for navigation, plus, review, close, status, browser, and delete. Provider marks are compact accessible initials/color marks if official assets are unavailable; do not approximate trademark geometry in custom SVG.

## Component architecture

`AppShell` owns standalone/embed chrome; `AccountInventory` owns rows and selection; `AttentionRequest` owns review/deny; `AccountDetail` owns identity, connection, grants, and revoke; `ConnectFlow` and `AccessReview` are modal routes; `AuditView` and `RequestsView` use the same shell and tokens. Broker data is accessed through a typed client; components never handle credentials.

## Core interactive state

Navigation changes views; account selection updates the detail pane; Review opens an access decision; Deny resolves the attention item; Connect account opens provider discovery; Review access lists grant consequences; Revoke requires confirmation and removes access locally. Embed mode hides the product rail but preserves all feature components and behavior.
