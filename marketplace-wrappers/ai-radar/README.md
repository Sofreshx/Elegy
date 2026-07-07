# elegy-ai-radar

Marketplace wrapper for the external `elegy-ai-radar` plugin.

Authority:

```text
implementation repo -> plugin archive -> this wrapper -> .elegy/marketplace.json
```

This directory only provides public Elegy marketplace discovery metadata. Runtime
files, skills, schemas, and binaries are supplied by the published plugin
archive from the private implementation repo. The primary runtime contract is
the bundled CLI.

Release assets are published to the public `Sofreshx/Elegy` GitHub release
namespace using the canonical external plugin names:

```text
elegy-ai-radar-plugin-<target>.zip
elegy-ai-radar-plugin-<target>.zip.sha256
```
