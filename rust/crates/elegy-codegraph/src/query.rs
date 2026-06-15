//! Query engine for the code graph.
//!
//! Provides symbol lookup, neighbor traversal, impact analysis, and
//! structural summary queries.

use crate::error::Result;
use crate::ir::EdgeKind;
use crate::ir::Entity;
use crate::store::Direction;
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

    /// Get neighbors of an entity.
    ///
    /// Returns all entities connected to the given entity by edges,
    /// in the specified direction (`in` or `out`). Each result includes
    /// the neighbor entity and the edge kind.
    pub fn neighbors(&self, id: &str, direction: &str) -> Result<String> {
        let dir = match direction {
            "out" | "outgoing" => Direction::Outgoing,
            "in" | "incoming" => Direction::Incoming,
            other => {
                return Ok(serde_json::to_string_pretty(
                    &QueryResult::<()>::error(format!(
                        "Invalid direction '{}'. Use 'in' or 'out'.",
                        other
                    )),
                )?);
            }
        };

        let results = self.store.get_neighbors(&id.to_string(), dir)?;

        if results.is_empty() {
            Ok(serde_json::to_string_pretty(
                &QueryResult::<Vec<(crate::ir::Entity, EdgeKind)>>::not_found(format!(
                    "No neighbors found for entity '{}' in direction '{}'",
                    id, direction
                )),
            )?)
        } else {
            // Serialize as array of {entity, edgeKind}
            let output: Vec<serde_json::Value> = results
                .iter()
                .map(|(entity, kind)| {
                    serde_json::json!({
                        "entity": entity,
                        "edgeKind": kind,
                    })
                })
                .collect();
            Ok(serde_json::to_string_pretty(
                &QueryResult::ok(&output),
            )?)
        }
    }

    /// Analyze the impact of changes to a file.
    ///
    /// Returns all entities in the given file plus their immediate
    /// outgoing neighbors (calls, references, exports). Delta-only in v0.
    pub fn impact(&self, path: &str) -> Result<String> {
        let all = self.store.all_entities()?;
        let file_entities: Vec<_> = all.iter().filter(|e| e.file == path).collect();

        if file_entities.is_empty() {
            return Ok(serde_json::to_string_pretty(
                &QueryResult::<()>::not_found(format!(
                    "No entities found in file '{}'", path
                )),
            )?);
        }

        let mut impacted = Vec::new();
        for entity in &file_entities {
            let neighbors = self
                .store
                .get_neighbors(&entity.id, crate::store::Direction::Outgoing)?;
            for (neighbor, kind) in &neighbors {
                impacted.push(serde_json::json!({
                    "source": entity,
                    "target": neighbor,
                    "relationship": kind,
                }));
            }
        }

        let output = serde_json::json!({
            "file": path,
            "entityCount": file_entities.len(),
            "impactCount": impacted.len(),
            "entities": file_entities,
            "impacts": impacted,
        });

        Ok(serde_json::to_string_pretty(&QueryResult::ok(&output))?)
    }

    /// Get a structural summary of the repository.
    ///
    /// Returns counts by entity kind, edge kind, file count, and
    /// extractor metadata — useful for agents to get a quick overview.
    pub fn summary(&self) -> Result<String> {
        let entities = self.store.all_entities()?;
        let total_entities = entities.len();
        let total_edges = self.store.count_edges()?;

        // Count entities by kind
        let mut kind_counts: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();
        let mut file_set: std::collections::HashSet<String> =
            std::collections::HashSet::new();
        for entity in &entities {
            let kind = serde_json::to_string(&entity.kind)
                .unwrap_or_else(|_| "unknown".to_string())
                .trim_matches('"')
                .to_string();
            *kind_counts.entry(kind).or_default() += 1;
            file_set.insert(entity.file.clone());
        }

        let summary = serde_json::json!({
            "entityCount": total_entities,
            "edgeCount": total_edges,
            "fileCount": file_set.len(),
            "entitiesByKind": kind_counts,
            "sampleEntities": entities.iter().take(5).collect::<Vec<_>>(),
        });

        Ok(serde_json::to_string_pretty(&QueryResult::ok(&summary))?)
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

    #[test]
    fn test_neighbors_query() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("test.redb");
        let store = Store::open(db_path.to_str().unwrap()).unwrap();

        let src = make_entity("src1", "caller", "elegy-codegraph-ts");
        let dst = make_entity("dst1", "callee", "elegy-codegraph-ts");
        store.insert_entity(&src).unwrap();
        store.insert_entity(&dst).unwrap();

        store.insert_edge(&crate::ir::Edge {
            src: "src1".into(),
            dst: "dst1".into(),
            kind: crate::ir::EdgeKind::Calls,
            provenance: crate::ir::Provenance {
                extractor: "test".into(),
                confidence: crate::ir::Confidence::Exact,
                evidence_refs: vec![],
            },
        }).unwrap();

        let engine = QueryEngine::new(store);
        let output = engine.neighbors("src1", "out").unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert_eq!(parsed["status"], "ok");
        assert_eq!(parsed["data"].as_array().unwrap().len(), 1);
        assert_eq!(parsed["data"][0]["entity"]["name"], "callee");
        assert_eq!(parsed["data"][0]["edgeKind"], "calls");
    }

    #[test]
    fn test_neighbors_invalid_direction() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("test.redb");
        let store = Store::open(db_path.to_str().unwrap()).unwrap();
        let engine = QueryEngine::new(store);

        let output = engine.neighbors("any", "sideways").unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert_eq!(parsed["status"], "error");
    }

    #[test]
    fn test_summary_query() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("test.redb");
        let store = Store::open(db_path.to_str().unwrap()).unwrap();

        store.insert_entity(&make_entity("e1", "add", "elegy-codegraph-ts")).unwrap();
        store.insert_entity(&make_entity("e2", "sub", "elegy-codegraph-ts")).unwrap();

        let engine = QueryEngine::new(store);
        let output = engine.summary().unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert_eq!(parsed["status"], "ok");
        assert_eq!(parsed["data"]["entityCount"], 2);
        assert_eq!(parsed["data"]["fileCount"], 1);
    }

    #[test]
    fn test_impact_query() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("test.redb");
        let store = Store::open(db_path.to_str().unwrap()).unwrap();

        let src = make_entity("src1", "caller", "elegy-codegraph-ts");
        let mut dst = make_entity("dst1", "callee", "elegy-codegraph-ts");
        dst.file = "other.rs".to_string();
        store.insert_entity(&src).unwrap();
        store.insert_entity(&dst).unwrap();

        store.insert_edge(&crate::ir::Edge {
            src: "src1".into(),
            dst: "dst1".into(),
            kind: crate::ir::EdgeKind::Calls,
            provenance: crate::ir::Provenance {
                extractor: "test".into(),
                confidence: crate::ir::Confidence::Exact,
                evidence_refs: vec![],
            },
        }).unwrap();

        let engine = QueryEngine::new(store);
        let output = engine.impact("lib.rs").unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert_eq!(parsed["status"], "ok");
        assert_eq!(parsed["data"]["entityCount"], 1);
        assert_eq!(parsed["data"]["impactCount"], 1);
    }

    #[test]
    fn test_impact_not_found() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("test.redb");
        let store = Store::open(db_path.to_str().unwrap()).unwrap();
        let engine = QueryEngine::new(store);

        let output = engine.impact("nonexistent.ts").unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert_eq!(parsed["status"], "not_found");
    }
}
