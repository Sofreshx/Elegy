//! Query engine for the code graph.
//!
//! Provides symbol lookup, neighbor traversal, impact analysis, and
//! structural summary queries.

use crate::error::Result;
use crate::ir::EntityId;
use crate::store::Store;

/// Query the graph store.
pub struct QueryEngine {
    #[allow(dead_code)]
    store: Store,
}

impl QueryEngine {
    /// Create a new query engine backed by the given store.
    pub fn new(store: Store) -> Self {
        Self { store }
    }

    /// Look up a symbol by name.
    pub fn symbol(&self, _name: &str, _lang: Option<&str>) -> Result<serde_json::Value> {
        unimplemented!("symbol query (wp-query-symbol)")
    }

    /// Get neighbors of an entity.
    pub fn neighbors(&self, _id: &EntityId, _direction: &str) -> Result<serde_json::Value> {
        unimplemented!("neighbors query (wp-query-neighbors)")
    }

    /// Analyze the impact of changes to a file.
    pub fn impact(&self, _path: &str) -> Result<serde_json::Value> {
        unimplemented!("impact query (wp-query-impact)")
    }

    /// Get a structural summary of the repository.
    pub fn summary(&self) -> Result<serde_json::Value> {
        unimplemented!("summary query (wp-query-summary)")
    }
}
