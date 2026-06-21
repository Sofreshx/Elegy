# Agent Skill Bridge Mirrors (OBSOLETE)

This document is obsolete and retained for historical reference only.

The repo-local skill mirror model described here has been retired. The current
delivery model is:

- Plugin package JSON is the authority.
- Holon consumes Elegy plugins directly.
- Codex/OpenCode get generated host exports (via `elegy plugin export`) when needed.
- Repo-maintained skill mirrors under `.agents/skills/**` and `.github/skills/**`
  are removed.

The `.github/skills/**` directory has been removed.
