# elegy-checks

Marketplace wrapper for the external `elegy-checks` plugin.

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
elegy-checks-plugin-<target>.zip
elegy-checks-plugin-<target>.zip.sha256
```
