//! Query engine for the code graph.
//!
//! Provides symbol lookup, neighbor traversal, impact analysis, and
//! structural summary queries.

use crate::error::Result;
use crate::ir::Entity;
use crate::store::Store;
use serde::Serialize;

/// Query output envelope with provenance.
#[derive(Serialize)]
struct QueryResult<T: Serialize> {
    status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

impl<T: Serialize> QueryResult<T> {
    fn ok(data: T) -> Self {
        Self {
            status: "ok".to_string(),
            data: Some(data),
            error: None,
        }
    }

    fn not_found(msg: String) -> Self {
        Self {
            status: "not_found".to_string(),
            data: None,
            error: Some(msg),
        }
    }

    fn error(msg: String) -> Self {
        Self {
            status: "error".to_string(),
            data: None,
            error: Some(msg),
        }
    }
}

/// Query the graph store.
pub struct QueryEngine {
    store: Store,
}

impl QueryEngine {
    /// Create a new query engine backed by the given store.
    pub fn new(store: Store) -> Self {
        Self { store }
    }

    /// Look up a symbol by name.
    ///
    /// Searches the entities_by_name index for exact name matches.
    /// If `lang` is provided, filters results to entities extracted from
    /// that language. The lang filter uses the extractor metadata stored
    /// with the entity's provenance.
    ///
    /// Returns JSON with status "ok", "not_found", or "error".
    pub fn symbol(&self, name: &str, lang: Option<&str>) -> Result<String> {
        let entities = self.store.get_by_name(name)?;

        let results: Vec<&Entity> = if let Some(lang_filter) = lang {
            // Filter by language: we match the extractor name prefix
            // since provenance.extractor is like "elegy-codegraph-ts"
            entities
                .iter()
                .filter(|e| {
                    let extractor = &e.provenance.extractor;
                    match lang_filter {
                        "ts" => extractor.contains("ts"),
                        "rust" => extractor.contains("rust"),
                        _ => true,
                    }
                })
                .collect()
        } else {
            entities.iter().collect()
        };

        if results.is_empty() {
            let msg = if entities.is_empty() {
                format!("No entity found with name '{}'", name)
            } else {
                format!(
                    "Found {} entities named '{}' but none match lang filter '{}'",
                    entities.len(),
                    name,
                    lang.unwrap_or("?")
                )
            };
            Ok(serde_json::to_string_pretty(&QueryResult::<Vec<&Entity>>::not_found(msg))?)
        } else {
            Ok(serde_json::to_string_pretty(&QueryResult::ok(&results))?)
        }
    }

    /// Get neighbors of an entity (deferred to wp-query-neighbors).
    pub fn neighbors(&self, _id: &str, _direction: &str) -> Result<String> {
        Ok(serde_json::to_string_pretty(&QueryResult::<()>::error(
            "neighbors query not yet implemented (wp-query-neighbors)".to_string(),
        ))?)
    }

    /// Analyze impact of changes to a file (deferred to wp-query-impact).
    pub fn impact(&self, _path: &str) -> Result<String> {
        Ok(serde_json::to_string_pretty(&QueryResult::<()>::error(
            "impact query not yet implemented (wp-query-impact)".to_string(),
        ))?)
    }

    /// Get a structural summary (deferred to wp-query-summary).
    pub fn summary(&self) -> Result<String> {
        Ok(serde_json::to_string_pretty(&QueryResult::<()>::error(
            "summary query not yet implemented (wp-query-summary)".to_string(),
        ))?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{Confidence, Entity, EntityKind, Provenance};

    fn make_entity(id: &str, name: &str, extractor: &str) -> Entity {
        Entity {
            id: id.to_string(),
            kind: EntityKind::Function,
            layer: "source".to_string(),
            name: name.to_string(),
            qualified_name: format!("mod::{}", name),
            file: "lib.rs".to_string(),
            span: None,
            inputs: vec![],
            outputs: vec![],
            side_effects: vec![],
            dependencies: vec![],
            tests: vec![],
            docs: vec![],
            provenance: Provenance {
                extractor: extractor.to_string(),
                confidence: Confidence::Exact,
                evidence_refs: vec![],
            },
        }
    }

    #[test]
    fn test_symbol_query_json_format() {
        // Test that the symbol query output is valid JSON with expected structure
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("test.redb");
        let store = Store::open(db_path.to_str().unwrap()).unwrap();

        let entity = make_entity("e1", "add", "elegy-codegraph-ts");
        store.insert_entity(&entity).unwrap();

        let engine = QueryEngine::new(store);
        let output = engine.symbol("add", None).unwrap();

        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert_eq!(parsed["status"], "ok");
        assert!(parsed["data"].is_array());
        assert_eq!(parsed["data"][0]["name"], "add");
    }

    #[test]
    fn test_symbol_not_found() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("test.redb");
        let store = Store::open(db_path.to_str().unwrap()).unwrap();
        let engine = QueryEngine::new(store);

        let output = engine.symbol("nonexistent", None).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert_eq!(parsed["status"], "not_found");
    }

    #[test]
    fn test_symbol_lang_filter() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("test.redb");
        let store = Store::open(db_path.to_str().unwrap()).unwrap();

        store.insert_entity(&make_entity("e1", "helper", "elegy-codegraph-ts")).unwrap();
        store.insert_entity(&make_entity("e2", "helper", "elegy-codegraph-rust")).unwrap();

        let engine = QueryEngine::new(store);

        // Without filter, should find both
        let output = engine.symbol("helper", None).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert_eq!(parsed["data"].as_array().unwrap().len(), 2);

        // With ts filter, should find only the TS one
        let output = engine.symbol("helper", Some("ts")).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert_eq!(parsed["data"].as_array().unwrap().len(), 1);
        assert!(parsed["data"][0]["provenance"]["extractor"]
            .as_str()
            .unwrap()
            .contains("ts"));
    }
}
