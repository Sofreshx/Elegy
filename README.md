# Elegy

Elegy is the authoritative home for governed contracts and policy artifacts plus the Rust runtime and CLI family that consumes, validates, or projects those artifacts.

The durable authority lives in `contracts/`, `governance/`, and `rust/`. The `.github/skills/*` files are contributor-routing outputs only. They help agents find the right CLI handoff, but they are not authoritative release, runtime, or policy surfaces.

## What ships now

### User-facing tools and release assets

| Surface | Readiness | What it is for |
| --- | --- | --- |
| `elegy` | Ready now | Umbrella CLI surface. This is the general entrypoint and currently carries the Mermaid toolset. |
| `elegy mermaid render` | Ready now | Render Mermaid input through the umbrella CLI. |
| `elegy mermaid reverse` | Ready now | Perform bounded reverse projection from Mermaid into canonical structures. |
| `elegy mermaid narrate` | Ready now | Produce concise narrative output from Mermaid/canonical inputs. |
| `elegy-mcp` | Ready now | Dedicated CLI for governed MCP descriptor authoring and descriptor analysis. |
| `elegy-skills` | Ready now | Dedicated CLI for governed MCP-to-skill generation. |
| `elegy-memory` | MVP / preview | Dedicated local memory CLI backed by the in-repo Rust implementation. Usable now for add, search, list, inspect, health, export, purge, contradictions, and the current preview `reembed` command surface. |
| Contracts bundle | Ready now | Governed machine-readable handoff for schemas, fixtures, compatibility metadata, and parity fixtures. |
| Installer bootstrap | Ready now | Generic install helper for release-based downstream consumption. |
| Wrapper archives | Ready now | Thin integration surfaces for `elegy-memory`, `elegy-mcp`, and `elegy-skills`. |
| `.github/skills/*` | Routing only | Repo-local, non-authoritative contributor-routing files. They are not the release surface and do not define runtime truth. |

## How to use Elegy

Downstream consumers should pin a tagged Elegy release and consume release assets, not sibling repositories or package-feed mirrors.

At a high level:

1. Install the governed contracts bundle when you need schemas, fixtures, manifests, or compatibility metadata.
2. Install only the CLI archives you need: `elegy-cli` (which contains the `elegy` binary), `elegy-mcp`, `elegy-skills`, and/or `elegy-memory`.
3. Optionally install the standalone bootstrap or wrapper archives when you want a repo-local integration path rather than direct archive extraction.
4. Invoke the selected CLI directly from the installed tool location.

Detailed distribution, archive, and installer guidance lives in [docs/distribution.md](docs/distribution.md).

## Surface summary

- `elegy` is the umbrella CLI and the home for Mermaid tooling. There is no separate Mermaid binary.
- `elegy-mcp` and `elegy-skills` are the current ready-to-use dedicated authoring surfaces.
- `elegy-memory` is the current shipped preview memory surface. It matches the implemented CLI in `rust/crates/elegy-memory` and should be described as an MVP, not as the older planned artifact-management flow.
- The governed contracts bundle is a first-class shipped output, even though it is not itself a CLI.
- Wrapper roots under `src/Elegy-memory`, `src/Elegy-mcp`, and `src/Elegy-skills` are thin integration surfaces only.
- Stable downstream consumption should pin semver release tags. The rolling `main-snapshot` prerelease is for latest-branch validation, not a stable contract.

## Workspace crate map

Not every Rust crate in the workspace is a directly shipped user-facing tool. The current workspace is organized into:

- User-facing CLI crates: `elegy-cli`, `elegy-memory`, `elegy-mcp`, `elegy-skills`
- Governed/data crates: `elegy-contracts`, `elegy-policy`, `elegy-descriptor`
- Runtime/host crates: `elegy-runtime`, `elegy-core`, `elegy-host-mcp`, `elegy-agent-events`
- Adapter/tooling crates: `elegy-adapter-fs`, `elegy-adapter-http`, `elegy-tooling`, `elegy-mermaid`

That distinction matters for consumers: the release lane is CLI-first, while the rest of the workspace is primarily implementation and runtime support.

## Read next

- [docs/architecture/README.md](docs/architecture/README.md)
- [docs/architecture/elegy-memory-v1.md](docs/architecture/elegy-memory-v1.md)
- [docs/distribution.md](docs/distribution.md)
- [CONTRIBUTING.md](CONTRIBUTING.md)
