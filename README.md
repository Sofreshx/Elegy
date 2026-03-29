# Elegy

Elegy is the authoritative home for governed contracts and policy artifacts plus the Rust runtime and CLI family that consumes, validates, or projects those artifacts.

The durable authority lives in `contracts/`, `governance/`, and `rust/`. The `.github/skills/*` files are contributor-routing outputs only. They help agents find the right CLI handoff, but they are not authoritative release, runtime, or policy surfaces.

## What ships now

| Surface | Readiness | What it is for |
| --- | --- | --- |
| `elegy` | Ready now | Umbrella CLI surface, including Mermaid render, reverse, and narrate commands. Mermaid output is derived and non-authoritative. |
| `elegy-mcp` | Ready now | Dedicated CLI for MCP descriptor authoring and analysis. |
| `elegy-skills` | Ready now | Dedicated CLI for governed MCP-to-skill generation. |
| `elegy-memory` | MVP / preview | Dedicated local memory CLI backed by the in-repo Rust implementation. Usable now for add, search, list, inspect, health, export, purge, contradictions, and the current preview `reembed` command surface. |
| `.github/skills/*` | Routing only | Repo-local, non-authoritative contributor-routing files. They are not the release surface and do not define runtime truth. |

## How to use Elegy

Downstream consumers should pin a tagged Elegy release and consume release assets, not sibling repositories or package-feed mirrors.

At a high level:

1. Install the governed contracts bundle when you need schemas, fixtures, manifests, or compatibility metadata.
2. Install only the CLI archives you need: `elegy-cli` (which contains the `elegy` binary), `elegy-mcp`, `elegy-skills`, and/or `elegy-memory`.
3. Invoke the selected CLI directly from the installed tool location.

Detailed distribution, archive, and installer guidance lives in [docs/distribution.md](docs/distribution.md).

## Surface summary

- `elegy` is the umbrella CLI and the home for Mermaid tooling. There is no separate Mermaid binary.
- `elegy-mcp` and `elegy-skills` are the current ready-to-use dedicated authoring surfaces.
- `elegy-memory` is the current shipped preview memory surface. It matches the implemented CLI in `rust/crates/elegy-memory` and should be described as an MVP, not as the older planned artifact-management flow.
- Wrapper roots under `src/Elegy-memory`, `src/Elegy-mcp`, and `src/Elegy-skills` are thin integration surfaces only.

## Read next

- [docs/architecture/README.md](docs/architecture/README.md)
- [docs/architecture/elegy-memory-v1.md](docs/architecture/elegy-memory-v1.md)
- [docs/distribution.md](docs/distribution.md)
- [CONTRIBUTING.md](CONTRIBUTING.md)
