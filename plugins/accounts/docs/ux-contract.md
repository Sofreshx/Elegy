# Account Center UX contract

## Primary screen

The first viewport answers three questions: which accounts are available, what needs the user's attention, and what agents may currently do. The dominant action is **Connect account**. A provider row is selected into a detail pane; grants are readable sentences, not raw scope strings.

## Core paths

- Connect/discover: Connect account → choose detected or listed provider → Allow/open provider → verify identity → choose default access → connected.
- Resume authorization: persistent attention banner → Review → open provider. Closing any Account Center surface is safe. Expired attempts become **Retry now** and create one fresh provider request only when the user is present.
- Agent request: attention item → inspect client, purpose, account, operations, and duration → allow once/allow for duration/deny.
- Create account: request details → automated progress → human checkpoint when required → resume → credential validation → connected.
- Revoke: account or grant detail → revoke → explicit consequence → confirmation → immediate state update.
- Audit: filter by account/client/result; view sanitized event explanation and correlation ID.

## Copy and safety

Never imply that browser passwords are imported. Use “Continue with your signed-in browser” or “Create a limited token” according to adapter capability. Show why access is requested, what actions it permits, which account it affects, and when it expires. Human checkpoints explain that the agent is paused and cannot complete the step.

Authorization requests must never exist only in a transient modal or agent-owned browser tab. Standalone Account Center, the Holon/Elegy embed, and agent tools resolve the same broker session ID. They may open or focus the review surface, but none of them own provider polling or private OAuth state.
