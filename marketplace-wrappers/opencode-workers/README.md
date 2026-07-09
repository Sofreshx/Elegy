# elegy-opencode-workers

Marketplace wrapper for the external `elegy-opencode-workers` plugin.

Authority:

```text
implementation repo -> plugin archive -> this wrapper -> .elegy/marketplace.json
```

Implementation repository:

```text
https://github.com/Sofreshx/elegy-opencode-workers
```

This directory only provides public Elegy marketplace discovery metadata. Runtime
files, skills, optional MCP descriptors, and binaries are supplied by the
published plugin archive from the private implementation repo. The primary
runtime contract is the bundled CLI.

Windows Codex projection:

```text
bin/elegy-opencode-workers.exe
.mcp.json command -> ./bin/elegy-opencode-workers.exe
```
