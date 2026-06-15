//! Integration tests for the elegy-codegraph extraction and query pipeline.
//!
//! Tests extract + query against the in-tree fixture repos:
//! - `rust/tests/fixtures/rust-mini/` — small Rust crate
//! - `rust/tests/fixtures/ts-mini/` — small TypeScript project
//!
//! Structural assertions: entity counts, expected kinds/names, edge types,
//! provenance presence, and query correctness.

use elegy_codegraph::ir::{Confidence, EdgeKind, EntityKind};

/// Helper: find fixture path relative to the crate root.
fn fixture_path(name: &str) -> String {
    // CARGO_MANIFEST_DIR is rust/crates/elegy-codegraph/
    // Fixtures are at rust/tests/fixtures/ — go up two levels, then into tests/fixtures
    format!(
        "{}/../../tests/fixtures/{}",
        env!("CARGO_MANIFEST_DIR"),
        name
    )
}

// ── Rust fixture tests ──────────────────────────────────────────

#[test]
fn rust_extract_has_expected_entity_kinds() {
    let path = fixture_path("rust-mini");
    let graph = elegy_codegraph::extractor::rust_lang::extract(&path)
        .expect("Rust extraction should succeed");

    // Entity count should be reasonable (at least 5)
    assert!(
        graph.entities.len() >= 5,
        "Expected at least 5 entities, got {}",
        graph.entities.len()
    );

    // Should have file entities
    assert!(
        graph.entities.iter().any(|e| e.kind == EntityKind::File),
        "Expected at least one file entity"
    );

    // Should have function entities
    assert!(
        graph.entities.iter().any(|e| e.kind == EntityKind::Function),
        "Expected at least one function entity"
    );

    // Should have type/struct entity (structs are extracted as Type)
    assert!(
        graph.entities.iter().any(|e| e.kind == EntityKind::Type),
        "Expected at least one type (struct) entity"
    );

    // Should have the add function by name
    assert!(
        graph.entities.iter().any(|e| e.name == "add"),
        "Expected 'add' function entity"
    );

    // Should have the Counter struct by name
    assert!(
        graph.entities.iter().any(|e| e.name == "Counter"),
        "Expected 'Counter' struct entity"
    );

    // Should have the helper module
    assert!(
        graph.entities.iter().any(|e| e.name == "helper"),
        "Expected 'helper' module entity"
    );
}

#[test]
fn rust_extract_has_expected_edges() {
    let path = fixture_path("rust-mini");
    let graph = elegy_codegraph::extractor::rust_lang::extract(&path)
        .expect("Rust extraction should succeed");

    // Should have exports edges (pub functions are exported from file)
    let exports: Vec<_> = graph
        .edges
        .iter()
        .filter(|e| e.kind == EdgeKind::Exports)
        .collect();
    assert!(
        !exports.is_empty(),
        "Expected at least one exports edge"
    );

    // Should have owns edges (module ownership)
    let owns: Vec<_> = graph
        .edges
        .iter()
        .filter(|e| e.kind == EdgeKind::Owns)
        .collect();
    assert!(!owns.is_empty(), "Expected at least one owns edge");
}

#[test]
fn rust_extract_all_entities_have_provenance() {
    let path = fixture_path("rust-mini");
    let graph = elegy_codegraph::extractor::rust_lang::extract(&path)
        .expect("Rust extraction should succeed");

    for entity in &graph.entities {
        assert!(
            !entity.provenance.extractor.is_empty(),
            "Entity {} missing extractor in provenance",
            entity.name
        );
        // Confidence should be Exact (syntax-level facts)
        assert!(
            entity.provenance.confidence == Confidence::Exact
                || entity.provenance.confidence == Confidence::Inferred,
            "Entity {} has unexpected confidence {:?}",
            entity.name,
            entity.provenance.confidence
        );
    }

    for edge in &graph.edges {
        assert!(
            !edge.provenance.extractor.is_empty(),
            "Edge {:?} -> {:?} missing extractor",
            edge.src,
            edge.dst
        );
    }
}

#[test]
fn rust_extract_finds_test_functions() {
    let path = fixture_path("rust-mini");
    let graph = elegy_codegraph::extractor::rust_lang::extract(&path)
        .expect("Rust extraction should succeed");

    // Should have entities from the test layer
    let tests: Vec<_> = graph
        .entities
        .iter()
        .filter(|e| e.layer == "test")
        .collect();
    assert!(
        !tests.is_empty(),
        "Expected at least one entity with layer='test'"
    );

    // Should have the test file itself
    assert!(
        graph.entities.iter().any(|e| e.name.contains("integration_test")),
        "Expected the integration test file entity"
    );
}

#[test]
fn rust_query_pipeline() {
    let path = fixture_path("rust-mini");
    let graph = elegy_codegraph::extractor::rust_lang::extract(&path)
        .expect("Rust extraction should succeed");

    // Store in memory via temp file
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("test.redb");
    let store = elegy_codegraph::store::Store::open(db_path.to_str().unwrap()).unwrap();

    for entity in &graph.entities {
        store.insert_entity(entity).unwrap();
    }
    for edge in &graph.edges {
        store.insert_edge(edge).unwrap();
    }

    // Query
    let engine = elegy_codegraph::query::QueryEngine::new(store);

    // Symbol lookup
    let output = engine.symbol("add", None).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
    assert_eq!(parsed["status"], "ok");
    assert!(!parsed["data"].as_array().unwrap().is_empty());

    // Summary
    let output = engine.summary().unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
    assert_eq!(parsed["status"], "ok");
    let count = parsed["data"]["entityCount"].as_u64().unwrap();
    assert!(count > 0, "Summary entity count should be positive");
}

// ── TypeScript fixture tests (requires Node.js + typescript) ───

/// Check if the TS extractor prerequisites are available.
fn ts_extractor_available() -> bool {
    std::process::Command::new("node")
        .arg("-e")
        .arg("try { require('typescript'); process.exit(0); } catch(e) { process.exit(1); }")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

#[test]
fn ts_extract_has_expected_entity_kinds() {
    if !ts_extractor_available() {
        eprintln!("Skipping TS test: Node.js + typescript not available");
        return;
    }

    let path = fixture_path("ts-mini");
    let graph = elegy_codegraph::extractor::ts::extract(&path)
        .expect("TS extraction should succeed");

    assert!(
        graph.entities.len() >= 5,
        "Expected at least 5 entities, got {}",
        graph.entities.len()
    );

    assert!(
        graph.entities.iter().any(|e| e.kind == EntityKind::File),
        "Expected file entities"
    );
    assert!(
        graph.entities.iter().any(|e| e.kind == EntityKind::Function),
        "Expected function entities"
    );
    assert!(
        graph.entities.iter().any(|e| e.name == "add"),
        "Expected 'add' function"
    );
    assert!(
        graph.entities.iter().any(|e| e.name == "Counter"),
        "Expected 'Counter' class"
    );
}

#[test]
fn ts_extract_all_entities_have_provenance() {
    if !ts_extractor_available() {
        eprintln!("Skipping TS test: Node.js + typescript not available");
        return;
    }

    let path = fixture_path("ts-mini");
    let graph = elegy_codegraph::extractor::ts::extract(&path)
        .expect("TS extraction should succeed");

    for entity in &graph.entities {
        assert!(
            !entity.provenance.extractor.is_empty(),
            "Entity {} missing provenance",
            entity.name
        );
    }
}

#[test]
fn ts_query_pipeline() {
    if !ts_extractor_available() {
        eprintln!("Skipping TS test: Node.js + typescript not available");
        return;
    }

    let path = fixture_path("ts-mini");
    let graph = elegy_codegraph::extractor::ts::extract(&path)
        .expect("TS extraction should succeed");

    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("test.redb");
    let store = elegy_codegraph::store::Store::open(db_path.to_str().unwrap()).unwrap();

    for entity in &graph.entities {
        store.insert_entity(entity).unwrap();
    }
    for edge in &graph.edges {
        store.insert_edge(edge).unwrap();
    }

    let engine = elegy_codegraph::query::QueryEngine::new(store);
    let output = engine.summary().unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
    assert_eq!(parsed["status"], "ok");
    assert!(parsed["data"]["entityCount"].as_u64().unwrap() > 0);
}
