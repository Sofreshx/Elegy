# Changelog

All notable changes to this project will be documented in this file.

The format is based on Keep a Changelog principles, adapted for the current bootstrap and consolidation stage.

## [Unreleased]

### Added

- dedicated multi-OS Rust CI in the main `Elegy` monorepo
- monorepo security workflow covering Rust dependency review, secret scanning, and mixed-language CodeQL analysis
- root contributor, security, conduct, spec-baseline, and architecture index docs so `Elegy` is the public entrypoint for project governance
- Rust workspace toolchain and dependency-policy files inside `rust/`
- portable canonical workflow graph contracts plus deterministic serializer, deserializer, normalization, and portable workflow transformation support in the workflow formalization packages

### Changed

- package-boundary governance checks now focus on `.NET`/contract authority concerns instead of also acting as the only Rust CI surface
- repository docs now describe `Elegy` as the operational center for contributor and security posture
- sibling repos are being prepared for closeout verification with documentation that redirects active work to the main monorepo
- package-family version advanced to `0.2.0` for the next downstream workflow formalization adoption slice
