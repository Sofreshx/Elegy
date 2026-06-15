//! Redb-backed graph storage.
//!
//! Provides persistence for entities and edges with indexed lookup.

use crate::error::{Error, Result};
use crate::ir::{Edge, EdgeKind, Entity, EntityId};
use redb::{
    Database, ReadableDatabase, ReadableTable, ReadableTableMetadata, TableDefinition,
    WriteTransaction,
};

// ---------------------------------------------------------------------------
// Table definitions
// ---------------------------------------------------------------------------

/// Primary entity storage: EntityId → JSON-serialized Entity.
const ENTITIES: TableDefinition<&str, &str> = TableDefinition::new("entities");

/// Name index: name → JSON array of EntityIds (many-to-one).
const ENTITIES_BY_NAME: TableDefinition<&str, &str> = TableDefinition::new("entities_by_name");

/// File-entity index: file path → EntityId of the file entity.
const FILES: TableDefinition<&str, &str> = TableDefinition::new("files");

/// Outgoing edges: src_id → JSON array of `[dst_id, edge_kind]`.
const OUTGOING: TableDefinition<&str, &str> = TableDefinition::new("outgoing");

/// Incoming edges: dst_id → JSON array of `[src_id, edge_kind]`.
const INCOMING: TableDefinition<&str, &str> = TableDefinition::new("incoming");

// ---------------------------------------------------------------------------
// Direction
// ---------------------------------------------------------------------------

/// Direction for neighbor traversal.
#[derive(Debug, Clone, Copy)]
pub enum Direction {
    /// From source to destination.
    Outgoing,
    /// From destination to source.
    Incoming,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Convert any redb error type into our crate-level [`Error`].
///
/// All redb public error types (`StorageError`, `TableError`, `DatabaseError`,
/// `TransactionError`, `CommitError`, `CompactionError`) implement
/// `Into<redb::Error>`.
fn map_err<E: Into<redb::Error>>(e: E) -> Error {
    Error::Storage(e.into())
}

/// Helper: open every known table inside a write transaction, creating them
/// if they do not exist.
fn ensure_tables(txn: &WriteTransaction) -> Result<()> {
    txn.open_table(ENTITIES).map_err(map_err)?;
    txn.open_table(ENTITIES_BY_NAME).map_err(map_err)?;
    txn.open_table(FILES).map_err(map_err)?;
    txn.open_table(OUTGOING).map_err(map_err)?;
    txn.open_table(INCOMING).map_err(map_err)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Store
// ---------------------------------------------------------------------------

/// Redb-backed graph storage.
pub struct Store {
    db: Database,
}

impl Store {
    /// Open or create a graph database at the given path.
    pub fn open(path: &str) -> Result<Self> {
        let db = Database::create(path).map_err(map_err)?;
        let txn = db.begin_write().map_err(map_err)?;
        ensure_tables(&txn)?;
        txn.commit().map_err(map_err)?;
        Ok(Store { db })
    }

    /// Insert an entity into the store.
    pub fn insert_entity(&self, entity: &Entity) -> Result<()> {
        let json = serde_json::to_string(entity)?;
        let txn = self.db.begin_write().map_err(map_err)?;

        // 1. Primary entity storage
        {
            let mut table = txn.open_table(ENTITIES).map_err(map_err)?;
            table.insert(entity.id.as_str(), json.as_str()).map_err(map_err)?;
        }

        // 2. Name index (append to JSON array)
        {
            let mut table = txn.open_table(ENTITIES_BY_NAME).map_err(map_err)?;
            let mut ids: Vec<EntityId> = match table.get(entity.name.as_str()).map_err(map_err)? {
                Some(guard) => serde_json::from_str(guard.value())?,
                None => Vec::new(),
            };
            ids.push(entity.id.clone());
            let updated = serde_json::to_string(&ids)?;
            table
                .insert(entity.name.as_str(), updated.as_str())
                .map_err(map_err)?;
        }

        // 3. File-entity index (only for File-kind entities)
        {
            let mut table = txn.open_table(FILES).map_err(map_err)?;
            table
                .insert(entity.file.as_str(), entity.id.as_str())
                .map_err(map_err)?;
        }

        txn.commit().map_err(map_err)?;
        Ok(())
    }

    /// Insert an edge into the store.
    pub fn insert_edge(&self, edge: &Edge) -> Result<()> {
        let txn = self.db.begin_write().map_err(map_err)?;

        // 1. Append to outgoing table
        {
            let mut table = txn.open_table(OUTGOING).map_err(map_err)?;
            let mut edges: Vec<(EntityId, EdgeKind)> =
                match table.get(edge.src.as_str()).map_err(map_err)? {
                    Some(guard) => serde_json::from_str(guard.value())?,
                    None => Vec::new(),
                };
            edges.push((edge.dst.clone(), edge.kind.clone()));
            let updated = serde_json::to_string(&edges)?;
            table
                .insert(edge.src.as_str(), updated.as_str())
                .map_err(map_err)?;
        }

        // 2. Append to incoming table
        {
            let mut table = txn.open_table(INCOMING).map_err(map_err)?;
            let mut edges: Vec<(EntityId, EdgeKind)> =
                match table.get(edge.dst.as_str()).map_err(map_err)? {
                    Some(guard) => serde_json::from_str(guard.value())?,
                    None => Vec::new(),
                };
            edges.push((edge.src.clone(), edge.kind.clone()));
            let updated = serde_json::to_string(&edges)?;
            table
                .insert(edge.dst.as_str(), updated.as_str())
                .map_err(map_err)?;
        }

        txn.commit().map_err(map_err)?;
        Ok(())
    }

    /// Look up an entity by ID.
    pub fn get_by_id(&self, id: &EntityId) -> Result<Option<Entity>> {
        let txn = self.db.begin_read().map_err(map_err)?;
        let table = txn.open_table(ENTITIES).map_err(map_err)?;
        match table.get(id.as_str()).map_err(map_err)? {
            Some(guard) => Ok(Some(serde_json::from_str(guard.value())?)),
            None => Ok(None),
        }
    }

    /// Look up entities by name.
    pub fn get_by_name(&self, name: &str) -> Result<Vec<Entity>> {
        let txn = self.db.begin_read().map_err(map_err)?;

        // 1. Read the ID list from the name index
        let ids: Vec<EntityId> = {
            let table = txn.open_table(ENTITIES_BY_NAME).map_err(map_err)?;
            match table.get(name).map_err(map_err)? {
                Some(guard) => serde_json::from_str(guard.value())?,
                None => return Ok(Vec::new()),
            }
        };

        // 2. Resolve each ID
        let entity_table = txn.open_table(ENTITIES).map_err(map_err)?;
        let mut results = Vec::with_capacity(ids.len());
        for id in &ids {
            if let Some(guard) = entity_table.get(id.as_str()).map_err(map_err)? {
                results.push(serde_json::from_str(guard.value())?);
            }
        }
        Ok(results)
    }

    /// Get neighbors of an entity in the given direction.
    ///
    /// Returns `(neighbor_entity, edge_kind)` pairs where `neighbor_entity`
    /// is the entity at the *other* end of each edge.
    pub fn get_neighbors(
        &self,
        id: &EntityId,
        direction: Direction,
    ) -> Result<Vec<(Entity, EdgeKind)>> {
        let txn = self.db.begin_read().map_err(map_err)?;

        // 1. Read edge list from the relevant table
        let edge_table = match direction {
            Direction::Outgoing => txn.open_table(OUTGOING).map_err(map_err)?,
            Direction::Incoming => txn.open_table(INCOMING).map_err(map_err)?,
        };
        // The edge_table is either ReadOnlyTable or Table, both of which we
        // need to bind as a &dyn ReadableTable. Since they are concrete types
        // we just keep them alive through the scope and use them directly.
        let edges: Vec<(EntityId, EdgeKind)> =
            match edge_table.get(id.as_str()).map_err(map_err)? {
                Some(guard) => serde_json::from_str(guard.value())?,
                None => return Ok(Vec::new()),
            };

        // 2. Resolve each neighbor entity
        let entity_table = txn.open_table(ENTITIES).map_err(map_err)?;
        let mut results = Vec::with_capacity(edges.len());
        for (neighbor_id, kind) in &edges {
            match entity_table.get(neighbor_id.as_str()).map_err(map_err)? {
                Some(guard) => {
                    results.push((serde_json::from_str(guard.value())?, kind.clone()));
                }
                None => {
                    return Err(Error::NotFound(format!(
                        "Neighbor entity {} not found",
                        neighbor_id
                    )));
                }
            }
        }
        Ok(results)
    }

    /// Get all entities associated with a file path.
    ///
    /// Iterates the primary entity table and filters by the `file` field.
    pub fn get_file_entities(&self, path: &str) -> Result<Vec<Entity>> {
        let txn = self.db.begin_read().map_err(map_err)?;
        let table = txn.open_table(ENTITIES).map_err(map_err)?;
        let mut results = Vec::new();
        for result in table.iter().map_err(map_err)? {
            let (_key, value) = result.map_err(map_err)?;
            let entity: Entity = serde_json::from_str(value.value())?;
            if entity.file == path {
                results.push(entity);
            }
        }
        Ok(results)
    }

    /// Compact the database (call after bulk writes).
    pub fn compact(&mut self) -> Result<()> {
        self.db.compact().map_err(map_err)?;
        Ok(())
    }

    /// Count all entities in the store.
    pub fn count_entities(&self) -> Result<usize> {
        let txn = self.db.begin_read().map_err(map_err)?;
        let table = txn.open_table(ENTITIES).map_err(map_err)?;
        Ok(table.len().map_err(map_err)? as usize)
    }

    /// Count all edges in the store (sum of outgoing edge lists).
    pub fn count_edges(&self) -> Result<usize> {
        let txn = self.db.begin_read().map_err(map_err)?;
        let table = txn.open_table(OUTGOING).map_err(map_err)?;
        let iter = table.iter().map_err(map_err)?;
        let mut count = 0usize;
        for item in iter {
            let (_key, value) = item.map_err(map_err)?;
            let edges: Vec<(String, crate::ir::EdgeKind)> = serde_json::from_str(value.value())?;
            count += edges.len();
        }
        Ok(count)
    }

    /// Get all entities in the store.
    pub fn all_entities(&self) -> Result<Vec<crate::ir::Entity>> {
        let txn = self.db.begin_read().map_err(map_err)?;
        let table = txn.open_table(ENTITIES).map_err(map_err)?;
        let iter = table.iter().map_err(map_err)?;
        let mut entities = Vec::new();
        for item in iter {
            let (_key, value) = item.map_err(map_err)?;
            entities.push(serde_json::from_str(value.value())?);
        }
        Ok(entities)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{Confidence, EdgeKind, Entity, EntityKind, Provenance};
    use std::fs;

    /// Create a temporary database path (randomised so tests do not collide).
    fn temp_db_path() -> std::path::PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!("elegy-codegraph-test-{}.redb", uuid::Uuid::new_v4()));
        p
    }

    /// Minimal entity factory for use in tests.
    fn make_entity(
        id: &str,
        name: &str,
        qualified_name: &str,
        kind: EntityKind,
        file: &str,
    ) -> Entity {
        Entity {
            id: id.to_string(),
            kind,
            layer: "source".into(),
            name: name.to_string(),
            qualified_name: qualified_name.to_string(),
            file: file.to_string(),
            span: None,
            inputs: vec![],
            outputs: vec![],
            side_effects: vec![],
            dependencies: vec![],
            tests: vec![],
            docs: vec![],
            provenance: Provenance {
                extractor: "test".into(),
                confidence: Confidence::Exact,
                evidence_refs: vec![],
            },
        }
    }

    #[test]
    fn test_open_and_insert_entity() {
        let path = temp_db_path();
        let store = Store::open(path.to_str().unwrap()).expect("open");
        let entity = make_entity("id-1", "MyFunc", "mod::MyFunc", EntityKind::Function, "lib.rs");
        store.insert_entity(&entity).expect("insert_entity");

        let found = store.get_by_id(&"id-1".into()).expect("get_by_id");
        assert!(found.is_some());
        let found = found.unwrap();
        assert_eq!(found.name, "MyFunc");
        assert_eq!(found.qualified_name, "mod::MyFunc");
        assert_eq!(found.kind, EntityKind::Function);

        // Cleanup
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn test_insert_and_get_by_name() {
        let path = temp_db_path();
        let store = Store::open(path.to_str().unwrap()).expect("open");
        let e1 = make_entity("id-a", "common", "a::common", EntityKind::Function, "a.rs");
        let e2 = make_entity("id-b", "common", "b::common", EntityKind::Function, "b.rs");
        store.insert_entity(&e1).expect("insert e1");
        store.insert_entity(&e2).expect("insert e2");

        let results = store.get_by_name("common").expect("get_by_name");
        assert_eq!(results.len(), 2);
        let names: std::collections::BTreeSet<&str> =
            results.iter().map(|e| e.qualified_name.as_str()).collect();
        assert!(names.contains("a::common"));
        assert!(names.contains("b::common"));

        // Non-existent name
        let empty = store.get_by_name("nope").expect("get_by_name none");
        assert!(empty.is_empty());

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn test_insert_edge_and_get_neighbors() {
        let path = temp_db_path();
        let store = Store::open(path.to_str().unwrap()).expect("open");

        let src = make_entity("src", "caller", "mod::caller", EntityKind::Function, "main.rs");
        let dst = make_entity("dst", "callee", "mod::callee", EntityKind::Function, "lib.rs");
        store.insert_entity(&src).expect("insert src");
        store.insert_entity(&dst).expect("insert dst");

        let edge = Edge {
            src: "src".into(),
            dst: "dst".into(),
            kind: EdgeKind::Calls,
            provenance: Provenance {
                extractor: "test".into(),
                confidence: Confidence::Exact,
                evidence_refs: vec![],
            },
        };
        store.insert_edge(&edge).expect("insert_edge");

        // Outgoing from src
        let outgoing = store
            .get_neighbors(&"src".into(), Direction::Outgoing)
            .expect("get_neighbors outgoing");
        assert_eq!(outgoing.len(), 1);
        assert_eq!(outgoing[0].0.id, "dst");
        assert_eq!(outgoing[0].1, EdgeKind::Calls);

        // Incoming to dst
        let incoming = store
            .get_neighbors(&"dst".into(), Direction::Incoming)
            .expect("get_neighbors incoming");
        assert_eq!(incoming.len(), 1);
        assert_eq!(incoming[0].0.id, "src");
        assert_eq!(incoming[0].1, EdgeKind::Calls);

        // No edges for a different ID
        let none = store
            .get_neighbors(&"other".into(), Direction::Outgoing)
            .expect("get_neighbors other");
        assert!(none.is_empty());

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn test_compact() {
        let path = temp_db_path();
        let mut store = Store::open(path.to_str().unwrap()).expect("open");

        let e = make_entity("compact-me", "C", "mod::C", EntityKind::Class, "comp.rs");
        store.insert_entity(&e).expect("insert");

        // compact returns Ok(true) when work was done, Ok(false) if no further
        // compaction is possible. Either is fine for this test.
        let _ = store.compact().expect("compact");

        // DB is still readable after compact
        let found = store.get_by_id(&"compact-me".into()).expect("get_by_id");
        assert!(found.is_some());

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn test_missing_entity_returns_none() {
        let path = temp_db_path();
        let store = Store::open(path.to_str().unwrap()).expect("open");

        let result = store.get_by_id(&"does-not-exist".into());
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());

        let _ = fs::remove_file(&path);
    }
}
