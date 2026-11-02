// Shared-access coverage is maintained in this child module so it can use the
// parent module's private repository helpers without widening production APIs.

use super::*;

use async_trait::async_trait;
use elegy_memory::{EmbeddingError, EmbeddingProvider, MemoryStore, SqliteMemoryStore};
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use tempfile::TempDir;

fn binding(agent: &str) -> MemoryBinding {
    MemoryBinding::new("fictional-team", agent)
        .expect("binding")
        .with_shared_agents(vec!["agent-a".into(), "agent-b".into()])
        .expect("shared binding")
}

fn memory(content: &str, owner: &str) -> Memory {
    Memory {
        id: Uuid::new_v4(),
        content: content.into(),
        summary: None,
        scope: MemoryScope::Agent,
        memory_type: MemoryType::Fact,
        provenance: ProvenanceLevel::UserStated,
        importance_score: 0.8,
        reliability_score: ProvenanceLevel::UserStated.base_reliability(),
        sensitivity: SensitivityLevel::Low,
        state: MemoryState::Active,
        tags: Vec::new(),
        status: None,
        custom_metadata: HashMap::new(),
        access_count: 0,
        corroboration_count: 0,
        embedding_stale: true,
        created_at: "2026-05-10T12:00:00Z".parse().expect("fixture timestamp"),
        updated_at: "2026-05-10T12:00:00Z".parse().expect("fixture timestamp"),
        last_accessed_at: None,
        tenant_id: None,
        user_id: None,
        agent_id: Some(owner.into()),
    }
}

async fn seed(repository: &MemoryRepository, value: Memory) -> Uuid {
    let id = value.id;
    repository.store.store(value).await.expect("seed");
    id
}

fn args<T: serde::de::DeserializeOwned>(value: serde_json::Value) -> T {
    serde_json::from_value(value).expect("args")
}

#[test]
fn shared_agents_allowlist_rejects_invalid_lists() {
    assert!(MemoryBinding::new("fictional-team", "agent-a")
        .unwrap()
        .with_shared_agents(vec![])
        .is_err());
    assert!(MemoryBinding::new("fictional-team", "agent-a")
        .unwrap()
        .with_shared_agents(vec!["agent-b".into()])
        .is_err());
    assert!(MemoryBinding::new("fictional-team", "agent-a")
        .unwrap()
        .with_shared_agents(vec!["agent-a".into(), "".into()])
        .is_err());
    assert!(MemoryBinding::new("fictional-team", "agent-a")
        .unwrap()
        .with_shared_agents(vec!["agent-a".into(), "agent-a".into()])
        .is_err());
}

#[tokio::test]
async fn default_binding_remains_private() {
    let temp = TempDir::new().unwrap();
    let repository =
        MemoryRepository::new(temp.path().join("memory.db"), MemoryBinding::default()).unwrap();
    let id = seed(&repository, memory("private other", "agent-b")).await;
    assert!(repository.recall(&id.to_string()).await.unwrap().is_none());
}

#[tokio::test]
async fn shared_readers_can_list_recall_search_and_stats() {
    let temp = TempDir::new().unwrap();
    let repository =
        MemoryRepository::new(temp.path().join("memory.db"), binding("agent-a")).unwrap();
    let id = seed(&repository, memory("shared distinctive nebula", "agent-b")).await;
    assert!(repository
        .list(&args(json!({"limit": 50})))
        .await
        .unwrap()
        .iter()
        .any(|m| m.id == id));
    assert_eq!(
        repository
            .recall(&id.to_string())
            .await
            .unwrap()
            .unwrap()
            .id,
        id
    );
    let search_args: MemorySearchArgs = args(json!({"query":"distinctive nebula", "limit":10}));
    let matches = repository.search(&search_args).await.unwrap();
    assert!(matches.iter().any(|m| m.memory.id == id));
    let search_json = serde_json::to_value(MemorySearchResponse::new(
        &repository,
        &search_args,
        matches,
    ))
    .unwrap();
    assert_eq!(
        search_json["results"][0]["agentId"].as_str(),
        Some("agent-b")
    );
    let list_args: MemoryListArgs = args(json!({"limit":50}));
    let list = repository.list(&list_args).await.unwrap();
    let list_json =
        serde_json::to_value(MemoryListResponse::new(&repository, &list_args, list)).unwrap();
    assert_eq!(
        list_json["memories"][0]["agentId"].as_str(),
        Some("agent-b")
    );
    assert_eq!(repository.stats().await.unwrap().total_count, 1);
}

#[tokio::test]
async fn shared_reads_exclude_tenant_user_and_higher_scope_rows() {
    let temp = TempDir::new().unwrap();
    let repository = MemoryRepository::new(
        temp.path().join("memory.db"),
        binding("agent-a").with_read_scope(MemoryScope::Session),
    )
    .unwrap();
    let visible = seed(&repository, memory("shared row", "agent-b")).await;
    let mut tenant = memory("tenant hidden", "agent-b");
    tenant.tenant_id = Some("t".into());
    let tenant_id = seed(&repository, tenant).await;
    let mut user = memory("user hidden", "agent-b");
    user.user_id = Some("u".into());
    let user_id = seed(&repository, user).await;
    let store =
        SqliteMemoryStore::new(temp.path().join("memory.db"), MemoryScope::Workspace).unwrap();
    let mut higher = memory("workspace hidden", "agent-b");
    higher.scope = MemoryScope::Workspace;
    let higher_id = higher.id;
    store.store(higher).await.unwrap();
    let ids = repository
        .list(&args(json!({"limit":50})))
        .await
        .unwrap()
        .into_iter()
        .map(|m| m.id)
        .collect::<Vec<_>>();
    assert!(ids.contains(&visible));
    assert!(!ids.contains(&tenant_id));
    assert!(!ids.contains(&user_id));
    assert!(!ids.contains(&higher_id));
    for hidden_id in [tenant_id, user_id, higher_id] {
        assert!(repository
            .recall(&hidden_id.to_string())
            .await
            .unwrap()
            .is_none());
    }
    let results = repository
        .search(&args(json!({"query":"hidden", "limit":50})))
        .await
        .unwrap();
    assert!(!results
        .iter()
        .any(|row| [tenant_id, user_id, higher_id].contains(&row.memory.id)));
    assert_eq!(repository.stats().await.unwrap().total_count, 1);
}

#[tokio::test]
async fn shared_rows_are_read_only_to_nonowners() {
    let temp = TempDir::new().unwrap();
    let repository =
        MemoryRepository::new(temp.path().join("memory.db"), binding("agent-a")).unwrap();
    let id = seed(&repository, memory("shared immutable", "agent-b")).await;
    let before = repository.store.get_raw(&id).await.unwrap().unwrap();
    assert!(repository
        .update_memory(&args(json!({"id":id.to_string(),"content":"changed"})))
        .await
        .is_err());
    assert!(repository
        .correct_memory(&args(json!({"id":id.to_string(),"content":"corrected"})))
        .await
        .is_err());
    assert!(repository
        .delete_memory(&args(json!({"id":id.to_string()})))
        .await
        .is_err());
    let after = repository.store.get_raw(&id).await.unwrap().unwrap();
    assert_eq!(
        serde_json::to_value(before).unwrap(),
        serde_json::to_value(after).unwrap()
    );
}

#[tokio::test]
async fn own_correction_does_not_merge_another_allowed_owner() {
    let temp = TempDir::new().unwrap();
    let repository =
        MemoryRepository::new(temp.path().join("memory.db"), binding("agent-a")).unwrap();
    let other = seed(&repository, memory("same correction text", "agent-b")).await;
    let own = seed(&repository, memory("own original", "agent-a")).await;
    let other_before = repository.store.get_raw(&other).await.unwrap().unwrap();
    repository
        .correct_memory(&args(
            json!({"id":own.to_string(),"content":"same correction text"}),
        ))
        .await
        .unwrap();
    let row = repository.store.get_raw(&other).await.unwrap().unwrap();
    assert_eq!(
        serde_json::to_value(other_before).unwrap(),
        serde_json::to_value(row).unwrap()
    );
    let own_row = repository.store.get_raw(&own).await.unwrap().unwrap();
    assert_eq!(own_row.state, MemoryState::Active);
}

#[tokio::test]
async fn consolidate_does_not_change_other_allowed_owner() {
    let temp = TempDir::new().unwrap();
    let repository =
        MemoryRepository::new(temp.path().join("memory.db"), binding("agent-a")).unwrap();
    let first = seed(&repository, memory("duplicate shared fact", "agent-b")).await;
    let second = seed(&repository, memory("duplicate shared fact", "agent-b")).await;
    let before_first = repository.store.get_raw(&first).await.unwrap().unwrap();
    let before_second = repository.store.get_raw(&second).await.unwrap().unwrap();
    repository
        .consolidate_memories(&MemoryConsolidateArgs::default())
        .await
        .unwrap();
    let after_first = repository.store.get_raw(&first).await.unwrap().unwrap();
    let after_second = repository.store.get_raw(&second).await.unwrap().unwrap();
    assert_eq!(
        serde_json::to_value(before_first).unwrap(),
        serde_json::to_value(after_first).unwrap()
    );
    assert_eq!(
        serde_json::to_value(before_second).unwrap(),
        serde_json::to_value(after_second).unwrap()
    );
}

#[tokio::test]
async fn store_retains_owner_and_response_shape() {
    let temp = TempDir::new().unwrap();
    let repository =
        MemoryRepository::new(temp.path().join("memory.db"), binding("agent-a")).unwrap();
    let response = repository
        .store_memory(&args(json!({"content":"owned durable fact"})))
        .await
        .unwrap();
    let id = Uuid::parse_str(&response.memory.id).unwrap();
    assert_eq!(
        repository
            .store
            .get_raw(&id)
            .await
            .unwrap()
            .unwrap()
            .agent_id
            .as_deref(),
        Some("agent-a")
    );
    let serialized = serde_json::to_value(response.memory).unwrap();
    assert_eq!(
        serialized.get("agentId").and_then(|value| value.as_str()),
        Some("agent-a")
    );
}

#[tokio::test]
async fn search_does_not_touch_hidden_decoy_access_count_or_dates() {
    let temp = TempDir::new().unwrap();
    let repository =
        MemoryRepository::new(temp.path().join("memory.db"), binding("agent-a")).unwrap();
    let visible = seed(&repository, memory("distinctive comet visible", "agent-b")).await;
    let hidden = seed(&repository, memory("distinctive comet hidden", "agent-c")).await;
    let before = repository.store.get_raw(&hidden).await.unwrap().unwrap();
    let results = repository
        .search(&args(json!({"query":"distinctive comet", "limit":10})))
        .await
        .unwrap();
    assert!(results.iter().any(|r| r.memory.id == visible));
    assert!(!results.iter().any(|r| r.memory.id == hidden));
    let after = repository.store.get_raw(&hidden).await.unwrap().unwrap();
    assert_eq!(
        (
            before.access_count,
            before.updated_at,
            before.last_accessed_at
        ),
        (after.access_count, after.updated_at, after.last_accessed_at)
    );
}

#[tokio::test]
async fn own_tenant_and_user_rows_remain_readable_and_writable() {
    for shared in [false, true] {
        let temp = TempDir::new().unwrap();
        let binding = if shared {
            binding("agent-a")
        } else {
            MemoryBinding::new("fictional-team", "agent-a").unwrap()
        };
        let repository = MemoryRepository::new(temp.path().join("memory.db"), binding).unwrap();
        let mut own_tenant = memory("own tenant row", "agent-a");
        own_tenant.tenant_id = Some("tenant-a".into());
        let own_tenant_id = seed(&repository, own_tenant).await;
        let mut own_user = memory("own user row", "agent-a");
        own_user.user_id = Some("user-a".into());
        let own_user_id = seed(&repository, own_user).await;
        let mut foreign_tenant = memory("foreign tenant row", "agent-b");
        foreign_tenant.tenant_id = Some("tenant-a".into());
        let foreign_tenant_id = seed(&repository, foreign_tenant).await;
        let mut foreign_user = memory("foreign user row", "agent-b");
        foreign_user.user_id = Some("user-a".into());
        let foreign_user_id = seed(&repository, foreign_user).await;
        for id in [own_tenant_id, own_user_id] {
            assert!(repository.recall(&id.to_string()).await.unwrap().is_some());
            repository
                .update_memory(&args(
                    json!({"id": id.to_string(), "content": "updated own row"}),
                ))
                .await
                .unwrap();
        }
        let own_search_ids = repository
            .search(&args(json!({"query": "updated own row", "limit": 10})))
            .await
            .unwrap()
            .into_iter()
            .map(|row| row.memory.id)
            .collect::<Vec<_>>();
        assert!(own_search_ids.contains(&own_tenant_id));
        assert!(own_search_ids.contains(&own_user_id));
        for id in [foreign_tenant_id, foreign_user_id] {
            assert!(repository.recall(&id.to_string()).await.unwrap().is_none());
        }
        assert!(!own_search_ids.contains(&foreign_tenant_id));
        assert!(!own_search_ids.contains(&foreign_user_id));
    }
}

#[tokio::test]
async fn own_correction_does_not_merge_same_text_from_foreign_wider_scope() {
    let temp = TempDir::new().unwrap();
    let repository =
        MemoryRepository::new(temp.path().join("memory.db"), binding("agent-a")).unwrap();
    let own = seed(&repository, memory("own original", "agent-a")).await;
    let mut foreign_workspace = memory("same corrected text", "agent-b");
    foreign_workspace.scope = MemoryScope::Workspace;
    let workspace_id = foreign_workspace.id;
    let workspace_store =
        SqliteMemoryStore::new(temp.path().join("memory.db"), MemoryScope::Workspace).unwrap();
    workspace_store.store(foreign_workspace).await.unwrap();
    let mut foreign_user = memory("same corrected text", "agent-b");
    foreign_user.scope = MemoryScope::User;
    let user_id = foreign_user.id;
    let user_store =
        SqliteMemoryStore::new(temp.path().join("memory.db"), MemoryScope::User).unwrap();
    user_store.store(foreign_user).await.unwrap();
    let before_workspace = workspace_store
        .get_raw(&workspace_id)
        .await
        .unwrap()
        .unwrap();
    let before_user = user_store.get_raw(&user_id).await.unwrap().unwrap();
    let correction = repository
        .correct_memory(&args(
            json!({"id": own.to_string(), "content": "same corrected text"}),
        ))
        .await
        .unwrap();
    assert_eq!(correction.correction.disposition, "applied");
    assert_eq!(
        repository.store.get_raw(&own).await.unwrap().unwrap().state,
        MemoryState::Active
    );
    assert_eq!(
        serde_json::to_value(before_workspace).unwrap(),
        serde_json::to_value(
            workspace_store
                .get_raw(&workspace_id)
                .await
                .unwrap()
                .unwrap()
        )
        .unwrap()
    );
    assert_eq!(
        serde_json::to_value(before_user).unwrap(),
        serde_json::to_value(user_store.get_raw(&user_id).await.unwrap().unwrap()).unwrap()
    );
    assert_ne!(
        correction.correction.related_memory_id,
        Some(workspace_id.to_string())
    );
    assert_ne!(
        correction.correction.related_memory_id,
        Some(user_id.to_string())
    );
}

#[derive(Debug, Default)]
struct FixedEmbeddingProvider;

#[async_trait]
impl EmbeddingProvider for FixedEmbeddingProvider {
    async fn embed(&self, _text: &str) -> Result<Vec<f32>, EmbeddingError> {
        Ok(axis_embedding())
    }
    fn dimensions(&self) -> usize {
        768
    }
    fn model_id(&self) -> &str {
        "shared-access-test"
    }
}

fn axis_embedding() -> Vec<f32> {
    let mut embedding = vec![0.0; 768];
    embedding[0] = 1.0;
    embedding
}

#[tokio::test]
async fn vector_search_authorizes_only_allowed_owner_and_leaves_hidden_decoys_untouched() {
    let temp = TempDir::new().unwrap();
    let repository = MemoryRepository::new_with_embedding_provider(
        temp.path().join("memory.db"),
        binding("agent-a"),
        Arc::new(FixedEmbeddingProvider),
    )
    .unwrap();
    let allowed = seed(&repository, memory("allowed vector row", "agent-b")).await;
    repository
        .store
        .store_embedding(&allowed, &axis_embedding())
        .await
        .unwrap();
    let hidden = seed(&repository, memory("hidden vector decoy", "agent-c")).await;
    repository
        .store
        .store_embedding(&hidden, &axis_embedding())
        .await
        .unwrap();
    let before = repository.store.get_raw(&hidden).await.unwrap().unwrap();
    let results = repository
        .search(&args(json!({"query":"unrelated query words", "limit": 10})))
        .await
        .unwrap();
    assert!(results.iter().any(|row| row.memory.id == allowed));
    assert!(!results.iter().any(|row| row.memory.id == hidden));
    let after = repository.store.get_raw(&hidden).await.unwrap().unwrap();
    assert_eq!(
        serde_json::to_value(before).unwrap(),
        serde_json::to_value(after).unwrap()
    );
}

#[tokio::test]
async fn own_update_and_correction_history_records_actor_authorship() {
    let temp = TempDir::new().unwrap();
    let repository =
        MemoryRepository::new(temp.path().join("memory.db"), binding("agent-a")).unwrap();
    let id = seed(&repository, memory("history original", "agent-a")).await;
    repository
        .update_memory(&args(
            json!({"id": id.to_string(), "content": "history updated"}),
        ))
        .await
        .unwrap();
    repository
        .correct_memory(&args(
            json!({"id": id.to_string(), "content": "history corrected"}),
        ))
        .await
        .unwrap();
    assert!(repository
        .store
        .list_versions(&id)
        .unwrap()
        .iter()
        .any(|version| version.changed_by == "agent:agent-a"));
    assert!(repository
        .store
        .list_corrections(Some(&id), 50)
        .unwrap()
        .iter()
        .any(|correction| correction.corrected_by == "agent:agent-a"));
}

#[tokio::test]
async fn owner_filtered_gate_ignores_foreign_exact_vector_decoy() {
    let temp = TempDir::new().unwrap();
    let repository =
        MemoryRepository::new(temp.path().join("memory.db"), binding("agent-a")).unwrap();
    let foreign = seed(&repository, memory("foreign exact vector decoy", "agent-c")).await;
    repository
        .store
        .store_embedding(&foreign, &axis_embedding())
        .await
        .unwrap();
    let before = repository.store.get_raw(&foreign).await.unwrap().unwrap();
    let candidate = MemoryCandidate {
        content: "candidate has an unrelated exact vector".into(),
        summary: None,
        memory_type: MemoryType::Fact,
        provenance: ProvenanceLevel::UserStated,
        importance_score: 0.8,
        sensitivity: SensitivityLevel::Low,
        tags: Vec::new(),
        custom_metadata: HashMap::new(),
        embedding: Some(axis_embedding()),
    };
    let gate = DefaultSalienceGate::new(repository.store.scope_config().unwrap());
    let view = repository.store.gate_view_for_agent("agent-a");
    let decision = gate.evaluate(&candidate, &view).await.unwrap();
    assert!(matches!(
        decision,
        GateDecision::Accept {
            similar_to: None,
            ..
        }
    ));
    let after = repository.store.get_raw(&foreign).await.unwrap().unwrap();
    assert_eq!(
        serde_json::to_value(before).unwrap(),
        serde_json::to_value(after).unwrap()
    );
}

#[tokio::test]
async fn provider_backed_correction_keeps_nearest_foreign_row_unchanged() {
    let temp = TempDir::new().unwrap();
    let repository = MemoryRepository::new_with_embedding_provider(
        temp.path().join("memory.db"),
        binding("agent-a"),
        Arc::new(FixedEmbeddingProvider),
    )
    .unwrap();
    let foreign = seed(&repository, memory("nearest foreign target", "agent-b")).await;
    repository
        .store
        .store_embedding(&foreign, &axis_embedding())
        .await
        .unwrap();
    let before = repository.store.get_raw(&foreign).await.unwrap().unwrap();
    let own = seed(&repository, memory("own source", "agent-a")).await;
    let correction = repository
        .correct_memory(&args(
            json!({"id": own.to_string(), "content": "nearest foreign target variation"}),
        ))
        .await
        .unwrap();
    assert_eq!(correction.correction.disposition, "applied");
    assert_eq!(
        repository.store.get_raw(&own).await.unwrap().unwrap().state,
        MemoryState::Active
    );
    let after = repository.store.get_raw(&foreign).await.unwrap().unwrap();
    assert_eq!(
        serde_json::to_value(before).unwrap(),
        serde_json::to_value(after).unwrap()
    );
    assert_ne!(
        correction.correction.related_memory_id,
        Some(foreign.to_string())
    );
}

fn angled_embedding(x: f32, y: f32) -> Vec<f32> {
    let mut embedding = vec![0.0; 768];
    embedding[0] = x;
    embedding[1] = y;
    embedding
}

#[tokio::test]
async fn shared_search_tracks_only_the_returned_page() {
    let temp = TempDir::new().unwrap();
    let repository =
        MemoryRepository::new(temp.path().join("memory.db"), binding("agent-a")).unwrap();
    let first = seed(&repository, memory("quasar visible first", "agent-a")).await;
    let second = seed(&repository, memory("quasar visible second", "agent-b")).await;
    let results = repository
        .search(&args(json!({"query":"quasar", "limit":1})))
        .await
        .unwrap();
    assert_eq!(results.len(), 1);
    for id in [first, second] {
        let row = repository.store.get_raw(&id).await.unwrap().unwrap();
        assert_eq!(row.access_count, u32::from(id == results[0].memory.id));
    }
}

#[tokio::test]
async fn hidden_vector_pool_cannot_starve_shared_or_owned_results() {
    let temp = TempDir::new().unwrap();
    let repository = MemoryRepository::new_with_embedding_provider(
        temp.path().join("memory.db"),
        binding("agent-a"),
        Arc::new(FixedEmbeddingProvider),
    )
    .unwrap();
    for index in 0..1001 {
        let hidden = seed(
            &repository,
            memory(&format!("unrelated hidden vector {index}"), "agent-c"),
        )
        .await;
        repository
            .store
            .store_embedding(&hidden, &angled_embedding(0.99, 0.1))
            .await
            .unwrap();
    }
    let allowed = seed(&repository, memory("unrelated allowed vector", "agent-b")).await;
    repository
        .store
        .store_embedding(&allowed, &angled_embedding(0.8, 0.6))
        .await
        .unwrap();
    let own = seed(&repository, memory("unrelated owned vector", "agent-a")).await;
    repository
        .store
        .store_embedding(&own, &angled_embedding(0.8, 0.6))
        .await
        .unwrap();
    let results = repository
        .search(&args(
            json!({"query": "quasar spectral retrieval", "limit": 1}),
        ))
        .await
        .unwrap();
    assert_eq!(results.len(), 1);
    assert!([allowed, own].contains(&results[0].memory.id));
    let view = repository.store.gate_view_for_agent("agent-a");
    let matches = view.find_similar(&axis_embedding(), 0.5, 1).await.unwrap();
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].memory.id, own);
}

#[tokio::test]
async fn legacy_private_search_returns_owned_and_unowned_past_hidden_vector_pool() {
    let temp = TempDir::new().unwrap();
    let repository = MemoryRepository::new_with_embedding_provider(
        temp.path().join("memory.db"),
        MemoryBinding::new("fictional-team", "agent-a")
            .unwrap()
            .including_unowned(true),
        Arc::new(FixedEmbeddingProvider),
    )
    .unwrap();
    let mut hidden_before = Vec::new();
    for index in 0..1001 {
        let hidden = seed(
            &repository,
            memory(&format!("unrelated hidden vector {index}"), "agent-c"),
        )
        .await;
        repository
            .store
            .store_embedding(&hidden, &angled_embedding(0.99, 0.1))
            .await
            .unwrap();
        hidden_before.push(repository.store.get_raw(&hidden).await.unwrap().unwrap());
    }
    let own = seed(&repository, memory("unrelated owned vector", "agent-a")).await;
    repository
        .store
        .store_embedding(&own, &angled_embedding(0.8, 0.6))
        .await
        .unwrap();
    let mut unowned = memory("unrelated unowned vector", "agent-a");
    unowned.agent_id = None;
    let unowned = seed(&repository, unowned).await;
    repository
        .store
        .store_embedding(&unowned, &angled_embedding(0.7, 0.7))
        .await
        .unwrap();

    let results = repository
        .search(&args(
            json!({"query": "quasar spectral retrieval", "limit": 2}),
        ))
        .await
        .unwrap();

    assert_eq!(results.len(), 2);
    assert!(results.iter().any(|row| row.memory.id == own));
    assert!(results.iter().any(|row| row.memory.id == unowned));
    for before in hidden_before {
        let after = repository.store.get_raw(&before.id).await.unwrap().unwrap();
        assert_eq!(
            serde_json::to_value(before).unwrap(),
            serde_json::to_value(after).unwrap()
        );
    }
}

#[tokio::test]
async fn legacy_private_search_tracks_only_the_returned_page() {
    let temp = TempDir::new().unwrap();
    let repository = MemoryRepository::new(
        temp.path().join("memory.db"),
        MemoryBinding::new("fictional-team", "agent-a")
            .unwrap()
            .including_unowned(true),
    )
    .unwrap();
    let own = seed(&repository, memory("quasar owned first", "agent-a")).await;
    let mut unowned = memory("quasar unowned second", "agent-a");
    unowned.agent_id = None;
    let unowned = seed(&repository, unowned).await;
    let foreign = seed(&repository, memory("quasar foreign decoy", "agent-c")).await;
    let mut before_rows = Vec::new();
    for id in [own, unowned, foreign] {
        before_rows.push(repository.store.get_raw(&id).await.unwrap().unwrap());
    }

    let results = repository
        .search(&args(json!({"query": "quasar", "limit": 1})))
        .await
        .unwrap();

    assert_eq!(results.len(), 1);
    assert!([own, unowned].contains(&results[0].memory.id));
    for before in before_rows {
        let after = repository.store.get_raw(&before.id).await.unwrap().unwrap();
        if before.id == results[0].memory.id {
            assert_eq!(after.access_count, before.access_count + 1);
            assert!(after.last_accessed_at.is_some());
        } else {
            assert_eq!(
                serde_json::to_value(before).unwrap(),
                serde_json::to_value(after).unwrap()
            );
        }
    }
}
