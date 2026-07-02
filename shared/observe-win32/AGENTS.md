# Elegy Observe Win32

## Boundaries

- This crate is a thin Win32 observation leaf. It should expose safe typed primitives over raw Win32 calls, not higher-level orchestration or policy.
- Observation contracts and cross-platform behavior belong to governed artifacts and `elegy-observe`, not this crate.
- Keep Windows-only details here and return errors that map cleanly into the safe wrapper surface.
- Public APIs should remain safe typed wrappers over Win32 primitives.
- Every `unsafe` block must document the invariant that makes it sound.
