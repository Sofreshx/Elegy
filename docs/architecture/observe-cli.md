# Observe CLI

`elegy observe ...` is Elegy's read-only desktop and OS observation surface. It ships inside the umbrella `elegy` CLI and gives hosts a bounded subprocess interface for local inspection without turning observation into host-owned runtime authority.

## What It Ships Today

The current observe family is a mix of snapshot commands plus one bounded recorder command:

- `elegy observe processes`
- `elegy observe window`
- `elegy observe windows`
- `elegy observe screen`
- `elegy observe clipboard`
- `elegy observe filesystem`
- `elegy observe system`
- `elegy observe record`

All commands support `--json` machine output through the standard CLI envelope.

## Command Model

Snapshot commands return one immediate observation result.

`elegy observe record` is the current bounded recorder slice. It is a one-shot command, not a daemon or a start or stop lifecycle:

```bash
elegy observe record --duration-seconds 5 --poll-interval-ms 250 --json
```

Today that recorder:

- is Windows-first
- polls the current foreground window over a bounded duration
- generates its own `ObservationSession.sessionId`
- returns one governed `ObservationSession` artifact
- keeps `eventsPreview` bounded to 8 entries
- emits a compact `summary` instead of raw unbounded event history

This is the intended MVP shape for downstream hosts such as Holon that need one bounded capture artifact they can persist or transform locally.

## Output Contracts

Most observe commands return an `ObserveResult` envelope payload.

`elegy observe record --json` is the exception because it returns a governed observation artifact:

- `status: "ok"`
- `dataSchema: "https://elegy/contracts/schemas/observation-session.schema.json"`
- `data`: `ObservationSession`

The durable contract authority for the recorder output lives in:

- `contracts/schemas/observation-session.schema.json`
- `contracts/schemas/observation-event.schema.json`
- `contracts/schemas/observation-summary.schema.json`

Minimal governed examples live in:

- `contracts/fixtures/observation-session.minimal.json`
- `contracts/fixtures/observation-event.minimal.json`
- `contracts/fixtures/observation-summary.minimal.json`

## Platform Notes

- `processes`, `clipboard`, `filesystem`, and `system` are cross-platform.
- `window`, `windows`, `screen`, and `record` currently require Windows.
- `screen` uses a Windows GDI BitBlt path today and may not capture hardware-accelerated or DRM-protected content.
- `screen` currently supports only the primary monitor lane.

## Install And Distribution

`observe` does not have its own dedicated binary. It is part of the umbrella `elegy` CLI archive described in `docs/distribution.md`.

Typical local development flow:

```bash
cd rust
cargo run -p elegy-cli -- observe system --json
cargo run -p elegy-cli -- observe record --duration-seconds 1 --poll-interval-ms 50 --json
```

Typical downstream consumption flow:

- install the umbrella `elegy-cli-<cliVersion>-<target>.zip` asset
- resolve the extracted `elegy` executable directly
- keep governed contract consumption anchored to the installed `contracts/` directory

## Discovery Surface

The governed discovery authority for this surface is `contracts/fixtures/skill.elegy-observe.json`.

Hosts should prefer:

- `elegy agent check --json`
- `elegy agent manifest --json`
- `elegy agent discover --query <task> --detail --json`

Repo-local inspection can use:

- `elegy skills describe --skill-id observe --json`

## What Is Still Missing

The current observe MVP is intentionally small.

Not shipped yet:

- richer recorder backends such as Win32 hooks
- semantic desktop observation lanes such as UIA
- host-supplied recorder session ids
- raw event persistence as the default CLI contract
- daemon or start/stop recorder lifecycle commands
- a dedicated observe binary or separate recorder archive

Those remain deferred until there is real evidence that the bounded one-shot `observe record` lane is insufficient.
