# Elegy

Elegy is the authoritative home for governed contracts and policy artifacts plus the Rust runtime and CLI family that consumes, validates, or projects those artifacts.

The durable authority lives in `contracts/`, `governance/`, and `rust/`. The `.github/skills/*` files are contributor-routing outputs only. They help agents find the right CLI handoff, but they are not authoritative release, runtime, or policy surfaces.

## Release channels

Elegy publishes the same asset families in two channels:

- Stable releases such as `v1.3.2`. These are the tags downstream consumers should pin.
- Rolling prerelease `main-snapshot`. This is refreshed on every push to `main` and is meant for latest-branch validation, not as a stable contract.

The important difference is the stability promise, not the package shape. `main-snapshot` is not missing extra tools; it carries the same release surface as the stable lane.

## What most users actually need

Most downstream users only need two things:

1. The contracts bundle if they need schemas, fixtures, or compatibility metadata.
2. One or more CLI archives for the commands they actually plan to run.

Everything else in the release exists to support verified installation, automation, or bounded integrations.

## Asset guide

| Asset family | Example | Who usually needs it | Why it ships |
| --- | --- | --- | --- |
| Contracts bundle | `elegy-contracts-<bundleVersion>.zip` | Any downstream that validates or consumes governed artifacts | Canonical schemas, fixtures, compatibility metadata, and parity fixtures. |
| Umbrella CLI archive | `elegy-cli-<cliVersion>-<target>.zip` | Most CLI users | Ships the `elegy` binary plus an archive README. This is the umbrella entrypoint for Mermaid, diagram, skills discovery/runtime, observe, desktop, repo, web, data, and notify. |
| Dedicated CLI archives | `elegy-mcp-<cliVersion>-<target>.zip`, `elegy-skills-<cliVersion>-<target>.zip`, `elegy-memory-<cliVersion>-<target>.zip` | Users who want bounded tool-specific binaries | Ships dedicated binaries for MCP authoring/analysis, skill generation, and the preview memory surface. |
| Installer bootstrap | `elegy-installer-<bundleVersion>.zip` | Downstream repos that want scripted installation | Carries the generic installer helper plus an archive README. Useful for bootstrapping, but not required if you extract archives directly. |
| Wrapper archives | `elegy-memory-wrapper-<bundleVersion>.zip`, `elegy-mcp-wrapper-<bundleVersion>.zip`, `elegy-skills-wrapper-<bundleVersion>.zip` | Hosts that want a thin repo-local integration surface | Packages wrapper metadata, a local install entrypoint, a bundled installer copy, and an archive README for that bounded surface. |
| Release manifest | `elegy-release-manifest-<bundleVersion>.json` | Installer and maintainers | Declares the authoritative asset set, archive contents, targets, sizes, and hashes the installer should trust. |
| Release checksums | `elegy-release-checksums-<bundleVersion>.json` | Installer and maintainers | Lets the installer verify downloaded assets fail-closed instead of guessing. |

## Feature families in `elegy`

The umbrella `elegy` binary carries these feature families:

- **Discovery/runtime** — `elegy skills list`, `elegy skills describe`, `elegy skills search`, `elegy run`
- **Mermaid / diagram** — `elegy mermaid *`, `elegy diagram *`
- **Observe** — `elegy observe processes|window|windows|screen|clipboard|filesystem|system`
- **Desktop** — `elegy desktop click|type|key|focus|move|minimize|maximize`
- **Repo** — `elegy repo status|diff|branches|log`
- **Web** — `elegy web fetch|ping`
- **Data** — `elegy data convert|extract|validate`
- **Notify** — `elegy notify toast|webhook`
- **Dedicated binaries** — `elegy-memory`, `elegy-mcp`, `elegy-skills`

## Which download contains which features

| Download | What you get |
| --- | --- |
| `elegy-cli-<cliVersion>-<target>.zip` | The `elegy` binary and all umbrella feature families above in one archive. |
| `elegy-memory-<cliVersion>-<target>.zip` | The dedicated `elegy-memory` binary. |
| `elegy-mcp-<cliVersion>-<target>.zip` | The dedicated `elegy-mcp` binary. |
| `elegy-skills-<cliVersion>-<target>.zip` | The dedicated `elegy-skills` binary. |
| `elegy-*-wrapper-<bundleVersion>.zip` | Wrapper-specific integration content plus bundled installer helper. |
| `elegy-installer-<bundleVersion>.zip` | Standalone installer bootstrap. |

Every downloadable zip now includes an archive-root `README.md` so the distribution stays self-describing after extraction or mirroring.

## User-facing tools

| Surface | Readiness | What it is for |
| --- | --- | --- |
| `elegy` | Ready now | Umbrella CLI surface. Use this when you want the general command surface, including Mermaid tooling. |
| `elegy mermaid render` | Ready now | Render canonical workflow inputs into Mermaid output. |
| `elegy mermaid reverse` | Ready now | Perform bounded reverse projection from Mermaid into canonical workflow graph semantics. |
| `elegy mermaid narrate` | Ready now | Produce concise narrative output from Mermaid or canonical workflow graph inputs. |
| `elegy diagram create` | Ready now | Create empty semantic diagrams of a given type. |
| `elegy diagram patch` | Ready now | Surgically add/remove nodes and edges. Supports JSON stdin (`--patch-stdin`) for agent-friendly invocation. |
| `elegy diagram narrate` | Ready now | Produce human-readable summaries of diagram content. Accepts file or stdin. |
| `elegy diagram render` | Ready now | Render diagrams to Mermaid or other formats. Accepts file or stdin. |
| `elegy skills list` | Ready now | List all available skill definitions with metadata, category filtering, and lifecycle state. Supports `--detail` for capability-level progressive disclosure. |
| `elegy skills describe` | Ready now | Show full detail for a specific skill including all capabilities and implementation info. |
| `elegy skills search` | Ready now | Search skills by keyword or trigger pattern with scored ranking. Supports `--detail` for capability-level progressive disclosure. |
| `elegy run` (MCP host) | Ready now | Start the MCP host server over stdio. Now serves both resources and tools. |
| `elegy-mcp` | Ready now | Dedicated CLI for governed MCP descriptor authoring and descriptor analysis. |
| `elegy-skills` | Ready now | Dedicated CLI for governed MCP-to-skill generation. |
| `elegy-memory` | MVP / preview | Dedicated local memory CLI backed by the in-repo Rust implementation. Usable now for add, search, list, inspect, health, export, purge, contradictions, and the current preview `reembed` command surface. |
| `elegy observe processes` | Ready now | Snapshot running processes with PID, name, memory, and CPU. Supports `--filter` for name matching. |
| `elegy observe window` | Ready now | Get foreground window info including title, PID, and bounds. Windows only. |
| `elegy observe windows` | Ready now | List all visible windows. Supports `--filter` for title matching. Windows only. |
| `elegy observe screen` | Ready now | Capture screen as PNG. Save to file with `--output` or get base64 in JSON. Windows only. |
| `elegy observe clipboard` | Ready now | Read current clipboard text and image presence. Cross-platform. |
| `elegy observe filesystem` | Ready now | Snapshot-diff a directory over a bounded time window. Cross-platform. |
| `elegy observe system` | Ready now | System hardware and OS info snapshot. Cross-platform. |
| `elegy desktop click` | Ready now | Simulate mouse click at pixel coordinates. Supports `--dry-run` for preview. Windows only. |
| `elegy desktop type` | Ready now | Simulate keyboard text input (Unicode). Supports `--dry-run`. Windows only. |
| `elegy desktop key` | Ready now | Simulate key combos (e.g., ctrl+s, alt+tab). Supports `--dry-run`. Windows only. |
| `elegy desktop focus` | Ready now | Focus a window by title or HWND. Strict matching — fails on ambiguity. Windows only. |
| `elegy desktop move` | Ready now | Move and resize a window by title or HWND. Supports `--dry-run`. Windows only. |
| `elegy desktop minimize` | Ready now | Minimize a window by title or HWND. Supports `--dry-run`. Windows only. |
| `elegy desktop maximize` | Ready now | Maximize a window by title or HWND. Supports `--dry-run`. Windows only. |
| `elegy repo status` | Ready now | Structured read-only git status with branch tracking and changed entry counts. |
| `elegy repo diff` | Ready now | Structured read-only diff summary against `HEAD` or an explicit base ref. |
| `elegy repo branches` | Ready now | List local branches, current branch, and upstream tracking refs. |
| `elegy repo log` | Ready now | Return a bounded structured commit log. |
| `elegy web fetch` | Ready now | Perform bounded HTTP fetches with optional headers, request body, and JSON extraction. |
| `elegy web ping` | Ready now | Check HTTP/HTTPS reachability with bounded timeouts. |
| `elegy data convert` | Ready now | Convert structured data between JSON, YAML, TOML, and CSV. |
| `elegy data extract` | Ready now | Extract JSON values by JSON Pointer or dotted path. |
| `elegy data validate` | Ready now | Validate input JSON against a JSON Schema file. |
| `elegy notify toast` | Ready now | Show a local toast notification. Windows only. |
| `elegy notify webhook` | Ready now | Send an outbound webhook POST with optional text or JSON payload. |

## What is intentionally not a separate release package

- There is no dedicated Mermaid binary. Mermaid lives on the umbrella `elegy` CLI by design.
- `.github/skills/*` are repo-local contributor-routing files, not release/runtime authority.
- Most Rust crates in the workspace are implementation crates, not directly shipped user-facing packages.

## How to use Elegy

Downstream consumers should pin a tagged Elegy release and consume release assets, not sibling repositories or package-feed mirrors.

At a high level:

1. Pin a stable semver release unless you are explicitly testing current `main`.
2. Install the governed contracts bundle when you need schemas, fixtures, manifests, or compatibility metadata.
3. Install only the CLI archives you need: `elegy-cli` (which contains the `elegy` binary), `elegy-mcp`, `elegy-skills`, and/or `elegy-memory`.
4. Optionally use the standalone installer bootstrap or wrapper archives when you want a scripted repo-local integration path rather than direct archive extraction.
5. Invoke the selected CLI directly from the installed tool location.

Detailed distribution, archive, and installer guidance lives in [docs/distribution.md](docs/distribution.md).

## Surface summary

- `elegy` is the umbrella CLI and the home for Mermaid tooling. There is no separate Mermaid package missing from the release.
- `elegy-mcp` and `elegy-skills` are the current ready-to-use dedicated authoring surfaces.
- `elegy-memory` is the current shipped preview memory surface. It matches the implemented CLI in `rust/crates/elegy-memory` and should be described as an MVP.
- The governed contracts bundle is a first-class shipped output even though it is not itself a CLI.
- The installer, manifest, and checksum assets are useful support assets, but many direct CLI users will never touch them manually.
- Wrapper roots under `src/Elegy-memory`, `src/Elegy-mcp`, and `src/Elegy-skills` are thin integration surfaces only.
- Stable downstream consumption should pin semver release tags. The rolling `main-snapshot` prerelease is for latest-integration validation only.

## Agent integration

Elegy is designed to be consumable by LLM agents and automation systems. Key patterns:

- **Structured JSON output:** All commands support `--json` for machine-readable envelope output with `correlationId`, diagnostics, and typed data.
- **Stdin-friendly:** Diagram and Mermaid commands accept input from stdin when `--input` is omitted, enabling pipe-based composition.
- **JSON patch for mutations:** Use `--patch-stdin` to pipe a JSON `DiagramPatch` object instead of fragile positional arguments.
- **Governed skill definitions:** Each capability family has a v2 skill definition in `contracts/fixtures/` describing exact invocation patterns, parameters, and governance metadata.
- **MCP tool dispatch:** The MCP host (`elegy run`) auto-generates MCP tools from v2 skill definitions. Agents connecting via MCP get `tools/list` and `tools/call` backed by CLI subprocess dispatch — zero configuration required.
- **Runtime discovery:** Use `elegy skills list --json` and `elegy skills describe --skill-id <id> --json` to discover all capabilities, their invocation patterns, and parameters at runtime.
- **Progressive disclosure:** Discovery commands support `--detail` for capability-level information. Default output is compact (~50 tokens per skill). Use `elegy skills describe` for full definitions. Token cost estimates are included in JSON output to help agents manage context budgets.
- **Skill router meta-skill:** The `skill-router` skill definition lets agents discover the discovery surface itself. Load one skill definition to access all capabilities on demand.

For the full agent integration guide, see [docs/agent-integration.md](docs/agent-integration.md) (coming soon).

## Workspace crate map

Not every Rust crate in the workspace is a directly shipped user-facing tool. The current workspace is organized into:

- User-facing CLI crates: `elegy-cli`, `elegy-memory`, `elegy-mcp`, `elegy-skills`
- Governed/data crates: `elegy-contracts`, `elegy-policy`, `elegy-descriptor`
- Runtime/host crates: `elegy-runtime`, `elegy-core`, `elegy-host-mcp` (resources + tool dispatch), `elegy-agent-events`
- Adapter/tooling crates: `elegy-adapter-fs`, `elegy-adapter-http`, `elegy-tooling`, `elegy-mermaid`, `elegy-diagram`
- Automation crates: `elegy-repo` (repo automation), `elegy-web` (web automation), `elegy-data` (data transformation), `elegy-notify` (notifications)
- Observation crates: `elegy-observe` (safe observation API), `elegy-observe-win32` (isolated Win32 FFI leaf)
- Desktop automation crates: `elegy-desktop` (safe automation API), `elegy-desktop-win32` (isolated Win32 FFI leaf)

That distinction matters for consumers: the release lane is CLI-first, while the rest of the workspace is primarily implementation and runtime support.

## Read next

- [docs/architecture/README.md](docs/architecture/README.md)
- [docs/architecture/elegy-memory-v1.md](docs/architecture/elegy-memory-v1.md)
- [docs/distribution.md](docs/distribution.md)
- [CONTRIBUTING.md](CONTRIBUTING.md)
- [Agentic Adoption Plan](docs/roadmaps/agentic-adoption-plan.md) (planned)
