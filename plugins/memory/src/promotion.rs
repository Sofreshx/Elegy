use crate::{Memory, MemoryId, MemoryScope, SqliteMemoryStore, StoreError};

/// Lightweight promotion orchestrator for automatic and manual scope promotion.
#[derive(Debug, Default, Clone, Copy)]
pub struct PromotionEngine;

impl PromotionEngine {
    /// Evaluate automatic promotion criteria for the store's visible scopes.
    pub fn run(
        &self,
        store: &SqliteMemoryStore,
        limit: Option<usize>,
        trigger_session_id: Option<&str>,
    ) -> Result<Vec<Memory>, StoreError> {
        store.run_promotion_pass(limit, trigger_session_id)
    }

    /// Promote a single memory to an explicit broader scope.
    pub fn promote_to(
        &self,
        store: &SqliteMemoryStore,
        id: &MemoryId,
        to_scope: MemoryScope,
        changed_by: &str,
        reason: &str,
        trigger_session_id: Option<&str>,
    ) -> Result<Option<Memory>, StoreError> {
        store.promote_memory_to(id, to_scope, changed_by, reason, trigger_session_id)
    }
}
