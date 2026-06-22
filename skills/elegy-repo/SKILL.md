---
name: elegy-repo
description: Read-only repository automation for structured git status, diff summary, branch listing, and commit log queries.
version: "2.0"
---

# Elegy Repository Automation

Read-only repository automation for structured git status, diff summary, branch listing, and commit log queries.

## Capabilities

- `repo-status`: Return a structured read-only summary of repository status, branch tracking, and changed entries.
- `repo-diff`: Return a structured read-only diff summary against HEAD or an explicit base reference.
- `repo-branches`: Return a structured read-only list of local branches, the current branch, and upstream tracking refs.
- `repo-log`: Return a structured read-only commit log with bounded commit count.
