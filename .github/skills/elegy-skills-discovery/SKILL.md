# Elegy Skills Discovery

Use this skill when an agent needs to discover the current Elegy capability surface before choosing a CLI or MCP action.

## Primary Commands

```bash
elegy skills list --json
elegy skills search --query "<task>" --json
elegy skills describe --skill-id <id-or-alias> --json
```

## Rules

- Treat v2 skill definitions as authoritative. They live in `contracts/fixtures/skill-definition-v2.*.json`.
- Do not use or recreate v1 `skill-definition.*.json` files.
- Inspect `capabilities[].implementation.arguments` before invoking a command.
- Check `capabilities[].execution.hasSideEffects` before running mutations.
- Prefer stdin-capable commands when `input.stdinFormat` is present.
- Use `elegy run` when an MCP stdio host is needed. Side-effecting MCP tools are blocked by default unless the call is a dry run or the host is started with `--allow-side-effects`.
