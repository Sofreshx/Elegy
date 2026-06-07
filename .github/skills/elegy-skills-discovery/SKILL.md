# Elegy Skills Discovery

## Primary Commands

For host onboarding and profile-filtered progressive discovery:

```bash
elegy agent check --json
elegy agent manifest --json --profile <profile-path>
elegy agent discover --query "<task>" --detail --json --profile <profile-path>
```

For registry inspection while developing Elegy or choosing a governed skill:

```bash
elegy-skills list --json
elegy-skills search --query "<task>" --json
elegy-skills resolve --query "<task>" --json
elegy-skills get --skill-id <id-or-alias> --json
elegy-skills capability --capability-id <id> --json
elegy-skills validate --file <path> --json
```

Umbrella compatibility commands remain available:

```bash
elegy skills list --json
elegy skills search --query "<task>" --json
elegy skills resolve --query "<task>" --json
elegy skills describe --skill-id <id-or-alias> --json
elegy skills capability --capability-id <id> --json
```

## Rules

- Treat skill definitions as authoritative. They live in `contracts/fixtures/skill.*.json`.
- Treat `elegy agent ...` as the host onboarding and discovery path.
- Treat `elegy-skills` as the dedicated governed registry surface and `elegy skills ...` as the umbrella compatibility path.
- Do not use or recreate v1 `skill-definition.*.json` files.
- Inspect `capabilities[].implementation.arguments` before invoking a command.
- Check `capabilities[].execution.hasSideEffects` before running mutations.
- Prefer stdin-capable commands when `input.stdinFormat` is present.
- Use `elegy run` when an MCP stdio host is needed. Side-effecting MCP tools are blocked by default unless the call is a dry run or the host is started with `--allow-side-effects`.
