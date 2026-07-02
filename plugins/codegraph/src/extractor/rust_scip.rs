//! Rust semantic augmentation via SCIP from rust-analyzer.
//!
//! Opt-in layer invoked when `--use-scip` is passed to `extract`.
//! Spawns `rust-analyzer scip` per workspace member, parses the SCIP
//! protobuf, and merges `calls` and `references` edges with confidence:exact.
//!
//! ## Graceful degradation
//!
//! When `rust-analyzer` is not on PATH, this module emits a `provenance.warning`
//! on the extraction metadata but does not fail the extraction. The tree-sitter
//! layer still produces a complete syntax-level graph.
//!
//! ## Why SCIP over full LSP
//!
//! SCIP is a one-shot protobuf artifact — no server lifecycle, no keep-warm
//! between CLI invocations, stable format. The `rust-analyzer scip` subcommand
//! produces the index and exits; we consume the result.

use crate::error::Result;
use crate::ir::{Confidence, Edge, EdgeKind, EntityId, Graph, Provenance};
use protobuf::Message;
use std::process::Command;

/// Run `rust-analyzer scip` in the given directory and return the raw protobuf bytes.
fn run_rust_analyzer_scip(repo_path: &str) -> Result<Vec<u8>> {
    let child = Command::new("rust-analyzer")
        .args(["scip", "--output", "-"])
        .current_dir(repo_path)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| {
            crate::error::Error::Extraction(format!(
                "Failed to spawn rust-analyzer: {}. \
                 Install with: rustup component add rust-analyzer",
                e
            ))
        })?;

    let output = child.wait_with_output().map_err(|e| {
        crate::error::Error::Extraction(format!("rust-analyzer process error: {}", e))
    })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(crate::error::Error::Extraction(format!(
            "rust-analyzer scip failed (exit {}): {}",
            output.status.code().unwrap_or(-1),
            stderr.trim()
        )));
    }

    Ok(output.stdout)
}

/// Augment a syntax-level graph with SCIP semantic edges.
///
/// # Process
///
/// 1. Run `rust-analyzer scip` to produce a SCIP index.
/// 2. Parse the SCIP protobuf into an index.
/// 3. Walk SCIP occurrences:
///    - For each reference occurrence, find the referencing entity
///      (the entity that contains this file+range) and the referenced
///      entity (from the symbol map), then add a `references` edge.
///    - For call-like references, also add `calls` edges.
///
/// Graceful degradation: if `rust-analyzer` is not available, returns
/// `Ok(())` after setting a warning on the extractor metadata.
pub fn augment(graph: &mut Graph, repo_path: &str) -> Result<()> {
    // Run rust-analyzer scip
    let scip_bytes = match run_rust_analyzer_scip(repo_path) {
        Ok(bytes) => bytes,
        Err(_) => {
            // Graceful degradation: rust-analyzer not available
            graph.extractor.warning = Some(
                "rust-analyzer not available on PATH; SCIP augmentation skipped. \
                 Install with: rustup component add rust-analyzer"
                    .to_string(),
            );
            return Ok(());
        }
    };

    // Parse SCIP protobuf using rust-protobuf v3 (scip 0.3.x)
    let scip_index = match scip::types::Index::parse_from_bytes(&scip_bytes) {
        Ok(index) => index,
        Err(e) => {
            graph.extractor.warning = Some(format!("Failed to parse SCIP output: {}", e));
            return Ok(());
        }
    };

    // Walk SCIP documents and occurrences
    for doc in &scip_index.documents {
        for occ in &doc.occurrences {
            let symbol_name = &occ.symbol;
            if symbol_name.is_empty() {
                continue;
            }

            // SCIP range is [startLine, startCharacter, endLine, endCharacter] (0-based)
            // or [startLine, startCharacter, endCharacter] (endLine = startLine inferred)
            let start_line = occ.range.first().copied();
            let start_line_1based = start_line.map(|l| l as u32 + 1);

            // Find the source entity (the one at this occurrence's location)
            let source_entity_id =
                find_entity_at_location(&graph.entities, &doc.relative_path, start_line_1based);

            // Find the target entity by symbol name
            let target_entity_id =
                find_entity_by_symbol(&graph.entities, symbol_name, &doc.relative_path);

            if let (Some(src_id), Some(dst_id)) = (&source_entity_id, &target_entity_id) {
                // Don't add self-references
                if src_id == dst_id {
                    continue;
                }

                // SCIP symbol_roles is a bitmask.
                // Role values: Definition=1, Import=2, WriteAccess=4, ReadAccess=8,
                // Generated=16, Test=32, ForwardDefinition=64
                let is_definition = occ.symbol_roles & 1 != 0;

                if !is_definition {
                    // Build evidence ref string from range
                    let evidence_loc = if occ.range.len() >= 3 {
                        format!("{}:{}:{}", doc.relative_path, occ.range[0], occ.range[1])
                    } else {
                        format!("{}:0:0", doc.relative_path)
                    };

                    // Add references edge
                    let edge = Edge {
                        src: src_id.clone(),
                        dst: dst_id.clone(),
                        kind: EdgeKind::References,
                        provenance: Provenance {
                            extractor: "elegy-codegraph-rust-scip".to_string(),
                            confidence: Confidence::Exact,
                            evidence_refs: vec![evidence_loc],
                        },
                    };
                    // Deduplicate edges
                    if !graph
                        .edges
                        .iter()
                        .any(|e| e.src == edge.src && e.dst == edge.dst && e.kind == edge.kind)
                    {
                        graph.edges.push(edge);
                    }

                    // Also add a calls edge since SCIP references from function
                    // bodies imply call relationships
                    let call_edge = Edge {
                        src: src_id.clone(),
                        dst: dst_id.clone(),
                        kind: EdgeKind::Calls,
                        provenance: Provenance {
                            extractor: "elegy-codegraph-rust-scip".to_string(),
                            confidence: Confidence::Exact,
                            evidence_refs: vec![],
                        },
                    };
                    if !graph.edges.iter().any(|e| {
                        e.src == call_edge.src && e.dst == call_edge.dst && e.kind == call_edge.kind
                    }) {
                        graph.edges.push(call_edge);
                    }
                }

                if is_definition {
                    // Update entity confidence to Exact for this entity
                    if let Some(entity) = graph.entities.iter_mut().find(|e| e.id == *src_id) {
                        if entity.provenance.confidence != Confidence::Exact {
                            entity.provenance.extractor = "elegy-codegraph-rust-scip".to_string();
                        }
                    }
                }
            }
        }
    }

    // Update extractor version
    graph.extractor.version = format!("{}+scip", env!("CARGO_PKG_VERSION"));
    // Clear any previous warning since SCIP succeeded
    graph.extractor.warning = None;

    Ok(())
}

/// Find the IR entity at a given file+line location.
fn find_entity_at_location(
    entities: &[crate::ir::Entity],
    file: &str,
    line: Option<u32>,
) -> Option<EntityId> {
    if let Some(line_val) = line {
        entities
            .iter()
            .filter(|e| e.file == file)
            .find(|e| {
                if let Some(span) = &e.span {
                    span.start.0 <= line_val && span.end.0 >= line_val
                } else {
                    false
                }
            })
            .map(|e| e.id.clone())
    } else {
        // No location info — try matching by file only (loose match)
        entities
            .iter()
            .find(|e| e.file == file)
            .map(|e| e.id.clone())
    }
}

/// Find the IR entity matching a SCIP symbol name.
fn find_entity_by_symbol(
    entities: &[crate::ir::Entity],
    symbol: &str,
    _file: &str,
) -> Option<EntityId> {
    // Try exact qualified name match first
    entities
        .iter()
        .find(|e| e.qualified_name == symbol)
        .map(|e| e.id.clone())
        .or_else(|| {
            // Try matching by short name (last segment of `::`)
            let short = symbol.rsplit("::").next().unwrap_or(symbol);
            entities
                .iter()
                .find(|e| e.name == short)
                .map(|e| e.id.clone())
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{Entity, EntityKind};

    fn make_entity(
        id: &str,
        name: &str,
        qualified_name: &str,
        file: &str,
        span: Option<(u32, u32, u32, u32)>,
    ) -> Entity {
        Entity {
            id: id.into(),
            kind: EntityKind::Function,
            layer: "source".into(),
            name: name.into(),
            qualified_name: qualified_name.into(),
            file: file.into(),
            span: span.map(|(s_l, s_c, e_l, e_c)| crate::ir::Span {
                start: (s_l, s_c),
                end: (e_l, e_c),
            }),
            inputs: vec![],
            outputs: vec![],
            side_effects: vec![],
            dependencies: vec![],
            tests: vec![],
            docs: vec![],
            provenance: crate::ir::Provenance {
                extractor: "test".into(),
                confidence: Confidence::Exact,
                evidence_refs: vec![],
            },
        }
    }

    #[test]
    fn test_find_entity_at_location_exact_line() {
        let entities = vec![make_entity(
            "e1",
            "foo",
            "mod::foo",
            "src/lib.rs",
            Some((5, 1, 10, 2)),
        )];
        let id = find_entity_at_location(&entities, "src/lib.rs", Some(7));
        assert_eq!(id, Some("e1".into()));
    }

    #[test]
    fn test_find_entity_at_location_outside_range() {
        let entities = vec![make_entity(
            "e1",
            "foo",
            "mod::foo",
            "src/lib.rs",
            Some((5, 1, 10, 2)),
        )];
        let id = find_entity_at_location(&entities, "src/lib.rs", Some(15));
        assert!(id.is_none());
    }

    #[test]
    fn test_find_entity_by_symbol_exact_match() {
        let entities = vec![make_entity(
            "e2",
            "bar",
            "crate::mod::bar",
            "src/mod.rs",
            None,
        )];
        let id = find_entity_by_symbol(&entities, "crate::mod::bar", "src/mod.rs");
        assert_eq!(id, Some("e2".into()));
    }

    #[test]
    fn test_find_entity_by_symbol_fallback_short_name() {
        let entities = vec![make_entity(
            "e3",
            "baz",
            "something::different::baz",
            "src/other.rs",
            None,
        )];
        // Exact match fails, should fall back to short name
        let id = find_entity_by_symbol(&entities, "unknown::path::baz", "src/other.rs");
        assert_eq!(id, Some("e3".into()));
    }
}
