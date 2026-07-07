//! Integration tests for the elegy-codegraph extraction and query pipeline.
//!
//! Tests extract + query against the in-tree fixture repos:
//! - `tests/fixtures/rust-mini/` — small Rust crate
//! - `tests/fixtures/ts-mini/` — small TypeScript project
//!
//! Structural assertions: entity counts, expected kinds/names, edge types,
//! provenance presence, and query correctness.

use elegy_codegraph::ir::{Confidence, EdgeKind, EntityKind};

/// Helper: find fixture path relative to the crate root.
fn fixture_path(name: &str) -> String {
    // CARGO_MANIFEST_DIR is tools/codegraph/
    // Fixtures are at tools/codegraph/tests/fixtures/
    format!("{}/tests/fixtures/{}", env!("CARGO_MANIFEST_DIR"), name)
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
        graph
            .entities
            .iter()
            .any(|e| e.kind == EntityKind::Function),
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
    assert!(!exports.is_empty(), "Expected at least one exports edge");

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
        graph
            .entities
            .iter()
            .any(|e| e.name.contains("integration_test")),
        "Expected the integration test file entity"
    );
}

#[test]
fn rust_query_pipeline() {
    let path = fixture_path("rust-mini");
    let graph = elegy_codegraph::extractor::rust_lang::extract(&path)
        .expect("Rust extraction should succeed");

    // Store in memory via temp file
    let dir = tempfile::tempdir().expect("create tempdir");
    let db_path = dir.path().join("test.redb");
    let store = elegy_codegraph::store::Store::open(db_path.to_str().expect("path to str"))
        .expect("open store");

    for entity in &graph.entities {
        store.insert_entity(entity).expect("insert entity");
    }
    for edge in &graph.edges {
        store.insert_edge(edge).expect("insert edge");
    }

    // Query
    let engine = elegy_codegraph::query::QueryEngine::new(store);

    // Symbol lookup
    let output = engine.symbol("add", None).expect("query symbol");
    let parsed: serde_json::Value = serde_json::from_str(&output).expect("parse output JSON");
    assert_eq!(parsed["status"], "ok");
    assert!(!parsed["data"].as_array().expect("data is array").is_empty());

    // Summary
    let output = engine.summary().expect("query summary");
    let parsed: serde_json::Value = serde_json::from_str(&output).expect("parse output JSON");
    assert_eq!(parsed["status"], "ok");
    let count = parsed["data"]["entityCount"]
        .as_u64()
        .expect("entityCount is u64");
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
    let graph =
        elegy_codegraph::extractor::ts::extract(&path).expect("TS extraction should succeed");

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
        graph
            .entities
            .iter()
            .any(|e| e.kind == EntityKind::Function),
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
    let graph =
        elegy_codegraph::extractor::ts::extract(&path).expect("TS extraction should succeed");

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
    let graph =
        elegy_codegraph::extractor::ts::extract(&path).expect("TS extraction should succeed");

    let dir = tempfile::tempdir().expect("create tempdir");
    let db_path = dir.path().join("test.redb");
    let store = elegy_codegraph::store::Store::open(db_path.to_str().expect("path to str"))
        .expect("open store");

    for entity in &graph.entities {
        store.insert_entity(entity).expect("insert entity");
    }
    for edge in &graph.edges {
        store.insert_edge(edge).expect("insert edge");
    }

    let engine = elegy_codegraph::query::QueryEngine::new(store);
    let output = engine.summary().expect("query summary");
    let parsed: serde_json::Value = serde_json::from_str(&output).expect("parse output JSON");
    assert_eq!(parsed["status"], "ok");
    assert!(
        parsed["data"]["entityCount"]
            .as_u64()
            .expect("entityCount is u64")
            > 0
    );
}

// ── CLI integration tests ─────────────────────────────────────

#[test]
fn cli_extract_rust_and_query_pipeline() {
    let repo_path = fixture_path("rust-mini");
    let dir = tempfile::tempdir().expect("create tempdir");
    let db_path = dir.path().join("test.redb");

    // Extract
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_elegy-codegraph"))
        .arg("extract")
        .arg("--lang")
        .arg("rust")
        .arg("--repo")
        .arg(&repo_path)
        .arg("--out")
        .arg(db_path.to_str().expect("path to str"))
        .output()
        .expect("Failed to run elegy-codegraph extract");

    assert!(output.status.success(), "Extract exited with error:\n{}", {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        format!("stderr:\n{}\nstdout:\n{}", stderr, stdout)
    });

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value =
        serde_json::from_str(&stdout).expect("Extract output is not valid JSON");
    assert_eq!(parsed["status"], "ok");
    let entity_count = parsed["entityCount"].as_u64().expect("entityCount is u64");
    assert!(entity_count > 0, "entityCount should be positive");
    let edge_count = parsed["edgeCount"].as_u64().expect("edgeCount is u64");
    assert!(edge_count > 0, "edgeCount should be positive");
    assert!(
        parsed["extractor"]
            .as_str()
            .expect("extractor is str")
            .contains("rust"),
        "extractor should contain 'rust'"
    );

    // Query: summary
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_elegy-codegraph"))
        .arg("query")
        .arg("--graph")
        .arg(db_path.to_str().expect("path to str"))
        .arg("summary")
        .output()
        .expect("Failed to run elegy-codegraph query summary");

    assert!(
        output.status.success(),
        "Query summary exited with error:\n{}",
        {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            format!("stderr:\n{}\nstdout:\n{}", stderr, stdout)
        }
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value =
        serde_json::from_str(&stdout).expect("Query summary output is not valid JSON");
    assert_eq!(parsed["status"], "ok");
    assert!(
        parsed["data"]["entityCount"]
            .as_u64()
            .expect("entityCount is u64")
            > 0,
        "Summary entityCount should be positive"
    );

    // Query: symbol --name add
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_elegy-codegraph"))
        .arg("query")
        .arg("--graph")
        .arg(db_path.to_str().expect("path to str"))
        .arg("symbol")
        .arg("--name")
        .arg("add")
        .output()
        .expect("Failed to run elegy-codegraph query symbol");

    assert!(
        output.status.success(),
        "Query symbol exited with error:\n{}",
        {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            format!("stderr:\n{}\nstdout:\n{}", stderr, stdout)
        }
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value =
        serde_json::from_str(&stdout).expect("Query symbol output is not valid JSON");
    assert_eq!(parsed["status"], "ok");
    let data = parsed["data"]
        .as_array()
        .expect("symbol data should be an array");
    assert!(!data.is_empty(), "symbol data should not be empty");
    assert!(
        data.iter().any(|e| e["name"] == "add"),
        "Expected entity named 'add' in symbol results"
    );

    // Query: impact --path src/lib.rs
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_elegy-codegraph"))
        .arg("query")
        .arg("--graph")
        .arg(db_path.to_str().expect("path to str"))
        .arg("impact")
        .arg("--path")
        .arg("src/lib.rs")
        .output()
        .expect("Failed to run elegy-codegraph query impact");

    assert!(
        output.status.success(),
        "Query impact exited with error:\n{}",
        {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            format!("stderr:\n{}\nstdout:\n{}", stderr, stdout)
        }
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value =
        serde_json::from_str(&stdout).expect("Query impact output is not valid JSON");
    assert_eq!(parsed["status"], "ok");
    assert!(
        parsed["data"]["entityCount"]
            .as_u64()
            .expect("entityCount is u64")
            > 0,
        "Impact entityCount should be positive"
    );
}

#[test]
fn cli_extract_invalid_lang_produces_error() {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_elegy-codegraph"))
        .arg("extract")
        .arg("--lang")
        .arg("go")
        .arg("--repo")
        .arg("/tmp")
        .arg("--out")
        .arg("/tmp/out.redb")
        .output()
        .expect("Failed to run elegy-codegraph extract");

    assert!(
        !output.status.success(),
        "Expected non-zero exit for unsupported language"
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value =
        serde_json::from_str(&stdout).expect("Error output is not valid JSON");
    assert_eq!(parsed["status"], "error");
    assert!(
        parsed["message"]
            .as_str()
            .expect("message is str")
            .contains("Unsupported"),
        "Error message should contain 'Unsupported'"
    );
}

#[test]
fn cli_extract_double_produces_fresh_snapshot() {
    let repo_path = fixture_path("rust-mini");
    let dir = tempfile::tempdir().expect("create tempdir");
    let db_path = dir.path().join("test.redb");

    // First extract
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_elegy-codegraph"))
        .arg("extract")
        .arg("--lang")
        .arg("rust")
        .arg("--repo")
        .arg(&repo_path)
        .arg("--out")
        .arg(db_path.to_str().expect("path to str"))
        .output()
        .expect("Failed to run elegy-codegraph extract");

    assert!(output.status.success(), "First extract failed");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let first: serde_json::Value =
        serde_json::from_str(&stdout).expect("First extract output is not valid JSON");
    let first_count = first["entityCount"].as_u64().expect("entityCount is u64");

    // Query redb to get baseline symbol count
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_elegy-codegraph"))
        .arg("query")
        .arg("--graph")
        .arg(db_path.to_str().expect("path to str"))
        .arg("symbol")
        .arg("--name")
        .arg("add")
        .output()
        .expect("Failed to run elegy-codegraph query symbol");
    assert!(output.status.success(), "First query symbol failed");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let first_query: serde_json::Value =
        serde_json::from_str(&stdout).expect("First query output is not valid JSON");
    let first_symbol_count = first_query["data"].as_array().expect("data is array").len();

    // Second extract — same output path
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_elegy-codegraph"))
        .arg("extract")
        .arg("--lang")
        .arg("rust")
        .arg("--repo")
        .arg(&repo_path)
        .arg("--out")
        .arg(db_path.to_str().expect("path to str"))
        .output()
        .expect("Failed to run elegy-codegraph extract");

    assert!(output.status.success(), "Second extract failed");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let second: serde_json::Value =
        serde_json::from_str(&stdout).expect("Second extract output is not valid JSON");
    let second_count = second["entityCount"].as_u64().expect("entityCount is u64");

    assert_eq!(
        first_count, second_count,
        "Re-extraction should produce the same entityCount (not doubled)"
    );

    // Query redb again — proves no duplicate name-index entries in persisted state
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_elegy-codegraph"))
        .arg("query")
        .arg("--graph")
        .arg(db_path.to_str().expect("path to str"))
        .arg("symbol")
        .arg("--name")
        .arg("add")
        .output()
        .expect("Failed to run elegy-codegraph query symbol after second extract");
    assert!(output.status.success(), "Second query symbol failed");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let second_query: serde_json::Value =
        serde_json::from_str(&stdout).expect("Second query output is not valid JSON");
    let second_symbol_count = second_query["data"]
        .as_array()
        .expect("data is array")
        .len();

    assert_eq!(
        first_symbol_count, second_symbol_count,
        "Symbol count in persisted redb should not change on re-extraction (no duplicate name-index entries)"
    );
}

#[test]
fn cli_extract_ts_when_available() {
    if !ts_extractor_available() {
        eprintln!("Skipping TS CLI test: Node.js + typescript not available");
        return;
    }

    let repo_path = fixture_path("ts-mini");
    let dir = tempfile::tempdir().expect("create tempdir");
    let db_path = dir.path().join("test.redb");

    // Extract
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_elegy-codegraph"))
        .arg("extract")
        .arg("--lang")
        .arg("ts")
        .arg("--repo")
        .arg(&repo_path)
        .arg("--out")
        .arg(db_path.to_str().expect("path to str"))
        .output()
        .expect("Failed to run elegy-codegraph extract");

    assert!(output.status.success(), "TS extract failed:\n{}", {
        String::from_utf8_lossy(&output.stderr)
    });

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value =
        serde_json::from_str(&stdout).expect("Extract output is not valid JSON");
    assert_eq!(parsed["status"], "ok");
    assert!(
        parsed["entityCount"].as_u64().expect("entityCount is u64") > 0,
        "entityCount should be positive"
    );

    // Query summary
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_elegy-codegraph"))
        .arg("query")
        .arg("--graph")
        .arg(db_path.to_str().expect("path to str"))
        .arg("summary")
        .output()
        .expect("Failed to run elegy-codegraph query summary");

    assert!(output.status.success(), "TS query summary failed:\n{}", {
        String::from_utf8_lossy(&output.stderr)
    });

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value =
        serde_json::from_str(&stdout).expect("Query summary output is not valid JSON");
    assert_eq!(parsed["status"], "ok");
    assert!(
        parsed["data"]["entityCount"]
            .as_u64()
            .expect("entityCount is u64")
            > 0,
        "Summary entityCount should be positive"
    );
}

#[test]
fn cli_extract_missing_parent_dir_produces_error() {
    let dir = tempfile::tempdir().expect("create tempdir");
    let bad_path = dir.path().join("nonexistent").join("out.redb");

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_elegy-codegraph"))
        .arg("extract")
        .arg("--lang")
        .arg("rust")
        .arg("--repo")
        .arg(fixture_path("rust-mini"))
        .arg("--out")
        .arg(bad_path.to_str().expect("path to str"))
        .output()
        .expect("Failed to run elegy-codegraph extract");

    assert!(
        !output.status.success(),
        "Expected non-zero exit for missing parent output directory"
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value =
        serde_json::from_str(&stdout).expect("Error output is not valid JSON");
    assert_eq!(parsed["status"], "error");
    assert!(
        parsed["message"]
            .as_str()
            .expect("message is str")
            .contains("Output directory does not exist"),
        "Error message should indicate missing output directory"
    );
}

#[test]
fn cli_extract_ts_with_use_scip_produces_warning() {
    if !ts_extractor_available() {
        eprintln!("Skipping TS CLI use-scip test: Node.js + typescript not available");
        return;
    }

    let repo_path = fixture_path("ts-mini");
    let dir = tempfile::tempdir().expect("create tempdir");
    let db_path = dir.path().join("test.redb");

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_elegy-codegraph"))
        .arg("extract")
        .arg("--lang")
        .arg("ts")
        .arg("--repo")
        .arg(&repo_path)
        .arg("--out")
        .arg(db_path.to_str().expect("path to str"))
        .arg("--use-scip")
        .output()
        .expect("Failed to run elegy-codegraph extract with --use-scip");

    assert!(
        output.status.success(),
        "TS extract with --use-scip failed:\n{}",
        { String::from_utf8_lossy(&output.stderr) }
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value =
        serde_json::from_str(&stdout).expect("Extract output is not valid JSON");
    assert_eq!(parsed["status"], "ok");
    assert!(
        parsed.get("warning").is_some(),
        "Expected a warning field when --use-scip is passed for TS"
    );
    let warning = parsed["warning"].as_str().expect("warning is str");
    assert!(
        warning.contains("not supported") || warning.contains("ignoring"),
        "Warning should indicate --use-scip is not supported for TS, got: {}",
        warning
    );
}
