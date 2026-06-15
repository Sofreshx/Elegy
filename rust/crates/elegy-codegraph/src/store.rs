//! Redb-backed graph storage.
//!
//! Provides persistence for entities and edges with indexed lookup.

use crate::error::Result;
use crate::ir::{Edge, Entity, EntityId};

/// Placeholder storage type. Implementation deferred to wp-redb-store.
pub struct Store;

impl Store {
    /// Open or create a graph database at the given path.
    pub fn open(_path: &str) -> Result<Self> {
        // TODO: redb-backed storage (wp-redb-store)
        Ok(Store)
    }

    /// Insert an entity into the store.
    pub fn insert_entity(&self, _entity: &Entity) -> Result<()> {
        unimplemented!("redb-backed entity insertion (wp-redb-store)")
    }

    /// Insert an edge into the store.
    pub fn insert_edge(&self, _edge: &Edge) -> Result<()> {
        unimplemented!("redb-backed edge insertion (wp-redb-store)")
    }

    /// Look up an entity by ID.
    pub fn get_by_id(&self, _id: &EntityId) -> Result<Option<Entity>> {
        unimplemented!("redb-backed entity lookup (wp-redb-store)")
    }

    /// Look up entities by name.
    pub fn get_by_name(&self, _name: &str) -> Result<Vec<Entity>> {
        unimplemented!("redb-backed name lookup (wp-redb-store)")
    }

    /// Compact the database (call after bulk writes).
    pub fn compact(&self) -> Result<()> {
        unimplemented!("redb compact (wp-redb-store)")
    }
}
