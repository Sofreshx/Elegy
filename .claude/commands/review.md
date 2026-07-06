Review the changes on the current branch against `dev`:

1. Run `git diff dev --stat`, then inspect `git diff dev`.
2. For each changed file, verify coherence with the smallest relevant architecture doc, ADR, spec, or AGENTS.md file.
3. Look for touched structural invariants, contract drift, stale generated/derived surfaces, production `unwrap` or `expect`, untested edge cases, and files changed outside scope.
4. For agent-facing changes, check the governed artifact plus the Rust behavior or projection that exposes it.
5. List findings first, ordered by severity: critical, important, minor.

Focus: $ARGUMENTS
