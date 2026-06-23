# Contracts Authority

This directory is the authored, language-agnostic authority root for
governed Elegy contract assets that have a real consumer in the Rust
workspace or the conformance suite.

Use it for:

- schemas under `contracts/schemas`
- fixtures under `contracts/fixtures`
- configuration templates, profiles, and built-in host blocks under
  `contracts/configuration`

Schemas and fixtures are kept only while a Rust production code path or a
conformance test loads them. When the consumer is removed, the file goes
in the same change. The Rust struct is the source of truth for any shape
described here.

Do not treat `artifacts/contracts` as the authored source of truth. That
directory is generated output for consumers and CI.
