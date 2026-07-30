---
title: Elegy deprecations and reclassification
status: current
owner: elegy-core
doc_kind: reference
---

# Elegy deprecations and reclassification

This page records labels and distribution forms that agents must no longer
treat as current architecture. Removing an Elegy plugin manifest does not
remove the underlying code or skill; it removes a false claim about what kind
of product the surface is.

The machine-readable authority is
[`distribution/surfaces.json`](../distribution/surfaces.json). Its `kind`
controls release mechanics. `surfaceClass` states what the surface actually
is, `lifecycle` states its current posture, and `disposition` explains the
decision.

## Removed plugin packaging

| Surface | Current class | Current disposition |
|---|---|---|
| Documentation, MCP descriptor tooling, Memory, Planning | `tool` | Keep the behavior as product/development tools. Their former `.elegy-plugin` manifests and marketplace packaging are removed. |
| Documentation practices, Obsidian, plugin authoring, skill authoring | `skill` | Keep as inspectable guidance. A skill can be bundled by a native host, but it is not an Elegy connector merely because it has a manifest. |
| Checks | `tool` | Keep the external check runner as a tool. Remove the Elegy marketplace-wrapper claim. |
| OpenCode Workers, Codex Go Agents | `host-extension` | Keep blocked experimental metadata only. They are host-specific and are not portable Elegy connectors. |
| Client Radar, AI Radar, Question Studio | product library/tool outside this catalog | Keep domain logic with its owning product. The central wrappers were removed. |

## Retained or pending adapters

| Surface | Posture | Reason |
|---|---|---|
| Accounts | Active adapter plugin, `implemented`, non-routable | It owns authentication, provider connections, credential-safe execution, and a typed capability catalog. It remains hidden until clean-install and real-task receipts qualify it as `usable`. |
| Desktop | Adapter candidate in `rework` | OS operations are a valid reusable boundary, but the removed skill-only package did not declare a typed capability catalog or prove a portable install. |
| Observe | Adapter candidate in `rework` | OS data collection is a valid reusable boundary, but the removed skill-only package did not declare a typed capability catalog or prove a portable install. |

## Deprecated assumptions

- A directory under `plugins/` is not automatically a plugin. Several paths
  retain their historical location to avoid an unrelated source-tree move.
- A schema, fixture, generated projection, skill, wrapper, or passing source
  suite does not establish plugin eligibility or usability.
- `skill-package` is a release/build kind, not an Elegy plugin class.
- MCP is an optional host projection over a capability. A CLI remains the
  portable executable boundary when the same operation can run without MCP.
- New business/domain logic must not be added to Elegy as a plugin. Extract an
  adapter only after the concrete external boundary exists; extract a generic
  contract only after two independent consumers prove it.

