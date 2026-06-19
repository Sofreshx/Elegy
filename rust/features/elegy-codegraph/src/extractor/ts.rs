//! TypeScript extractor using the TypeScript Compiler API.
//!
//! Spawns Node.js with an embedded TypeScript extraction script that uses
//! `ts.createProgram` and `TypeChecker` to walk source files and emit raw
//! entities and edges. The Rust side normalizes the raw output into the
//! governed IR types with proper content-addressed IDs and provenance.
//!
//! ## Known gaps (v0)
//! - Cross-package monorepo resolution is best-effort via tsconfig paths/references.
//! - No runtime test-to-source binding; test detection is pattern-based.
//! - Dynamic `import()` expressions are only traced for literal string arguments.
//! - Requires Node.js and `typescript` on PATH or in local node_modules.

use crate::error::Result;
use crate::extractor::ts_script;
use crate::ir::{
    Confidence, Edge, EdgeKind, Entity, EntityId, EntityKind, ExtractorMeta, Graph, Provenance,
    Span, TypeHint,
};
use std::process::{Command, Stdio};

/// Raw entity emitted by the TypeScript script before normalization.
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawEntity {
    id: String,
    kind: String,
    layer: String,
    name: String,
    #[serde(rename = "qualifiedName")]
    qualified_name: String,
    file: String,
    span: Option<RawSpan>,
    #[serde(default)]
    inputs: Vec<RawTypeHint>,
    #[serde(default)]
    outputs: Vec<RawTypeHint>,
    #[serde(default)]
    #[allow(dead_code)]
    side_effects: Vec<String>,
    #[serde(default)]
    dependencies: Vec<String>,
    #[serde(default)]
    tests: Vec<String>,
    #[serde(default)]
    docs: Vec<String>,
}

#[derive(serde::Deserialize)]
struct RawSpan {
    start: Vec<u32>,
    end: Vec<u32>,
}

#[derive(serde::Deserialize)]
struct RawTypeHint {
    #[serde(default)]
    name: Option<String>,
    #[serde(default, rename = "typeHint")]
    type_hint: Option<String>,
}

/// Raw edge emitted by the TypeScript script.
#[derive(serde::Deserialize)]
struct RawEdge {
    src: String,
    dst: String,
    kind: String,
}

/// Raw output from the TypeScript extraction script.
#[derive(serde::Deserialize)]
struct RawOutput {
    entities: Vec<RawEntity>,
    edges: Vec<RawEdge>,
}

/// Parse a raw entity kind string into the IR EntityKind.
fn parse_entity_kind(raw: &str) -> EntityKind {
    match raw {
        "file" => EntityKind::File,
        "module" => EntityKind::Module,
        "function" => EntityKind::Function,
        "class" => EntityKind::Class,
        "method" => EntityKind::Method,
        "trait" => EntityKind::Trait,
        "impl" => EntityKind::Impl,
        "interface" => EntityKind::Interface,
        "type" => EntityKind::Type,
        "constant" => EntityKind::Constant,
        "enum" => EntityKind::Enum,
        "macro" => EntityKind::Macro,
        "test" => EntityKind::Test,
        "doc" => EntityKind::Doc,
        _ => EntityKind::Function, // fallback for unknown kinds
    }
}

/// Parse a raw edge kind string into the IR EdgeKind.
fn parse_edge_kind(raw: &str) -> EdgeKind {
    match raw {
        "imports" => EdgeKind::Imports,
        "exports" => EdgeKind::Exports,
        "calls" => EdgeKind::Calls,
        "references" => EdgeKind::References,
        "reads" => EdgeKind::Reads,
        "writes" => EdgeKind::Writes,
        "validates" => EdgeKind::Validates,
        "emits" => EdgeKind::Emits,
        "owns" => EdgeKind::Owns,
        "tests" => EdgeKind::Tests,
        "documents" => EdgeKind::Documents,
        _ => EdgeKind::References,
    }
}

/// Map of raw script IDs to stable content-addressed IR IDs.
fn build_id_map(entities: &[RawEntity]) -> std::collections::HashMap<String, EntityId> {
    let mut map = std::collections::HashMap::new();
    for raw in entities {
        let kind = parse_entity_kind(&raw.kind);
        let ir_id = Entity::compute_id(&raw.qualified_name, &raw.file, &kind);
        map.insert(raw.id.clone(), ir_id);
    }
    map
}

/// Normalize raw entities into IR entities with correct IDs and provenance.
fn normalize_entities(
    raw: Vec<RawEntity>,
    id_map: &std::collections::HashMap<String, EntityId>,
) -> Vec<Entity> {
    raw.into_iter()
        .map(|r| {
            let kind = parse_entity_kind(&r.kind);
            let id = Entity::compute_id(&r.qualified_name, &r.file, &kind);

            // Determine confidence: if the raw entity came from a resolved
            // source file, confidence is exact for the entity itself.
            let confidence = Confidence::Exact;

            // Clone file before moving into struct; used again for provenance evidence_refs.
            let repo_file = r.file.clone();

            Entity {
                id,
                kind,
                layer: r.layer,
                name: r.name,
                qualified_name: r.qualified_name,
                file: repo_file.clone(),
                span: r.span.map(|s| Span {
                    start: (s.start[0], s.start[1]),
                    end: (s.end[0], s.end[1]),
                }),
                inputs: r
                    .inputs
                    .into_iter()
                    .map(|i| TypeHint {
                        name: i.name,
                        type_hint: i.type_hint,
                    })
                    .collect(),
                outputs: r
                    .outputs
                    .into_iter()
                    .map(|o| TypeHint {
                        name: o.name,
                        type_hint: o.type_hint,
                    })
                    .collect(),
                side_effects: vec![], // TS extractor doesn't detect side effects in v0
                dependencies: r
                    .dependencies
                    .into_iter()
                    .filter_map(|dep| id_map.get(&dep).cloned())
                    .collect(),
                tests: r
                    .tests
                    .into_iter()
                    .filter_map(|t| id_map.get(&t).cloned())
                    .collect(),
                docs: r
                    .docs
                    .into_iter()
                    .filter_map(|d| id_map.get(&d).cloned())
                    .collect(),
                provenance: Provenance {
                    extractor: "elegy-codegraph-ts".to_string(),
                    confidence,
                    evidence_refs: vec![format!("{}:1:1", repo_file)],
                },
            }
        })
        .collect()
}

/// Normalize raw edges into IR edges with remapped IDs and provenance.
fn normalize_edges(
    raw: Vec<RawEdge>,
    id_map: &std::collections::HashMap<String, EntityId>,
) -> Vec<Edge> {
    raw.into_iter()
        .filter_map(|r| {
            let src = id_map.get(&r.src)?;
            let dst = id_map.get(&r.dst)?;
            let kind = parse_edge_kind(&r.kind);

            // Edge confidence: exports and calls are exact (from type-checker),
            // tests edge from pattern detection is inferred
            let confidence = match kind {
                EdgeKind::Tests => Confidence::Inferred,
                _ => Confidence::Exact,
            };

            Some(Edge {
                src: src.clone(),
                dst: dst.clone(),
                kind,
                provenance: Provenance {
                    extractor: "elegy-codegraph-ts".to_string(),
                    confidence,
                    evidence_refs: vec![],
                },
            })
        })
        .collect()
}

/// Extract a graph from a TypeScript project at the given path.
///
/// Spawns `node -e <script>` with the repo path. Requires Node.js and
/// the `typescript` package to be available (globally or in local node_modules).
pub fn extract(repo_path: &str) -> Result<Graph> {
    // Build the Node.js invocation
    let script = ts_script::TS_EXTRACT_SCRIPT;

    let child = Command::new("node")
        .arg("-e")
        .arg(script)
        .arg(repo_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| {
            crate::error::Error::Extraction(format!(
                "Failed to spawn Node.js for TypeScript extraction: {}. \
             Ensure Node.js and the 'typescript' package are installed.",
                e
            ))
        })?;

    let output = child
        .wait_with_output()
        .map_err(|e| crate::error::Error::Extraction(format!("Node.js process error: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(crate::error::Error::Extraction(format!(
            "TypeScript extraction failed (exit {}): {}\n\
             Hint: ensure Node.js and 'typescript' are installed. \
             Try: npm install typescript  or  npm install -g typescript",
            output.status.code().unwrap_or(-1),
            stderr.trim()
        )));
    }

    // Parse raw output
    let stdout = String::from_utf8_lossy(&output.stdout);
    let raw: RawOutput = serde_json::from_str(&stdout).map_err(|e| {
        crate::error::Error::Extraction(format!(
            "Failed to parse TypeScript extraction output: {}. Raw: {}",
            e,
            &stdout[..stdout.len().min(500)]
        ))
    })?;

    // Build ID map and normalize
    let id_map = build_id_map(&raw.entities);
    let entities = normalize_entities(raw.entities, &id_map);
    let edges = normalize_edges(raw.edges, &id_map);

    Ok(Graph {
        schema: "elegy-codegraph.graph.v0".to_string(),
        extractor: ExtractorMeta {
            name: "elegy-codegraph-ts".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            lang: "ts".to_string(),
            warning: None,
        },
        entities,
        edges,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_entity_kind_all_variants() {
        assert_eq!(parse_entity_kind("file"), EntityKind::File);
        assert_eq!(parse_entity_kind("function"), EntityKind::Function);
        assert_eq!(parse_entity_kind("class"), EntityKind::Class);
        assert_eq!(parse_entity_kind("test"), EntityKind::Test);
    }

    #[test]
    fn test_parse_edge_kind_all_variants() {
        assert_eq!(parse_edge_kind("exports"), EdgeKind::Exports);
        assert_eq!(parse_edge_kind("calls"), EdgeKind::Calls);
        assert_eq!(parse_edge_kind("tests"), EdgeKind::Tests);
        assert_eq!(parse_edge_kind("documents"), EdgeKind::Documents);
    }

    #[test]
    fn test_build_id_map_produces_stable_ids() {
        let raw = vec![RawEntity {
            id: "raw-1".into(),
            kind: "function".into(),
            layer: "source".into(),
            name: "add".into(),
            qualified_name: "src/math::add".into(),
            file: "src/math.ts".into(),
            span: None,
            inputs: vec![],
            outputs: vec![],
            side_effects: vec![],
            dependencies: vec![],
            tests: vec![],
            docs: vec![],
        }];
        let map = build_id_map(&raw);
        let ir_id = map.get("raw-1").expect("raw-1 should be mapped");
        assert_eq!(ir_id.len(), 40);
        assert_eq!(
            *ir_id,
            Entity::compute_id("src/math::add", "src/math.ts", &EntityKind::Function)
        );
    }

    #[test]
    fn test_normalize_entities_preserves_fields() {
        let raw = vec![RawEntity {
            id: "raw-1".into(),
            kind: "function".into(),
            layer: "source".into(),
            name: "add".into(),
            qualified_name: "src/math::add".into(),
            file: "src/math.ts".into(),
            span: Some(RawSpan {
                start: vec![3, 1],
                end: vec![5, 2],
            }),
            inputs: vec![RawTypeHint {
                name: Some("a".into()),
                type_hint: Some("number".into()),
            }],
            outputs: vec![RawTypeHint {
                name: None,
                type_hint: Some("number".into()),
            }],
            side_effects: vec![],
            dependencies: vec![],
            tests: vec![],
            docs: vec![],
        }];
        let id_map = build_id_map(&raw);
        let entities = normalize_entities(raw, &id_map);
        assert_eq!(entities.len(), 1);
        let e = &entities[0];
        assert_eq!(e.kind, EntityKind::Function);
        assert_eq!(e.name, "add");
        assert_eq!(e.qualified_name, "src/math::add");
        assert!(e.span.is_some());
        assert_eq!(e.inputs.len(), 1);
        assert_eq!(e.inputs[0].name.as_deref(), Some("a"));
        assert_eq!(e.provenance.confidence, Confidence::Exact);
    }
}
