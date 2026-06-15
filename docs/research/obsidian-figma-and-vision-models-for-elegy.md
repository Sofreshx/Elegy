# Research: Obsidian, Figma, and Vision-Model Pipelines for Elegy

This note is research-oriented guidance for contributors. It is not the canonical source for shipped behavior.

Canonical implemented truth stays in:

- `contracts/`
- `governance/`
- `rust/`
- [docs/architecture/ecosystem-topology.md](../architecture/ecosystem-topology.md)
- [docs/architecture/substrate-governance.md](../architecture/substrate-governance.md)
- [docs/architecture/mcp-skill-tooling-placement.md](../architecture/mcp-skill-tooling-placement.md)
- [docs/architecture/elegy-memory-v1.md](../architecture/elegy-memory-v1.md)
- [docs/agent-integration.md](../agent-integration.md)

## Why this note exists

Three independent but converging trends are reshaping what a serious agentic coding stack needs to ship in 2026:

1. **External memory is the biggest single productivity jump.** The strongest agentic setups (Codex, OpenCode, Claude Code, Gemini CLI, Aider, Cursor Agents) use a knowledge layer (typically Obsidian) as durable, non-canonical project memory — the agent retrieves ADRs, architecture notes, requirements, and decisions across sessions instead of re-deriving them.

2. **Design truth is the second biggest jump for frontend work.** Design tools (Figma) and design-quality skills (Impeccable) eliminate the "agent guesses from a screenshot" loop. They give the agent actual component graphs, design tokens, and an anti-slop catalog of what not to ship.

3. **Vision-capable models close the visual feedback loop.** The agent must be able to look at a running dev server, identify a layout regression or contrast bug, describe the fix, and ship it — iteratively, not as a one-shot.

This note examines how each of these three lands on Elegy specifically (the Rust toolkit), with the two real consumers — `elegy-copilot` (the instruction-engine repo) and `Holon` (the SAASTools repo) — as the test beds.

## Executive summary

The strongest direction for Elegy is to keep all three integrations **non-canonical, opt-in, and side-effect-gated**, and to expose them through the existing Skill + argv-template + `elegy-host-mcp` seam. Concretely:

- **Obsidian** becomes a non-canonical mirror of `elegy-planning` state (roadmaps, todos, review points), with a deterministic, machine-diffable Markdown format and a back-link graph that agents can traverse. Direct port of the Copilot `obsidian-synced-notes-contract.md` shape.
- **Figma** becomes a read-mostly source of design truth (components, variables, styles) plus a side-effecting write lane (comment, token export). Figma variables flow into `elegy-memory` as `decision` records so they are retrievable context.
- **Vision-capable models** sit behind a Skill (`elegy-vision`) that any agent host can call as `elegy vision describe` / `elegy vision diff` / `elegy vision ui-review`, with the model picked per-task from a curated roster. The visual feedback loop lands on top of the existing `elegy-desktop` skill for headless browser capture.
- **Impeccable** is the strongest known skill for the "design quality" leg of the loop (slop detection, 23 commands, 46 anti-patterns, CI-friendly `npx impeccable detect`). It is consumed as an external skill and referenced from `elegy-vision ui-review` as a deterministic rule set.
- **Image-gen → UI** is a two-step pipeline: GPT Image 2 (`gpt-image-2`) generates the hi-fi mock; a vision-capable model with strong UI/grounding (Claude Opus 4.7 or MiMo-V2-Omni) reads the mock and produces the actual code. Figma enters as the optional middle step when the design is durable enough to want component-graph truth, not just an image.

The two anti-patterns this note explicitly rejects:

- **Do not make Obsidian or Figma canonical.** Both stay non-canonical, opt-in, side-effect-gated. The Copilot research note `ui-runtime-overlay-research.md:247` already lists "Making Obsidian TODO output the source of truth" as an anti-pattern. Holon's hard honesty rule generalizes to "External tools do not become the durable authority by being readable."
- **Do not adopt MCP-backed memory by default.** Holon already decided against `markdownlm-mcp-memory-analysis.md` default adoption. The file-based + `elegy-memory` write-through path is the in-house pattern; Obsidian/Figma outputs write *into* it, not *alongside* it.

## The two-layer model that drives the rest of this note

The research framing that this section responds to (Obsidian = knowledge, Figma = design truth, code = implementation truth) maps cleanly onto Elegy's existing layered authority model:

```
┌───────────────────────────────────────────────────────────────┐
│  Authority sources (non-canonical, optional)                  │
│  ┌──────────────┐  ┌──────────────┐  ┌────────────────────┐   │
│  │ Obsidian     │  │ Figma        │  │ Git repo           │   │
│  │ vault        │  │ design file  │  │ (canon)            │   │
│  └──────┬───────┘  └──────┬───────┘  └─────────┬──────────┘   │
│         │ mirror          │ mirror             │              │
│         ▼                 ▼                    ▼              │
│  ┌──────────────────────────────────────────────────────────┐ │
│  │ Elegy contract substrate (canonical authority)          │ │
│  │  contracts/  governance/  schemas/  policies/            │ │
│  │  skill.*.json                              │ │
│  └──────────────────────────────────────────────────────────┘ │
│         │ argv template / subprocess                          │
│         ▼                                                     │
│  ┌──────────────────────────────────────────────────────────┐ │
│  │ Elegy runtime                                            │ │
│  │  elegy-memory  elegy-planning  elegy-mcp                 │ │
│  │  elegy-skills  elegy-host-mcp  elegy-cli                 │ │
│  │  + new: elegy-vision  elegy-figma  elegy-planning-obs.  │ │
│  └──────────────────────────────────────────────────────────┘ │
│         │ stdio MCP / JSON envelopes                          │
│         ▼                                                     │
│  ┌──────────────────────────────────────────────────────────┐ │
│  │ Host (Holon Tauri, Copilot Tauri, OpenCode, Codex, …)    │ │
│  └──────────────────────────────────────────────────────────┘ │
└───────────────────────────────────────────────────────────────┘
```

Obsidian and Figma are **non-canonical, opt-in context sources**, not authority roots. They are mirrored deterministically and exposed as skills behind the MCP host.

---

# Part 1 — Obsidian for `elegy-planning`

## What's already wired

**Elegy proper** has zero "obsidian" mentions anywhere. The repo has no Obsidian adapter.

**Elegy-Copilot (`instruction-engine`)** has the strongest Obsidian seam in the whole ecosystem:

- `docs/system/obsidian-synced-notes-contract.md` (257 lines) — Obsidian is **external, non-canonical**, pull-only, fail-closed.
- `copilot-ui/lib/obsidianSyncService.js` + `obsidianPlanningRepresentations.js` — local + remote + deterministic mirror.
- `SyncedNoteSourceProvider = 'github' | 'gitea' | 'git'` — extendable to `'obsidian-vault'` and `'figma'`.
- `INSTRUCTION_ENGINE_OBSIDIAN_*` env-var + `~/.copilot/obsidian-planning.json` config — already a host-side vault pointer contract.
- Local-tracker daemon watches `obsidianNotePaths` and emits `obsidian_note_update` loopback events.
- Generated mirror files use `ie_kind: planning-obsidian-representation` frontmatter.

**Holon (`SAASTools`)** has a research note that references the Obsidian CLI 2026-02-10 release but no implementation. Substrate is in place: bundled plugin packages, `resource-pack` asset family, `holon-specialist-harness-contract` for composition.

## Proposal: `elegy-planning` Obsidian mirror surface

Add a non-canonical mirror mode where `elegy-planning` exports **read-only** Markdown representations of any planning entity (goal, roadmap, work-point, plan, todo, issue, review-point) into a vault path. The inverse path is treated as **input-only** vault references attached as `external_refs`.

**Concretely:**

1. **New CLI subcommands** on `elegy-planning`:
   - `elegy-planning obsidian mirror --vault <path> --scope <roadmap|bullets|entity> --entity-id <id> --format json|text`
   - `elegy-planning obsidian attach --entity <id> --vault-path <path> --kind <note|adr|spec> --label <text>`
   - `elegy-planning obsidian resolve --vault-path <path> --json` — returns the entity/entities the note points to
   - `elegy-planning obsidian list --vault <path> --json` — lists all mirrors and their entity bindings
   - `mirror` and `attach` declared `hasSideEffects: true`; `resolve` and `list` are read-only

2. **New schemas** in `contracts/schemas/`:
   - `obsidian-mirror-result.schema.json`
   - `obsidian-attach-result.schema.json`
   - `obsidian-resolve-result.schema.json`
   - `obsidian-vault-pointer.schema.json` (reusing Copilot's `IE_OBSIDIAN_*` shape for cross-product compatibility)

3. **New Skill** `contracts/fixtures/skill.elegy-planning-obsidian.json` wrapping the above with argv templates.

4. **Discovery mirror** in `contracts/fixtures/skill-discovery-index.elegy-planning-obsidian.json`.

5. **Host projection**: `elegy-host-mcp` exposes it as a tool automatically. Side-effecting tools honor `--allow-side-effects`.

6. **Mirror format** (deterministic, machine-diffable, mirrors Copilot's `ie_kind:` frontmatter):

   ```markdown
   ---
   ie_kind: elegy-planning-mirror
   schema_version: elegy-planning-obsidian/v1
   entity_type: roadmap
   entity_id: <uuid>
   scope_key: default
   status: active
   source: elegy-planning
   generated_at: 2026-06-02T...
   mirror_of: "[[Roadmap: Q3 Platform]]"
   ---

   # <roadmap.title>

   > Non-canonical mirror of `elegy-planning` entity.
   > Do not edit. Edit the source via `elegy-planning roadmap update-status ...`.

   ## Sections
   ...
   ```

## What this gets you

- **Human-friendly visualization** of durable planning state inside Obsidian's graph/backlinks/canvas.
- **Wikilink roundtrip** — `[[Roadmap: Q3 Platform]]` in any vault note can be resolved to a planning entity.
- **Backlink graph** — Obsidian automatically shows which notes reference a planning entity.
- **No canonical drift** — Obsidian is non-canonical, writes are blocked; only `elegy-planning` writes.
- **Cross-project consistency** — same vault, many repos, each with scoped mirrors.
- **Friction-free read** — agents can `obsidian://` open or `vault://` resolve without an MCP roundtrip.

## Where it lives in each host

- **Elegy proper**: just the CLI surface + skill fixture; nothing host-specific.
- **Elegy-Copilot**: extends the existing `obsidianSyncService.js` and `obsidianPlanningRepresentations.js` to call `elegy-planning obsidian mirror` instead of re-implementing the format. The `SyncedNoteSourceProvider` enum gets a new `'elegy-planning'` value.
- **Holon**: bundled `elegy-planning-obsidian.holon-plugin.json` modeled on `elegy-planning.holon-plugin.json`; capability bindings of `lane: "cli"` invoking `elegy-planning obsidian ...`. UI surface in `WorkspaceChatInspector` or a new `PlanningMirrors` view.

---

# Part 2 — Figma integration

## What's already wired

**Elegy proper** has zero "figma" mentions. **Elegy-Copilot** has zero "figma" mentions. **Holon** has Figma mentioned twice, both in `docs/research/ai/research-2026-04-30--professional-software-piloting-and-chatgpt-app-patterns.md` (lines 44, 138) as a research reference, not an integration.

There is no Figma plugin, no MCP client, no design-token import path, no `figma://` URI scheme handling anywhere in any of the three repos.

## Proposal: `elegy-figma` Skill (read-side) + optional write-side

Figma has a stable REST API and an official Figma MCP server. The research framing maps to Elegy as a read-mostly source plus a small write surface.

### Read surface (no side effects)

- `elegy-figma file get --file-key <key> --depth <shallow|deep> --json`
- `elegy-figma node get --file-key <key> --node-id <id> --json`
- `elegy-figma styles get --file-key <key> --json`
- `elegy-figma variables get --file-key <key> --json` (design tokens)
- `elegy-figma components get --file-key <key> --json`
- All declared `hasSideEffects: false`, `isDeterministic: true`

### Write surface (side-effecting, blocked by default)

- `elegy-figma comment create --file-key <key> --node-id <id> --body <text>`
- `elegy-figma webhook register --file-key <key> --url <url>` (optional)
- `elegy-figma token export --file-key <key> --format <css|scss|tailwind|json>` (writes a local file; declared side-effecting)
- All declared `hasSideEffects: true`

### Implementation

Thin Rust crate `elegy-figma-adapter` (or just an `elegy-adapter-http` wrapper) that:

- Reads `FIGMA_TOKEN` from `elegy-configuration`-managed config (or OS keychain via the existing secret model)
- Maps Figma's REST responses to governed JSON in `contracts/schemas/figma-*.schema.json`
- Caches responses (Figma has rate limits)

### Design-token export → memory bridge

Optional `elegy-figma tokens to-memory --file-key <key> --scope <workspace>` writes Figma variables to `elegy-memory` as `decision` records (provenance: `imported`). This is the **most valuable concrete win for Elegy** — Figma becomes a knowledge source for the agent, retrievable via `elegy-memory search`.

## What this gets you

- **Design truth becomes retrievable context** — `search-workspace-memory` returns design tokens.
- **Eliminates design-guessing** in agent code generation (the whole point of Figma MCP per the research).
- **Figma file becomes a versioned contract** — same file key, different agents, same tokens.
- **Holon's `resource-pack` family** is the natural sink; design tokens as Tier 0 inert assets.
- **Holon's `ContextRetrieval` trait (planned RM-025)** has a ready-made first non-memory source.

## Where it lives in each host

- **Elegy-Copilot**: new `SyncedNoteSourceProvider` value `'figma'`; `syncedNoteSourceFigmaRoutes.ts` peer of `syncedNoteSourceHttpRoutes.ts`; panel in `copilot-ui/ui/src/tabs/Planning/` (e.g. `FigmaDesignContextPanel.tsx`) mirroring `ObsidianNotesPanel.tsx`.
- **Holon**: bundled `figma-bridge.holon-plugin.json`; design tokens as `resource-pack`; Figma files as `adapter-binding` with `lane: "api"`; secret resolved via `desktop_runtime_secret_upsert`.
- **Elegy proper**: just the Skill fixture + a small adapter; the host projection is automatic.

---

# Part 3 — Vision-capable models for a visual feedback loop

## Why a visual feedback loop is the third leg

The first two legs (Obsidian memory, Figma truth) get the agent the *context* it needs. The third leg is the agent's ability to **see** what it shipped and **iterate** without a human in the loop. This is the work that needs vision-capable models and a UI-quality rule set.

The use case the user asked about — "ensure we have an agent that can do visual feedback loop and work on UI issues iteratively" — requires four capabilities in one agent loop:

1. **Capture**: take a screenshot of the running dev server at a known state.
2. **Describe**: the model reads the screenshot, identifies the issue (contrast, alignment, spacing, broken component, regression).
3. **Critique**: the model runs the description through a deterministic anti-slop rule set (e.g. Impeccable) to make sure the fix doesn't ship a new AI-tell.
4. **Edit**: the model proposes a code change; the host validates, applies, re-captures, and loops.

The first three are the visual feedback loop. The fourth is already in Elegy-Copilot and Holon.

## Vision-capable model roster (2026-06)

This is the curated set of vision-capable models that could back `elegy-vision`. It is intentionally narrow — the list is the result of a `elegy` provider-style audit, not a comprehensive market survey.

| Model | Provider | Vision? | Image-gen? | UI/grounding strength | Cost (per 1M tok, in/out) | Context | Self-hostable? | Notes |
|---|---|---|---|---|---|---|---|---|
| **Claude Opus 4.7** | Anthropic | Yes (3.75 MP) | No | Strongest visual acuity for diagrams / high-res UI | $5 / $25 | 1M | No | Best for "look at this complex dashboard and tell me what's wrong" |
| **Claude Sonnet 4.6** | Anthropic | Yes | No | Strong; cheaper than Opus 4.7 | $3 / $15 | 200K | No | Good default if Opus is overkill |
| **GPT-4o / GPT-5.4** | OpenAI | Yes (2.1 MP) | via gpt-image-2 | Strong on description + structured output | $2.50 / $10 | 128K–1M | No | Good for "describe and emit JSON" workflows |
| **Gemini 3 Pro** | Google | Yes (3.1 MP) | Yes (Imagen) | Strongest multimodal (MMMU-Pro 95); native video | $1.25 / $5 | 2M | No | Best price/perf for multimodal at scale |
| **Gemini 2.5 Flash** | Google | Yes | Yes | Cheapest frontier vision | $0.10 / $0.40 | 1M | No | Default for high-volume visual sweeps |
| **MiMo-V2-Omni** | Xiaomi | Yes (omni: image+video+audio) | No | GUI agentic on par with Opus 4.6 / GPT-5.2; audio > Gemini 3 | (open weights) | 256K | Yes (FP8) | Only open omni model; best for self-host |
| **MiMo-V2.5** | Xiaomi | Yes (omni) | No | MMMU-Pro 76.8, matches Gemini 3 on video, Claude Sonnet 4.6 on agentic | (open weights) | 1M | Yes (FP8) | Larger 1M context; great for "screenshot the whole flow" |
| **DeepSeek V4 Vision** | DeepSeek | Yes (mounted) | No | OCR + UI + screenshot; 10× cheaper than Claude (90 vs 870 KV cache entries) | ~10× cheaper than Claude | large (MoE) | Yes (open weights) | Best for self-host, cost-sensitive visual sweeps |
| **DeepSeek-VL2** | DeepSeek | Yes (MoE VLM) | No | Strong on OCR + chart + grounding | open-weights pricing | 4K (small) | Yes | Mature; good for embeddings-style visual indexing |
| **DeepSeek Janus-Pro** | DeepSeek | Yes + image gen | Yes | Unified understanding + generation | open-weights pricing | standard | Yes | Only model that does both vision-understanding and image-gen with open weights |
| **MiniMax M-series** | (curated) | Yes (M2/M3 multimodal) | No | Per Holon `provider_catalog.rs` | curated | standard | No | Already wired into Holon's `summary-scout` lane |
| **gpt-image-2** | OpenAI | N/A (gen) | Yes (1K/2K/4K) | UI mockups, text-in-image, multilingual CJK | $0.04–0.17/image | 16 reference images | No | The image-gen model of record for 2026 |
| **Nano Banana Pro** | (Google) | N/A (gen) | Yes | Photoreal portraits, faster than gpt-image-2 | (Google) | n/a | No | Best pure-photoreal alt |
| **Seedream 5** | (ByteDance) | N/A (gen) | Yes | Web-search-grounded stylised editorial | n/a | n/a | No | Best for stylised brand work |

### Per-task recommendation (the routing table that `elegy-vision` should encode)

| Task | First choice | Second choice | Why |
|---|---|---|---|
| UI screenshot → structured critique | Claude Opus 4.7 | MiMo-V2-Omni | Opus wins on visual acuity (3.75 MP, 98.5% computer-use); MiMo is the only open omni |
| Bulk screenshot diffing / regression sweeps | Gemini 2.5 Flash | DeepSeek V4 Vision | Cheapest frontier vision; 10× cost advantage at volume |
| Diagram / architecture analysis | Claude Opus 4.7 | Claude Sonnet 4.6 | Claude leads structured-content reasoning |
| UI mockup generation (image-gen) | gpt-image-2 (Thinking mode) | Midjourney v7 (artistic) | gpt-image-2 is the only model that renders UI text reliably |
| Multilingual CJK signage in mockups | gpt-image-2 | Ideogram 3 | Best CJK text rendering in 2026 |
| Self-host visual loop (no API egress) | MiMo-V2.5 | DeepSeek V4 Vision | Both open-weights, FP8; MiMo has 1M context |
| Cross-image comparison (before/after) | Gemini 3 Pro | MiMo-V2-Omni | Gemini supports up to 3,600 images in one context |
| "Is this AI slop?" audit | Impeccable (`npx impeccable detect`) | n/a | Deterministic, no LLM, 41 rules — never trust an LLM for this |
| High-resolution dashboard screenshot | Claude Opus 4.7 | GPT-5.4 | Opus 3.75 MP is the highest of the frontier |
| Long-running visual stream (>10h audio+video) | MiMo-V2-Omni | n/a | Only model that handles 10h+ continuous audio-video |

### The model-routing pattern that Holon already validates

Holon's RM-016/017/019–025 roadmap and its existing `provider_catalog.rs` already encode the right shape: a curated roster with a default model per task, plus the "use multiple models routed by task" pattern. Per the public research, "Model routing sends simple queries to cheaper models like Gemini Flash or GPT-4o-mini, reserving more expensive models like Claude Sonnet or GPT-4o for complex tasks. This can reduce inference costs by 40–70% with minimal quality impact on the simple queries."

`elegy-vision` should adopt that pattern as the **default**:

- Cheap model (Gemini Flash or DeepSeek V4 Vision) does the first pass: capture, describe, list.
- Frontier model (Claude Opus 4.7 or MiMo-V2-Omni) does the deep critique on items the cheap model flagged as "uncertain" or "complex."
- Deterministic rules (Impeccable) run after both passes to enforce the "no AI slop" floor.

## Proposal: `elegy-vision` Skill

A thin Skill that exposes the visual feedback loop as 5 capabilities, all argv-templated and JSON-emitting:

1. `elegy vision describe --image <path-or-url> --prompt <text> --model <auto|opus-4.7|mimo-v2-omni|...> --json`
   - Returns structured description (objects, regions, text, issues)
2. `elegy vision diff --image-a <path> --image-b <path> --threshold <0-1> --json`
   - Returns a delta list (region, kind, severity, suggested fix)
3. `elegy vision ui-review --screenshot <path> --target <url-or-component> --ruleset <impeccable|...> --json`
   - Combines model critique with the Impeccable rule set
4. `elegy vision generate --prompt <text> --target <ui-mockup|poster|icon|chart> --model <gpt-image-2|...> --output <path>`
   - Image-gen → UI mockup, declared `hasSideEffects: true` (writes a file)
5. `elegy vision mockup-to-code --image <path> --framework <react|astro|swiftui> --tokens <elegy-memory-scope> --output <path>`
   - Vision → code, declared `hasSideEffects: true`

All capabilities declared with `isDeterministic: false` and the correct `hasSideEffects` flag. The skill fixture references a tiny Rust crate `elegy-vision` that:

- Reads `ELEGY_VISION_DEFAULT_MODEL` from config (defaults to the model the host profile pins)
- Falls back through the cost-optimized routing table above
- Emits governed JSON matching `vision-result.schema.json`
- Optionally chains into `elegy-figma` (for designs that exist as Figma) and `elegy-memory add` (to record the critique as a `decision` / `observation`)

## Why Impeccable is the right deterministic anchor

`impeccable.style` (open-sourced by Paul Bakaus, 31k stars on GitHub) is the strongest known skill for the "design quality" leg of the loop. It is not a vision model — it is a deterministic rule set + a curated catalog of 23 commands + a 46-pattern anti-slop catalog.

The 46 anti-patterns (from the `impeccable.style/slop` page) cluster into:

- **Visual Details (7)**: rounded-card border accent, glassmorphism, side-tab border, hairline + wide shadow, repeating-gradient stripes, extreme border-radius, amateurish SVG.
- **Typography (10)**: flat hierarchy, icon-tile-above-heading, italic serif display, eyebrow pill chip, repeated kicker labels, oversized hero, crushed letter spacing, overused font (Inter / Geist / Space Grotesk / Instrument Serif), single font for everything, all-caps body.
- **Color & Contrast (5)**: AI color palette (purple/violet), dark mode with glowing accents, gradient text, gray on colored, cream/beige.
- **Layout & Space (8)**: hero metric layout, identical card grids, monotonous spacing, nested cards, 01/02/03 numbered markers, line length too long, content overflowing container, clipped positioned child.
- **Motion (3)**: bounce/elastic easing, layout property animation, image hover transform.
- **Copy (4)**: em-dash overuse, marketing buzzword, aphoristic cadence, "theater" framing.
- **Imagery (1)**: broken/placeholder image.
- **General quality (8)**: cramped padding, body text touching viewport edge, justified text, low contrast, skipped heading level, tight line height, tiny body text, wide letter spacing.

41 of those 46 are detectable by deterministic CLI rules (`npx impeccable detect`); 4 are opt-in provider tells (`--gpt`, `--gemini`); 5 need the LLM critique pass (`/impeccable critique`).

The right integration shape is:

- **`elegy vision ui-review`** calls the LLM pass first, then runs `npx impeccable detect` on the rendered page, then merges findings.
- **`elegy-mcp`** does **not** wrap Impeccable; it is a separate, install-time skill that lives under each host's asset catalog (Copilot already has the `npx impeccable skills install` path, Holon can add an `impeccable.holon-plugin.json` of `lane: "cli"` invoking the same command).
- The 46 anti-patterns become a governed `impeccable-ruleset/v1` schema that `elegy-vision` references, so the rule set can be versioned and pinned per host.

## Why GPT Image 2 is the right image-gen anchor

OpenAI `gpt-image-2` (shipped April 21, 2026, `chatgpt-image-latest` alias in ChatGPT) is the first image model that meets the "UI mockup" bar: pixel-perfect text rendering inside images (English + CJK), 1K/2K/4K output, 16-reference image editing, "Thinking" mode for complex layouts, and aspect ratios from 3:1 to 1:3.

The three headline wins that matter for the UI loop:

1. **Pixel-perfect text rendering** — signs, labels, UI copy, infographic text render legibly. This is the historic failure of DALL-E 3, Midjourney v6, GPT Image 1, and Stable Diffusion 3.
2. **Photorealism at 2K** — neutral color cast (no more warm yellow), accurate lighting, materials, depth of field.
3. **UI mockup generation** — screenshot-style prompts with exact menus, button labels, tab structures, and spacing rules now produce believable outputs. The 16:9 aspect ratio is recommended for desktop layouts; the prompt should specify exact text in quotes and explicitly forbid extra random text.

The next-best image-gen alternatives:

- **Midjourney v7/v8** — still the reference for stylised artistic output, but trails on text accuracy and instruction following.
- **Nano Banana Pro** — slightly stronger and cheaper for photoreal portraits and pure batch work; not as good for UI text.
- **Seedream 5** — better for web-search-grounded editorial illustration.
- **DeepSeek Janus-Pro** — the only open-weights model that does both understanding and generation, but not as strong on UI text as gpt-image-2.

The pipeline the user asked about — "image gen to UI, then have an agent that is specialized with vision into turning them into real UI" — maps directly onto the Impeccable + gpt-image-2 + `elegy-vision` stack:

1. **Generate the mock** with `gpt-image-2` (Thinking mode for complex layouts). Output: a 16:9 hi-fi mock with pixel-perfect text.
2. **Critique the mock** with `elegy vision describe` (MiMo-V2-Omni or Claude Opus 4.7). Output: structured description of regions, components, and any obvious anti-patterns.
3. **Plan the implementation** by writing the mock's components into `elegy-planning` as todos (e.g. `build Hero`, `build PricingCard`, `use Inter replacement`). This is the "do not vibe-code" guardrail.
4. **Implement** with the host's coding agent (Codex, Claude Code, OpenCode, Copilot) running the existing `elegy-planning` workflow. The vision model is **not** the one writing the code; it is the verifier.
5. **Verify** by capturing the running dev server, running `elegy vision ui-review` (which includes the Impeccable rule set), and iterating until deterministic rules pass and the LLM critique is clean.
6. **Persist** the design decisions to `elegy-memory` (typography, palette, spacing) so the next session inherits them.

That is the visual feedback loop. Figma enters the picture **only if** the design is durable enough to want component-graph truth — most prototypes never get there. For prototypes, the gpt-image-2 mock + `elegy-memory` decision record is enough.

## Where Figma actually earns its place

The honest answer: most agentic UI work does not need Figma. Figma earns its place when **at least one of the following is true**:

- The component has to round-trip with a human designer (Figma is the designer's tool of record).
- The token set is the durable contract (Figma Variables → CSS variables / Tailwind config).
- The design system already lives in Figma and 3+ teams depend on it.
- The design has 50+ components and the agent needs the actual node graph, not an image.

For everything else, the gpt-image-2 mock + `elegy-memory` decision record is cheaper, faster, and integrates with the existing planning authority.

When Figma is in play, the integration is:

1. `elegy-figma variables get` → tokens
2. `elegy-figma tokens to-memory --scope workspace --provenance imported` → memory records
3. `elegy-figma components get` → component graph (optional, for the cases where the agent needs it)
4. The agent's working set is `elegy-memory search` → tokens + decisions + planning todos
5. The vision loop is still gpt-image-2 → mock → `elegy-vision ui-review` → ship

---

# Part 4 — Concrete integration plan, ranked

## Priority order

| # | Item | Repo | Effort | Value |
|---|---|---|---|---|
| 1 | `elegy-planning obsidian mirror/attach/resolve/list` (4 subcommands + 4 schemas + 1 skill fixture) | Elegy | M | High — gives every Elegy host Obsidian as a non-canonical mirror |
| 2 | `elegy-vision` (5 subcommands + 1 schema + 1 skill fixture, thin crate) | Elegy | M | High — visual feedback loop, works for every host |
| 3 | `elegy-figma` read-side (5 subcommands + 5 schemas + 1 skill fixture, thin adapter) | Elegy | M | High — Figma becomes a memory source |
| 4 | `elegy-figma tokens to-memory` bridge (writes variables to `elegy-memory` as `decision` records) | Elegy | S | High — closes the design→memory loop |
| 5 | Holon `figma-bridge.holon-plugin.json` + design-token `resource-pack` + Impeccable plugin | Holon | M | High — first concrete `ContextRetrieval` source |
| 6 | Copilot `SyncedNoteSourceProvider: 'figma'` + `FigmaDesignContextPanel.tsx` | Copilot | M | High — parity with Obsidian panel |
| 7 | Copilot `elegy vision ui-review` exposed in the Planning / Runtime tab | Copilot | M | High — the visual loop the user is asking about |
| 8 | `elegy-vision` Image-gen leg (gpt-image-2 + mockup-to-code) | Elegy | M | High — closes the image-gen → UI pipeline |
| 9 | Holon `obsidian-bridge.holon-plugin.json` + workspace-surface extension | Holon | M | Medium — vault pointer for Holon |
| 10 | Impeccable `impeccable.holon-plugin.json` + UI surface in `SkillEditor` | Holon | S | High — the deterministic anti-slop floor |
| 11 | Copilot extends `obsidianPlanningRepresentations.js` to call `elegy-planning obsidian mirror` | Copilot | S | High — unifies formats, removes dup |
| 12 | `elegy-figma` write-side (comment, token export) | Elegy | M | Medium — useful but side-effecting |
| 13 | `elegy-figma` webhook integration for design-file change events | Elegy | L | Low — nice-to-have for live sync |
| 14 | Knowledge-graph reasoning (graph retrieval over vault links) | Cross-cutting | XL | Deferred — Holon research note already recommends bounded seed-plus-expansion, not full KG |

## Suggested first slice

If you want a single concrete starting point, I'd suggest the **Obsidian mirror for `elegy-planning`** (item #1) because:

- It reuses Copilot's already-proven contract shape (`docs/system/obsidian-synced-notes-contract.md` + `ie_kind: planning-obsidian-representation`).
- It does not require any new authority root — pure non-canonical mirror.
- It gives every host (Elegy, Copilot, Holon, any MCP-aware agent) the same external-memory surface for planning state.
- It is a single Skill fixture + 4 subcommands + 4 schemas — small enough to land in one PR.
- It directly serves the highest-value use case (Obsidian = persistent project memory) and is more durable than MCP alternatives because it writes through the planning authority, not around it.

## Suggested second slice (the user's actual ask)

The user explicitly asked about a vision-capable model that can "do visual feedback loop and work on UI issues iteratively." The right second slice is **`elegy-vision`** (item #2) plus the Impeccable plugin (item #10), because:

- It directly serves the requested capability (vision-based UI review loop).
- It composes cleanly with the existing `elegy-desktop` skill for headless browser capture.
- It is the right place to pin the curated model roster and the routing table.
- The Impeccable rule set is the only deterministic way to enforce "no AI slop" on the output.
- gpt-image-2 + Claude Opus 4.7 + MiMo-V2-Omni together cover the full image-gen → critique → iterate loop.

The slice lands as:

- `rust/crates/elegy-vision/` — thin crate, `elegy-vision describe|diff|ui-review|generate|mockup-to-code`
- `contracts/fixtures/skill.elegy-vision.json` — argv templates
- `contracts/schemas/vision-result.schema.json`
- `contracts/fixtures/impeccable-ruleset-v1.json` — the 41 deterministic + 4 opt-in + 5 LLM-only rules
- Holon: `elegy-vision.holon-plugin.json` + `impeccable.holon-plugin.json`
- Copilot: `elegn-vision` skill mirror + UI tab in the Planning surface
- `docs/specs/elegy-vision.md` — the durable spec
- One ADR capturing the model-routing decision

## Suggested third slice (Figma)

The third slice is **`elegy-figma` read-side + tokens to memory** (items #3 + #4), because:

- It directly serves the Figma-as-design-truth framing.
- It produces the highest-leverage memory writes (design tokens, used by every frontend agent in every future session).
- It is the same argv-template pattern as `elegy-planning obsidian`, so the implementation effort is mostly schema design.

---

# Part 5 — The two things to NOT do

1. **Do not make Obsidian or Figma canonical.** Both must stay non-canonical, opt-in, side-effect-gated. The Copilot research note `ui-runtime-overlay-research.md:247` explicitly lists "Making Obsidian TODO output the source of truth" as an anti-pattern. The Holon hard honesty rule ("Provider models do not become the execution authority by producing text or tool-call intent") generalizes to "External tools do not become the durable authority by being readable."

2. **Do not add MCP-backed memory or RAG for Obsidian/Figma.** Holon already decided against `markdownlm-mcp-memory-analysis.md` default adoption. The file-based + `elegy-memory` write-through path is the in-house pattern; Obsidian/Figma outputs write *into* it, not *alongside* it.

3. **Do not pin a single vision model.** The model landscape shifts too fast (DeepSeek V4 vision dropped in May 2026, MiMo-V2-Omni in March 2026, gpt-image-2 in April 2026, Opus 4.7 in April 2026). Pin the *capability* (visual acuity, context length, cost ceiling), not the model. The `provider_catalog` pattern Holon already uses is the right shape.

4. **Do not let the vision model write code.** The vision model's job is critique and verification. The coding agent's job is implementation. Mixing them re-creates the "agent vibes the answer" failure mode.

5. **Do not adopt Impeccable as the only design-quality check.** Impeccable is the strongest deterministic floor (46 anti-patterns, 41 CLI-runnable), but the user's specific brief is "image gen to UI" — that needs the LLM critique pass *and* a check that the gpt-image-2 mock matches the user's PRODUCT.md / DESIGN.md. The LLM pass is the only one that can read the brief and the mock together.

---

# Sources

## Obsidian + Figma framing

- The user's research framing: "Obsidian = design context / knowledge/memory/context management; Figma = design context" — the "external memory" / "design truth" thesis, validated by:
  - Figma's own MCP announcement: MCP provides design information directly to coding agents so generated code matches the design system more accurately.
  - The OpenClaw integration (Xiaomi MiMo-V2-Omni): the model sees, decides, and acts; the framework executes — a clean separation of perception from action.
  - Impeccable's design language: 23 commands, 46 anti-patterns, 41 deterministic detectors, "tune interfaces while they run" loop.

## Vision models

- DeepSeek V4 Vision Mode (April 2026) — DeepSeek-international guide; MindStudio analysis (KV cache 90 vs 870, 10× cost advantage); Sitepoint preview; HowAIWorks "Now, We See You" teaser.
- DeepSeek-VL2, Janus, JanusFlow, Janus-Pro, DeepSeek-OCR 2 — Roboflow blog; GitHub `deepseek-ai/DeepSeek-VL2`; Roboflow catalog.
- MiMo-V2-Omni / MiMo-V2.5 (March–April 2026) — Xiaomi MiMo product pages; the-decoder.com launch coverage; GitHub `XiaomiMiMo/MiMo-VL`; MiMo-VL-Miloco arXiv.
- Claude Opus 4.7 (April 16, 2026) — techbytes.app comparison; hostagentes.com / Paperclip benchmarks.
- Claude Sonnet 4.6 / Opus 4.6 — Kovil AI, linos.ai, benchlm.ai, brightcolumn.com.
- GPT-4o / GPT-5.4 — uatgpt.com multimodal comparison; Kovil AI; linos.ai; benchlm.ai.
- Gemini 2.5 Pro / Flash, Gemini 3 Pro, Gemini 3.1 Pro — Kovil AI; linos.ai; benchlm.ai; techbytes.app.
- MiniMax — already referenced in Holon `provider_catalog.rs:25-225` as a curated provider.

## Image-gen

- gpt-image-2 / ChatGPT Images 2.0 (April 21, 2026) — nivaalabs.com review; createvision.ai complete guide; buildfastwithai.com developer breakdown; cometapi.com comparison; mindstudio.ai; ducttape3.org "GPT Image 2 vs 1.5".
- Midjourney v6/v7 — referenced in uatgpt.com, linos.ai, mindstudio.ai.
- Ideogram 3 — crazyrouter.com.
- DeepSeek Janus-Pro — roboblog.
- Nano Banana Pro — createvision.ai.

## Impeccable

- impeccable.style home, /designing, /slop — full site scrape (2026-06-02).
- pbakaus/impeccable GitHub repo (31k stars).
- Anthropic frontend-design skill — flagged as unmaintained by Impeccable's own docs.

---

## Draft proposals (not committed)

These are the concrete artifacts to draft next, sketched here for review before they are written as schemas / specs / skills. None of this is committed yet.

### Draft 1 — `elegy-vision` Skill (5 capabilities)

All argv-templated, JSON-emitting. Thin Rust crate `elegy-vision` that reads `ELEGY_VISION_DEFAULT_MODEL` from config (defaults to the model the host profile pins) and falls back through the cost-optimized routing table.

| Capability | Side effects | Deterministic | Notes |
|---|---|---|---|
| `elegy vision describe --image <path-or-url> --prompt <text> --model <auto\|opus-4.7\|mimo-v2-omni\|...> --json` | false | false | Returns structured description (objects, regions, text, issues) |
| `elegy vision diff --image-a <path> --image-b <path> --threshold <0-1> --json` | false | false | Returns delta list (region, kind, severity, suggested fix) |
| `elegy vision ui-review --screenshot <path> --target <url-or-component> --ruleset <impeccable\|...> --json` | false | false | Combines LLM critique with Impeccable rule set |
| `elegy vision generate --prompt <text> --target <ui-mockup\|poster\|icon\|chart> --model <gpt-image-2\|...> --output <path>` | true (file write) | false | Image-gen → UI mockup |
| `elegy vision mockup-to-code --image <path> --framework <react\|astro\|swiftui> --tokens <elegy-memory-scope> --output <path>` | true (file write) | false | Vision → code; chained from `elegy vision generate` |

Schema name: `vision-result.schema.json` (single result envelope; the `kind` discriminator selects description | diff | ui-review | generate | mockup-to-code).

### Draft 2 — `impeccable-ruleset/v1` schema

A governed JSON snapshot of the 46 anti-patterns (41 deterministic CLI, 4 opt-in provider tells behind `--gpt` / `--gemini`, 5 LLM-only). Pins the rule IDs, severities, and detectors so the rule set is versioned and host-portable. Lives at `contracts/fixtures/impeccable-ruleset-v1.json`; the schema at `contracts/schemas/impeccable-ruleset.schema.json`.

**Rule set structure (sketch):**

```json
{
  "schemaVersion": "impeccable-ruleset/v1",
  "source": "pbakaus/impeccable",
  "sourceVersion": "<pinned>",
  "rules": [
    {
      "id": "rounded-card-border-accent",
      "category": "visual-details",
      "severity": "ai-slop",
      "detection": "cli",
      "description": "Thick colored border clashes with the radius on a rounded card."
    }
  ]
}
```

Categories (per Impeccable's own catalog): `visual-details` (7), `typography` (10), `color-contrast` (5), `layout-space` (8), `motion` (3), `copy` (4), `imagery` (1), `general-quality` (8). Total: 46.

**Integration shape (sketch):**

- `elegy vision ui-review` calls the LLM pass first, then runs `npx impeccable detect` on the rendered page, then merges findings.
- `elegy-mcp` does **not** wrap Impeccable directly. Impeccable is consumed as a separate, install-time skill that lives under each host's asset catalog:
  - Copilot: `npx impeccable skills install` (already a documented install path).
  - Holon: `impeccable.holon-plugin.json` with `lane: "cli"` invoking the same command.
- The 46 anti-patterns become a governed `impeccable-ruleset/v1` schema that `elegy-vision` references, so the rule set can be versioned and pinned per host.

### Draft 3 — `elegy-vision` model-routing table (governed artifact)

The per-task routing table from Part 3, encoded as a governed fixture (`contracts/fixtures/vision-model-routing-v1.json`) so a host can ship a different default model without changing code. Pin the *capability* (visual acuity, context length, cost ceiling), not the model.

```json
{
  "schemaVersion": "vision-model-routing/v1",
  "defaultModel": "gemini-2.5-flash",
  "routing": [
    {
      "task": "ui-screenshot-critique",
      "firstChoice": "claude-opus-4.7",
      "secondChoice": "mimo-v2-omni",
      "rationale": "Opus wins on visual acuity (3.75 MP, 98.5% computer-use); MiMo is the only open omni"
    }
  ]
}
```

This is the right shape for Holon's existing `provider_catalog.rs` pattern: a curated roster with a default model per task, plus the "use multiple models routed by task" pattern that has been shown to reduce inference costs by 40–70% with minimal quality impact.

### Draft 4 — Image-gen → UI pipeline (the "do not vibe-code" guardrail)

The full loop, expressed as a 6-step sequence that any host agent can execute:

1. **Generate the mock** with `gpt-image-2` (Thinking mode for complex layouts). Output: a 16:9 hi-fi mock with pixel-perfect text.
2. **Critique the mock** with `elegy vision describe` (MiMo-V2-Omni or Claude Opus 4.7). Output: structured description of regions, components, and any obvious anti-patterns.
3. **Plan the implementation** by writing the mock's components into `elegy-planning` as todos (e.g. `build Hero`, `build PricingCard`, `use Inter replacement`). This is the "do not vibe-code" guardrail.
4. **Implement** with the host's coding agent (Codex, Claude Code, OpenCode, Copilot) running the existing `elegy-planning` workflow. The vision model is **not** the one writing the code; it is the verifier.
5. **Verify** by capturing the running dev server, running `elegy vision ui-review` (which includes the Impeccable rule set), and iterating until deterministic rules pass and the LLM critique is clean.
6. **Persist** the design decisions to `elegy-memory` (typography, palette, spacing) so the next session inherits them.

### Draft 5 — Companion deliverable list (if/when this is committed)

If Part 4's "second slice" lands, the concrete deliverables are:

- `rust/crates/elegy-vision/` — thin crate, `elegy-vision describe|diff|ui-review|generate|mockup-to-code`
- `contracts/fixtures/skill.elegy-vision.json` — argv templates
- `contracts/schemas/vision-result.schema.json`
- `contracts/fixtures/impeccable-ruleset-v1.json` — the 41 deterministic + 4 opt-in + 5 LLM-only rules
- `contracts/fixtures/vision-model-routing-v1.json` — the governed routing table
- Holon: `elegy-vision.holon-plugin.json` + `impeccable.holon-plugin.json`
- Copilot: `elegy-vision` skill mirror + UI tab in the Planning surface
- `docs/specs/elegy-vision.md` — the durable spec
- One ADR capturing the model-routing decision
