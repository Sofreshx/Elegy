# Elegy Desktop Win32

## Boundaries

- This crate is a thin Win32 execution leaf for primitive input and window actions.
- High-level safety behavior such as dry-run, evidence capture, approval policy, and title-based targeting belongs in `elegy-desktop`, not here.
- Public APIs should remain safe typed wrappers over Win32 primitives.
- Return errors that map cleanly into the safe wrapper instead of encoding host policy or orchestration decisions.
- Every `unsafe` block must document the invariant that makes it sound.
