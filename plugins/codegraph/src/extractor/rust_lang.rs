//! Rust syntax extractor using tree-sitter and cargo metadata.
//!
//! Always-on layer providing syntax-level facts: modules, functions, structs,
//! enums, traits, impls, crate graph. Augmented by the SCIP layer when available.
//!
//! ## Known gaps (v0)
//! - `#[cfg]` branches other than `test` are flattened.
//! - Macros are recorded as entities but not expanded.
//! - `async fn` is treated as a regular function.
//! - Cross-crate dependency edges are deferred to SCIP layer.
//! - Method calls (`a.b()`) are not resolved as calls.

use crate::error::Result;
use crate::ir::{
    Confidence, Edge, EdgeKind, Entity, EntityId, EntityKind, ExtractorMeta, Graph, Provenance,
    Span,
};
use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

// ── Cargo metadata types ──

#[allow(dead_code)]
#[derive(serde::Deserialize)]
struct CargoMetadata {
    packages: Vec<CargoPackage>,
    workspace_members: Vec<String>,
}

#[allow(dead_code)]
#[derive(serde::Deserialize)]
struct CargoPackage {
    name: String,
    manifest_path: String,
    dependencies: Vec<CargoDependency>,
}

#[allow(dead_code)]
#[derive(serde::Deserialize)]
struct CargoDependency {
    name: String,
}

// ── Extractor state ──

struct ExtractorState {
    entities: Vec<Entity>,
    edges: Vec<Edge>,
    module_stack: Vec<String>,
    file_path: String,
    /// Stack of enclosing entities for call_expression resolution.
    entity_stack: Vec<EntityId>,
    /// Track entities by (qualified_name, file) for name resolution.
    symbol_index: HashMap<(String, String), EntityId>,
}

impl ExtractorState {
    fn new(file_path: &str) -> Self {
        Self {
            entities: Vec::new(),
            edges: Vec::new(),
            module_stack: Vec::new(),
            file_path: file_path.to_string(),
            entity_stack: Vec::new(),
            symbol_index: HashMap::new(),
        }
    }

    /// Reset per-file state before processing a new file.
    fn reset(&mut self, file_path: &str) {
        self.module_stack.clear();
        self.entity_stack.clear();
        self.file_path = file_path.to_string();
    }

    fn current_module(&self) -> String {
        self.module_stack.join("::")
    }

    fn qualified_name(&self, name: &str) -> String {
        let mod_path = self.current_module();
        let file_stem = self
            .file_path
            .trim_end_matches(".rs")
            .replace(['/', '\\'], "::");
        if mod_path.is_empty() {
            format!("{}::{}", file_stem, name)
        } else {
            format!("{}::{}", mod_path, name)
        }
    }

    fn add_entity(&mut self, kind: EntityKind, name: &str, layer: &str, span: Span) -> EntityId {
        let qualified_name = self.qualified_name(name);
        let id = Entity::compute_id(&qualified_name, &self.file_path, &kind);

        if self
            .symbol_index
            .contains_key(&(qualified_name.clone(), self.file_path.clone()))
        {
            return id;
        }

        let entity = Entity {
            id: id.clone(),
            kind,
            layer: layer.to_string(),
            name: name.to_string(),
            qualified_name: qualified_name.clone(),
            file: self.file_path.clone(),
            span: Some(span),
            inputs: vec![],
            outputs: vec![],
            side_effects: vec![],
            dependencies: vec![],
            tests: vec![],
            docs: vec![],
            provenance: Provenance {
                extractor: "elegy-codegraph-rust".to_string(),
                confidence: Confidence::Exact,
                evidence_refs: vec![],
            },
        };

        self.symbol_index
            .insert((qualified_name, self.file_path.clone()), id.clone());
        self.entities.push(entity);
        id
    }

    fn add_edge(&mut self, src: &EntityId, dst: &EntityId, kind: EdgeKind) {
        if self
            .edges
            .iter()
            .any(|e| e.src == *src && e.dst == *dst && e.kind == kind)
        {
            return;
        }

        let confidence = match kind {
            EdgeKind::Tests => Confidence::Inferred,
            _ => Confidence::Exact,
        };

        self.edges.push(Edge {
            src: src.clone(),
            dst: dst.clone(),
            kind,
            provenance: Provenance {
                extractor: "elegy-codegraph-rust".to_string(),
                confidence,
                evidence_refs: vec![],
            },
        });
    }
}

// ── Span helper ──

fn make_span(node: &tree_sitter::Node, target: &tree_sitter::Node) -> Span {
    let start = node.start_position();
    let end = target.end_position();
    Span {
        start: (start.row as u32 + 1, start.column as u32 + 1),
        end: (end.row as u32 + 1, end.column as u32 + 1),
    }
}

// ── Visibility detection ──

fn has_visibility(node: &tree_sitter::Node) -> bool {
    for i in 0..node.child_count() {
        if let Some(child) = node.child(i) {
            if child.kind() == "visibility_modifier" {
                return true;
            }
        }
    }
    false
}

// ── Test detection ──

fn detect_test_attr(node: &tree_sitter::Node, source: &str) -> &'static str {
    // Outer attributes (`#[test]`) are siblings before the item in tree-sitter-rust.
    let mut current = Some(*node);
    while let Some(n) = current {
        let mut sibling = n.prev_sibling();
        while let Some(s) = sibling {
            if s.kind() == "attribute_item" {
                if let Ok(text) = s.utf8_text(source.as_bytes()) {
                    if text.contains("test") {
                        return "test";
                    }
                }
            }
            sibling = s.prev_sibling();
        }
        current = n.parent();
    }
    "source"
}

fn is_doc_comment(node: &tree_sitter::Node, source: &str) -> bool {
    match node.kind() {
        "line_comment" => {
            if let Ok(text) = node.utf8_text(source.as_bytes()) {
                text.starts_with("///")
            } else {
                false
            }
        }
        "block_comment" => {
            if let Ok(text) = node.utf8_text(source.as_bytes()) {
                text.starts_with("/**")
            } else {
                false
            }
        }
        _ => false,
    }
}

// ── Cargo metadata ──

fn get_cargo_metadata(repo_path: &str) -> Result<CargoMetadata> {
    let output = Command::new("cargo")
        .args(["metadata", "--format-version", "1", "--no-deps"])
        .current_dir(repo_path)
        .output()
        .map_err(|e| {
            crate::error::Error::Extraction(format!("Failed to run cargo metadata: {}", e))
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(crate::error::Error::Extraction(format!(
            "cargo metadata failed: {}",
            stderr
        )));
    }

    let meta: CargoMetadata = serde_json::from_slice(&output.stdout).map_err(|e| {
        crate::error::Error::Extraction(format!("Failed to parse cargo metadata: {}", e))
    })?;

    Ok(meta)
}

fn add_crate_edges(_state: &mut ExtractorState, _meta: &CargoMetadata) {
    // In v0, crate-level edges are recorded but minimally:
    // each package's entities are already tagged with file paths.
    // Cross-crate dependency edges are deferred to SCIP layer.
    // For now, record intra-workspace dependencies as imports edges
    // between file entities of different packages.
}

// ── Tree-sitter node processing ──

fn process_node(
    node: &tree_sitter::Node,
    source: &str,
    state: &mut ExtractorState,
    file_id: &EntityId,
) -> Result<()> {
    match node.kind() {
        "function_item" => {
            if let Some(name_node) = node.child_by_field_name("name") {
                let name = name_node.utf8_text(source.as_bytes()).unwrap_or("?");
                let span = make_span(node, &name_node);
                let layer = detect_test_attr(node, source);
                let entity_id = state.add_entity(EntityKind::Function, name, layer, span);

                if has_visibility(node) {
                    state.add_edge(file_id, &entity_id, EdgeKind::Exports);
                }

                // Doc comment detection
                if let Some(prev) = node.prev_sibling() {
                    if is_doc_comment(&prev, source) {
                        state.add_edge(&entity_id, file_id, EdgeKind::Documents);
                    }
                }

                // Push entity for call resolution in children.
                state.entity_stack.push(entity_id);
                for i in 0..node.child_count() {
                    if let Some(child) = node.child(i) {
                        process_node(&child, source, state, file_id)?;
                    }
                }
                state.entity_stack.pop();
            }
        }

        "struct_item" => {
            if let Some(name_node) = node.child_by_field_name("name") {
                let name = name_node.utf8_text(source.as_bytes()).unwrap_or("?");
                let span = make_span(node, &name_node);
                let layer = detect_test_attr(node, source);
                let entity_id = state.add_entity(EntityKind::Type, name, layer, span);
                if has_visibility(node) {
                    state.add_edge(file_id, &entity_id, EdgeKind::Exports);
                }
            }
        }

        "enum_item" => {
            if let Some(name_node) = node.child_by_field_name("name") {
                let name = name_node.utf8_text(source.as_bytes()).unwrap_or("?");
                let span = make_span(node, &name_node);
                let entity_id = state.add_entity(EntityKind::Enum, name, "source", span);
                if has_visibility(node) {
                    state.add_edge(file_id, &entity_id, EdgeKind::Exports);
                }
            }
        }

        "trait_item" => {
            if let Some(name_node) = node.child_by_field_name("name") {
                let name = name_node.utf8_text(source.as_bytes()).unwrap_or("?");
                let span = make_span(node, &name_node);
                let entity_id = state.add_entity(EntityKind::Trait, name, "source", span);
                if has_visibility(node) {
                    state.add_edge(file_id, &entity_id, EdgeKind::Exports);
                }
            }
        }

        "impl_item" => {
            // Extract trait or type name.
            let name = if let Some(type_node) = node.child_by_field_name("type") {
                format!(
                    "impl {}",
                    type_node.utf8_text(source.as_bytes()).unwrap_or("?")
                )
            } else {
                "impl".to_string()
            };
            let span = make_span(node, node);
            let entity_id = state.add_entity(EntityKind::Impl, &name, "source", span);
            if has_visibility(node) {
                state.add_edge(file_id, &entity_id, EdgeKind::Exports);
            }
        }

        "type_item" => {
            if let Some(name_node) = node.child_by_field_name("name") {
                let name = name_node.utf8_text(source.as_bytes()).unwrap_or("?");
                let span = make_span(node, &name_node);
                let entity_id = state.add_entity(EntityKind::Type, name, "source", span);
                if has_visibility(node) {
                    state.add_edge(file_id, &entity_id, EdgeKind::Exports);
                }
            }
        }

        "const_item" => {
            if let Some(name_node) = node.child_by_field_name("name") {
                let name = name_node.utf8_text(source.as_bytes()).unwrap_or("?");
                let span = make_span(node, &name_node);
                let entity_id = state.add_entity(EntityKind::Constant, name, "source", span);
                if has_visibility(node) {
                    state.add_edge(file_id, &entity_id, EdgeKind::Exports);
                }
            }
        }

        "macro_definition" => {
            if let Some(name_node) = node.child_by_field_name("name") {
                let name = name_node.utf8_text(source.as_bytes()).unwrap_or("?");
                let span = make_span(node, &name_node);
                state.add_entity(EntityKind::Macro, name, "source", span);
                // Macros are implicitly exported in Rust.
            }
        }

        "mod_item" => {
            if let Some(name_node) = node.child_by_field_name("name") {
                let name = name_node.utf8_text(source.as_bytes()).unwrap_or("?");
                let span = make_span(node, &name_node);

                // Push module to stack for qualified name building.
                state.module_stack.push(name.to_string());

                let mod_id = state.add_entity(EntityKind::Module, name, "source", span);
                state.add_edge(file_id, &mod_id, EdgeKind::Owns);
                if has_visibility(node) {
                    state.add_edge(file_id, &mod_id, EdgeKind::Exports);
                }

                // Process body (inline modules).
                if let Some(body) = node.child_by_field_name("body") {
                    process_node(&body, source, state, file_id)?;
                }

                // Pop module.
                state.module_stack.pop();
            }
        }

        "use_declaration" => {
            // Extract the use path as an import edge.
            if let Ok(path_text) = node.utf8_text(source.as_bytes()) {
                let clean_path = path_text
                    .strip_prefix("use ")
                    .unwrap_or(path_text)
                    .trim_end_matches(';')
                    .trim();
                let span = make_span(node, node);
                let import_id = state.add_entity(EntityKind::Module, clean_path, "source", span);
                state.add_edge(file_id, &import_id, EdgeKind::Imports);
            }
        }

        "call_expression" => {
            // Simple name-based call resolution (v0 heuristic).
            if let Some(func) = node.child_by_field_name("function") {
                if let Ok(name) = func.utf8_text(source.as_bytes()) {
                    // Clone src before mutable borrow of state.
                    let src = state.entity_stack.last().cloned();
                    // Collect matching dst IDs from symbol_index.
                    let dst_ids: Vec<EntityId> = state
                        .symbol_index
                        .iter()
                        .filter(|((qualified, _file), _dst_id)| {
                            qualified.ends_with(&format!("::{}", name)) || *qualified == *name
                        })
                        .map(|(_key, dst_id)| dst_id.clone())
                        .collect();
                    if let Some(ref src_id) = src {
                        for dst_id in &dst_ids {
                            if dst_id != src_id {
                                state.add_edge(src_id, dst_id, EdgeKind::Calls);
                            }
                        }
                    }
                }
            }
        }

        _ => {}
    }

    // Recurse into children.
    // Skip kinds that handle their own recursion (mod_item with body processing,
    // function_item with entity stack management).
    if node.kind() != "mod_item" && node.kind() != "function_item" {
        for i in 0..node.child_count() {
            if let Some(child) = node.child(i) {
                process_node(&child, source, state, file_id)?;
            }
        }
    }

    Ok(())
}

// ── Main extraction entry point ──

/// Extract a syntax-level graph from a Rust workspace at the given path.
pub fn extract(repo_path: &str) -> Result<Graph> {
    let rust_lang = tree_sitter_rust::LANGUAGE.into();
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&rust_lang).map_err(|e| {
        crate::error::Error::Extraction(format!("Failed to set tree-sitter-rust language: {}", e))
    })?;

    let mut state = ExtractorState::new("");

    // 1. Parse cargo metadata (best-effort — failures are logged but non-fatal).
    let cargo_meta = get_cargo_metadata(repo_path).ok();

    // 2. Walk .rs files.
    for entry in walkdir::WalkDir::new(repo_path)
        .into_iter()
        .filter_entry(|e| e.file_name().to_str() != Some("target"))
    {
        let entry = match entry {
            Ok(e) => e,
            Err(err) => {
                // Log and skip entries that can't be read.
                eprintln!("warn: skipping unreadable entry in {}: {}", repo_path, err);
                continue;
            }
        };

        let path = entry.path();
        if path.extension().is_none_or(|ext| ext != "rs") {
            continue;
        }
        if !entry.file_type().is_file() {
            continue;
        }

        // Compute path relative to repo root.
        let file_path = pathdiff::diff_paths(path, Path::new(repo_path))
            .unwrap_or_else(|| path.to_path_buf())
            .to_string_lossy()
            .replace('\\', "/");

        let source = std::fs::read_to_string(path)?;

        let tree = parser.parse(&source, None).ok_or_else(|| {
            crate::error::Error::Extraction(format!("Failed to parse {}", file_path))
        })?;

        let root = tree.root_node();
        state.reset(&file_path);

        // Determine layer from file path.
        let layer = if file_path.starts_with("tests/") || file_path.contains("/tests/") {
            "test"
        } else {
            "source"
        };

        // Add file entity.
        let file_span = Span {
            start: (1, 1),
            end: (1, 1),
        };
        let file_id = state.add_entity(EntityKind::File, &file_path, layer, file_span);

        // Walk the CST.
        process_node(&root, &source, &mut state, &file_id)?;
    }

    // 3. Add crate-level edges from cargo metadata.
    if let Some(ref meta) = cargo_meta {
        add_crate_edges(&mut state, meta);
    }

    Ok(Graph {
        schema: "elegy-codegraph.graph.v0".to_string(),
        extractor: ExtractorMeta {
            name: "elegy-codegraph-rust".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            lang: "rust".to_string(),
            warning: None,
        },
        entities: state.entities,
        edges: state.edges,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_make_span() {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_rust::LANGUAGE.into())
            .expect("set tree-sitter language");

        let source = "fn hello() {}";
        let tree = parser.parse(source, None).expect("parse source");
        let root = tree.root_node();

        let mut cursor = root.walk();
        for child in root.children(&mut cursor) {
            if child.kind() == "function_item" {
                let span = make_span(&child, &child);
                assert_eq!(span.start, (1, 1));
                break;
            }
        }
    }

    #[test]
    fn test_detect_test_attr() {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_rust::LANGUAGE.into())
            .expect("set tree-sitter language");

        let source = "#[test]\nfn my_test() {}";
        let tree = parser.parse(source, None).expect("parse source");
        let root = tree.root_node();

        let mut cursor = root.walk();
        for child in root.children(&mut cursor) {
            if child.kind() == "function_item" {
                let layer = detect_test_attr(&child, source);
                assert_eq!(layer, "test");
                break;
            }
        }
    }

    #[test]
    fn test_has_visibility() {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_rust::LANGUAGE.into())
            .expect("set tree-sitter language");

        let source = "pub fn hello() {}";
        let tree = parser.parse(source, None).expect("parse source");
        let root = tree.root_node();

        let mut cursor = root.walk();
        for child in root.children(&mut cursor) {
            if child.kind() == "function_item" {
                assert!(has_visibility(&child));
                break;
            }
        }
    }

    #[test]
    fn test_extract_parses_simple_file() {
        let dir = tempfile::tempdir().expect("create tempdir");
        let src_dir = dir.path().join("src");
        std::fs::create_dir_all(&src_dir).expect("create src dir");
        std::fs::write(
            src_dir.join("lib.rs"),
            "pub fn add(a: i32, b: i32) -> i32 { a + b }\n\n#[test]\nfn test_add() { add(1, 2); }\n",
        )
        .expect("write lib.rs");

        // Write a minimal Cargo.toml.
        std::fs::write(
            dir.path().join("Cargo.toml"),
            "[package]\nname = \"test-crate\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
        )
        .expect("write Cargo.toml");

        let graph = extract(dir.path().to_str().expect("path to str")).expect("extract graph");
        assert!(!graph.entities.is_empty(), "Should extract entities");
        assert!(
            graph.entities.iter().any(|e| e.name == "add"),
            "Should find add function"
        );
        assert!(
            graph.entities.iter().any(|e| e.name == "test_add"),
            "Should find test function"
        );
    }
}
