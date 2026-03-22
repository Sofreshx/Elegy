# Changelog

All notable changes to this project will be documented in this file.

The format is based on Keep a Changelog principles, adapted for the current bootstrap and consolidation stage.

## [Unreleased]

### Added

- governed contract bundle export and validation flows centered on `scripts/export-contracts.ps1` and `scripts/validate-canonical-outputs.ps1`
- dedicated multi-OS Rust CI for the surviving workspace under `rust/`
- repository security workflow covering `cargo-deny`, `cargo-audit`, `gitleaks`, and Rust CodeQL analysis
- contributor-facing Rust CLI self-authoring slice for `author mcp`, `analyze mcp`, and `generate skills`, backed by `rust/crates/elegy-tooling`

### Changed

- repository authority language now centers on `contracts/`, `governance/`, `schemas/`, and `policies/` plus exported artifacts, rather than removed source-package roots
- architecture and policy docs now describe `rust/` as the active executable surface and avoid claiming a built-in MCP-native or skill-driven self-authoring surface that the repo does not yet ship
- validation posture now points to surviving scripts and workflows instead of removed legacy build and test flows
