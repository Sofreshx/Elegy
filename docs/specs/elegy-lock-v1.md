---
title: Elegy lock v1
status: active
owner: elegy-tooling
doc_kind: spec
---

# Elegy lock v1

`elegy-lock/v1` is the exact reviewed package selection for one agent setup.
It pins the agent ID, package name and exact version, target, source,
publisher identity, archive digest, canonical manifest digest, canonical
capability-catalog digest, every declared entrypoint executable digest, and
explicit capability allowlist. `executableDigests` uses the package's
canonical `./`-prefixed paths and must match the entrypoints in the package
manifest exactly.

Production consumers must reject version ranges, rolling snapshots, publisher
drift, digest drift, undeclared capabilities, and installed-file changes. The
installer writes a file-hash receipt; a lock-backed MCP bridge verifies that
receipt and the manifest/catalog digests before exposing any capability.

Create and inspect a lock with:

```text
elegy lock create --package ./my-tool --archive ./dist/my-tool.zip \
  --agent-id my-agent --target x86_64-unknown-linux-gnu \
  --capability my-tool.read --output ./agent.elegy.lock.json
elegy lock verify --lock ./agent.elegy.lock.json
elegy install --archive ./dist/my-tool.zip --lock ./agent.elegy.lock.json \
  --target x86_64-unknown-linux-gnu --install-root ./installed
```

Lock updates are reviewed changes. Elegy does not silently upgrade an agent
from a package marketplace or rolling source.

`elegy install --update` verifies the existing receipt before staging a
replacement and restores the previous directory if publication fails.
`elegy uninstall` refuses to remove an installation whose manifest, receipt,
catalog, or declared files do not match the lock.
