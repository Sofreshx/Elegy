# Anti-Patterns

Each entry names an anti-pattern, explains why it fails, and points to the
canonical replacement. Use this as the lookup table when writing audit
findings.

## AP-1: Skill is just a command list

**Pattern.** The SKILL.md body is "Here are the commands." with a code
block listing the CLI surface and no other content.

**Why it fails.** The agent already has the commands from
`elegy-skills get --skill-id <id> --json` or the fixture's
`capabilities[].implementation.arguments`. The skill adds no value
beyond what the registry already exposes. Agents that load the skill
still do not know which command to call first, when to call it, or
what to do when it fails.

**Replacement.** Add Quick start, Tool-call guardrails, Common issues,
and Examples. The command list stays in the Capability index and
Examples, but the body explains the *order* and the *failure modes*.

## AP-2: Skill is just an install page

**Pattern.** The body is "Run `./install.ps1` and then call
`elegy-X list`."

**Why it fails.** Mirror lanes (`.agents/skills/`, `.github/skills/`,
`src/<surface>/skills/`) end up as install pages because the wrapper
README is right next to them. Agents that load the skill from a mirror
get install steps and nothing else.

**Replacement.** Move install instructions to `references/install.md`
or a per-surface README. Keep the SKILL.md body focused on operations.

## AP-3: "When to use" is the bulk of the body

**Pattern.** Three paragraphs explaining when to load the skill, when
not to load it, and what other skills to consider — and then a thin
Quick start.

**Why it fails.** The agent already has trigger text in the frontmatter
`description`. The "when to use" decision is made before the body is
read. The body should be about *how* to use the skill, not *whether* to.

**Replacement.** Keep one short paragraph or a "Boundaries" section
that names the non-obvious cases. The bulk of the body should be
operationally useful: guardrails, workflow, common issues, examples.

## AP-4: No fetch-before-mutate

**Pattern.** A mutating capability is described without saying which
read must happen first.

**Why it fails.** The agent will call the mutation with stale context
and either fail or, worse, succeed against a target the user did not
mean. Examples: edit a page without first fetching it; update a
config without first showing it; delete a note without first reading
it.

**Replacement.** In the Tool-call guardrails for the mutation family,
write: "Always run `<read-capability>` first against the same
`<selector>` and use its result as the input to the mutation. Do not
construct mutation input from cached state."

## AP-5: No Common issues

**Pattern.** Common issues is empty, contains a single generic row, or
is missing entirely.

**Why it fails.** The Common issues table is the highest-value section
for an agent mid-task. It is where the author encodes "I tried this,
here is what actually broke." Without it, the agent re-discovers the
same failure on every run.

**Replacement.** Mine recent reviews, support threads, and bug reports
for 3+ real failure modes. The first row should be the failure the
author hit most recently.

## AP-6: "Expected output depends on your environment"

**Pattern.** Examples say "run the command, the output should look
something like X" or "varies by environment."

**Why it fails.** Examples that do not commit to a literal output are
useless for verification. The agent cannot diff against a fuzzy target,
and the author did not actually run the command.

**Replacement.** Run the command locally during authoring. Capture the
literal stdout. Paste it. If the command cannot be run in the current
environment, that is a Critical finding — the example must be marked
"unverified" and a verification task added to the change.

## AP-7: Description is a marketing paragraph

**Pattern.** Frontmatter `description` is three sentences about how
the skill is "powerful", "comprehensive", or "designed to help."

**Why it fails.** The description is the trigger text. Agents match on
verb + object. Marketing copy does not contain either. The skill will
be skipped for queries it should match, and matched for queries it
should not.

**Replacement.** One sentence, ≤ 200 characters, starts with "Use
when", names the verb, names the object. Example: "Use when an agent
needs to create, read, search, patch, or toggle tasks inside the
user's local Obsidian vault through the official Obsidian CLI."

## AP-8: Capability index rows do not match the fixture

**Pattern.** The Capability index lists 8 capabilities but the
governed fixture declares 10. Or the side-effect class in the table
disagrees with `execution.hasSideEffects` in the fixture.

**Why it fails.** The agent trusts the table over the fixture because
the table is human-readable. A disagreement makes the table the lie.

**Replacement.** Regenerate the table from the fixture on every
fixture change. The table cells are projections, not source of truth.

## AP-9: "Do not" anti-patterns are stylistic preferences

**Pattern.** Tool-call guardrails contain "Do not write in passive
voice" or "Do not use emojis in examples."

**Why it fails.** Anti-patterns must encode real failure modes, not
house style. A list of style preferences trains the agent to ignore
the section.

**Replacement.** Reserve "Do not" for foot-guns the agent will
actually hit: wrong argument shape, invented commands, fetch-after-
mutate, missing scope, wrong default path. Style is the formatter's
job, not the skill's.

## AP-10: Mirror lanes drift

**Pattern.** The `.github/skills/<name>/SKILL.md` and
`.agents/skills/<name>/SKILL.md` bodies are older, thinner, or
inconsistent with the canonical SKILL.md.

**Why it fails.** The mirror rule in
`docs/architecture/agent-skill-bridge-mirrors.md` requires the
mirrors to be a rendered view of one canonical source. Drift means
agents loading from different lanes see different content.

**Replacement.** Pick one lane as the rendered target (today:
`.github/skills/`) and regenerate the others from it. The future
`elegy skill audit` will fail the build on mirror drift.
