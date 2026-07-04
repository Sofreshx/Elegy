# Closed-Source Plugin Wrapper Template

Use this template to register an external or closed-source plugin in the Elegy marketplace. The wrapper provides discovery metadata only — the actual implementation lives in the external source.

## Usage

1. Copy this directory to `plugins/<your-plugin-name>/`
2. Edit `.elegy-plugin/plugin.json` with your plugin's metadata
3. Set the `source` field to point to your external repository or registry
4. Add an entry to `distribution/marketplace.json`

## Directory structure

```
plugins/<your-plugin-name>/
  .elegy-plugin/
    plugin.json       # discovery metadata + external source pointer
  README.md           # local docs about this wrapper (optional)
```

No `skills/` or `src/` directory — those live in the external repository.

## Source types

### Git repository

```json
{
  "source": {
    "source": "git",
    "url": "https://github.com/org/private-plugin",
    "tag": "v1.0.0"
  }
}
```

### Package registry

```json
{
  "source": {
    "source": "registry",
    "url": "https://registry.elegy.dev",
    "package": "elegy-private-plugin",
    "version": "^1.0.0"
  }
}
```

### Local (in-repo)

```json
{
  "source": {
    "source": "local",
    "path": "plugins/my-plugin"
  }
}
```

## Marketplace entry

Add to `distribution/marketplace.json`:

```json
{
  "name": "elegy-my-plugin",
  "source": { "source": "git", "url": "https://github.com/org/private-plugin", "tag": "v1.0.0" },
  "policy": { "installation": "AVAILABLE", "authentication": "ON_INSTALL" },
  "category": "Developer Tools"
}
```

## Authentication policy

- `NONE` — no auth required
- `ON_INSTALL` — auth needed when installing the plugin
- `ON_USE` — auth needed when invoking the plugin
