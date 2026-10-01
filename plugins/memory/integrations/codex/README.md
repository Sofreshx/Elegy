---
title: Codex contextual recall adapter
status: active
owner: Elegy Memory
doc_kind: guide
---

# Codex contextual recall adapter

This directory contains a thin, opt-in Python standard-library adapter for a
Codex `UserPromptSubmit` hook. It invokes the existing `elegy-memory`
`contextual-recall` CLI and accepts an injected context envelope only after
checking its schema, mode, status, and byte bound. It fails open: malformed
events, unsafe paths, child failures, oversized output, and timeouts produce
`{}` and do not block the host. The adapter is not a new plugin framework and
does not install itself.

## Configuration and command

Create an operator-approved config from the strict
[`contextual-recall-config.schema.json`](../../schemas/contextual-recall-config.schema.json).
Use absolute paths and start with the safe disabled fixture
[`contextual-recall-config.disabled.json`](../../fixtures/contextual-recall-config.disabled.json).
Set `mode` to `observe` or `inject` only after reviewing the source database,
domain, scopes, sensitivity bound, and separate journal path.

The hook's child command is:

```text
elegy-memory contextual-recall --config ABSOLUTE_CONFIG --json --non-interactive
```

Feedback is a separate event-bound command and must receive the same binding:

```text
elegy-memory recall-feedback --config ABSOLUTE_CONFIG --json --non-interactive
```

Both commands read JSON from stdin. The adapter reads recent transcript context
only when `includeRecentContext` is true and the transcript is under the
trusted absolute `transcriptRoot` (or project root when omitted). It forwards
at most four recent entries and 8000 UTF-8 bytes. It never writes transcripts
or source memories.

## Hook projection

[`hooks.example.json`](hooks.example.json) is a placeholder projection. Replace
every `ABSOLUTE_*` value with an operator-reviewed absolute path for the target
host; do not copy a personal path from an example. The one-second hook timeout
is an outer cutoff. Runtime-specific hook support must be smoke-tested with
synthetic events; this repository has no live-host installation evidence.

Run the adapter tests with:

```text
python -m unittest plugins/memory/integrations/codex/test_recall_hook.py
```

The adapter is opt-in and does not alter readiness or install state.
