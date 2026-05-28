# FLIGHT_RECORDER_PROTOCOL

Use `FLIGHT_RECORDER.md` as the append-only execution ledger for autonomous or multi-phase work tracked from `prompt.md`.

## Rules
- Append only. Never rewrite, delete, or silently replace prior entries; add a clearly labeled correction note if an earlier entry is wrong.
- Record a checkpoint before and after each work unit (WU) so intent, execution, and outcome are recoverable.
- For validation work, record the exact commands run plus the observed results or exit status.
- Log drifts, blockers, and prompt or documentation contradictions when they are discovered, with a short impact note.
- Keep entries factual and concise: what changed, what was validated, and what remains.
