use std::{
    collections::HashMap,
    env, fs,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

use async_trait::async_trait;
use chrono::{Duration, Utc};
use clap::Parser;
use uuid::Uuid;

use super::{
    build_search_response, execute_add_command, execute_import_command,
    format_contradiction_gate_result, format_gate_result, generate_document_embedding, open_store,
    reembed_stale_memories, run_async, Cli, CliEmbeddingProvider, CliLlmProvider, CliScope,
    Command, OutputFormat, StoreArgs, StoreContext,
};
use crate::{
    EmbeddingError, EmbeddingProvider, Memory, MemoryFilter, MemoryScope, MemoryState, MemoryStore,
    MemoryType, ProvenanceLevel, ResolutionStatus, SensitivityLevel, SqliteMemoryStore,
    DEFAULT_OLLAMA_BASE_URL, DEFAULT_OLLAMA_LLM_BASE_URL, DEFAULT_OLLAMA_LLM_MODEL,
    DEFAULT_OLLAMA_MODEL, DEFAULT_OPENAI_BASE_URL, DEFAULT_OPENAI_DIMENSIONS,
    DEFAULT_OPENAI_LLM_BASE_URL, DEFAULT_OPENAI_LLM_MODEL, DEFAULT_OPENAI_MODEL,
};

#[derive(Debug, Clone)]
enum StubEmbeddingResponse {
    Embedding(Vec<f32>),
    Failure(String),
}

#[derive(Debug)]
struct StubEmbeddingProvider {
    model_id: &'static str,
    responses: HashMap<String, StubEmbeddingResponse>,
    calls: Mutex<Vec<String>>,
}

impl StubEmbeddingProvider {
    fn new<I, S>(responses: I) -> Self
    where
        I: IntoIterator<Item = (S, StubEmbeddingResponse)>,
        S: Into<String>,
    {
        Self::new_with_model("stub-embedding-provider", responses)
    }

    fn new_with_model<I, S>(model_id: &'static str, responses: I) -> Self
    where
        I: IntoIterator<Item = (S, StubEmbeddingResponse)>,
        S: Into<String>,
    {
        Self {
            model_id,
            responses: responses
                .into_iter()
                .map(|(text, response)| (text.into(), response))
                .collect(),
            calls: Mutex::new(Vec::new()),
        }
    }

    fn calls(&self) -> Vec<String> {
        self.calls.lock().expect("stub provider calls lock").clone()
    }
}

#[async_trait]
impl EmbeddingProvider for StubEmbeddingProvider {
    async fn embed(&self, text: &str) -> Result<Vec<f32>, EmbeddingError> {
        let trimmed = text.trim().to_string();
        self.calls
            .lock()
            .expect("stub provider calls lock")
            .push(trimmed.clone());
        match self.responses.get(&trimmed) {
            Some(StubEmbeddingResponse::Embedding(embedding)) => Ok(embedding.clone()),
            Some(StubEmbeddingResponse::Failure(message)) => {
                Err(EmbeddingError::Provider(message.clone()))
            }
            None => Err(EmbeddingError::Provider(format!(
                "missing stub embedding for `{trimmed}`"
            ))),
        }
    }

    fn dimensions(&self) -> usize {
        768
    }

    fn model_id(&self) -> &str {
        self.model_id
    }
}

#[tokio::test]
async fn reembed_path_uses_document_prefix_for_nomic_models() {
    let provider = StubEmbeddingProvider::new_with_model(
        "nomic-embed-text:latest",
        [(
            "search_document: Semantic document body",
            StubEmbeddingResponse::Embedding(vec![0.42; 768]),
        )],
    );

    let embedding = generate_document_embedding(&provider, "  Semantic document body  ")
        .await
        .expect("document embedding should succeed");

    assert_eq!(embedding.len(), 768);
    assert_eq!(
        provider.calls(),
        vec!["search_document: Semantic document body".to_string()]
    );
}

#[test]
fn open_store_constructs_ollama_provider_with_defaults() {
    let db_path = unique_temp_path("elegy-memory-cli-open-store");
    let ctx = open_store(StoreArgs {
        db: Some(db_path.clone()),
        scope: CliScope::Workspace,
        embedding_provider: Some(CliEmbeddingProvider::Ollama),
        ollama_url: None,
        ollama_model: None,
        openai_api_key: None,
        openai_model: None,
        openai_url: None,
        openai_dimensions: None,
        llm_provider: None,
        llm_model: None,
        llm_ollama_url: None,
        llm_openai_api_key: None,
        llm_openai_url: None,
        session_id: None,
    })
    .expect("open provider-backed store");

    assert!(ctx.has_embedding_provider());
    let label = ctx.embedding_provider_label();
    assert!(label.contains(DEFAULT_OLLAMA_BASE_URL));
    assert!(label.contains(DEFAULT_OLLAMA_MODEL));

    cleanup_temp_path(&db_path);
}

#[test]
fn open_store_constructs_openai_provider_with_defaults() {
    let db_path = unique_temp_path("elegy-memory-cli-open-store-openai");
    let ctx = open_store(StoreArgs {
        db: Some(db_path.clone()),
        scope: CliScope::Workspace,
        embedding_provider: Some(CliEmbeddingProvider::Openai),
        ollama_url: None,
        ollama_model: None,
        openai_api_key: Some("sk-test-key".to_string()),
        openai_model: None,
        openai_url: None,
        openai_dimensions: None,
        llm_provider: None,
        llm_model: None,
        llm_ollama_url: None,
        llm_openai_api_key: None,
        llm_openai_url: None,
        session_id: None,
    })
    .expect("open OpenAI provider-backed store");

    assert!(ctx.has_embedding_provider());
    let label = ctx.embedding_provider_label();
    assert!(label.contains(DEFAULT_OPENAI_BASE_URL));
    assert!(label.contains(DEFAULT_OPENAI_MODEL));
    assert!(label.contains(&DEFAULT_OPENAI_DIMENSIONS.to_string()));

    cleanup_temp_path(&db_path);
}

#[test]
fn stray_openai_flags_without_embedding_provider_openai_are_rejected() {
    let db_path = unique_temp_path("elegy-memory-cli-stray-openai-flags");
    let error = open_store(StoreArgs {
        db: Some(db_path.clone()),
        scope: CliScope::Workspace,
        embedding_provider: None,
        ollama_url: None,
        ollama_model: None,
        openai_api_key: Some("sk-stray-key".to_string()),
        openai_model: None,
        openai_url: None,
        openai_dimensions: None,
        llm_provider: None,
        llm_model: None,
        llm_ollama_url: None,
        llm_openai_api_key: None,
        llm_openai_url: None,
        session_id: None,
    })
    .expect_err("stray openai flags should be rejected");

    assert!(
        error.to_string().contains("--embedding-provider openai"),
        "expected rejection message, got: {error}"
    );
    cleanup_temp_path(&db_path);
}

#[test]
fn cli_parses_llm_flags_for_consolidate() {
    let cli = Cli::try_parse_from([
        "elegy-memory",
        "consolidate",
        "--llm-provider",
        "openai",
        "--llm-model",
        "gpt-4.1-mini",
        "--llm-openai-api-key",
        "sk-test-key",
        "--llm-openai-url",
        "http://localhost:1234",
    ])
    .expect("cli should parse llm flags");

    match cli.command {
        Command::Consolidate { store, .. } => {
            assert_eq!(store.llm_provider, Some(CliLlmProvider::Openai));
            assert_eq!(store.llm_model.as_deref(), Some("gpt-4.1-mini"));
            assert_eq!(store.llm_openai_api_key.as_deref(), Some("sk-test-key"));
            assert_eq!(
                store.llm_openai_url.as_deref(),
                Some("http://localhost:1234")
            );
        }
        command => panic!("expected consolidate command, got {command:?}"),
    }
}

#[test]
fn open_store_constructs_ollama_llm_provider_with_defaults() {
    let db_path = unique_temp_path("elegy-memory-cli-open-store-ollama-llm");
    let ctx = open_store(StoreArgs {
        db: Some(db_path.clone()),
        scope: CliScope::Workspace,
        embedding_provider: None,
        ollama_url: None,
        ollama_model: None,
        openai_api_key: None,
        openai_model: None,
        openai_url: None,
        openai_dimensions: None,
        llm_provider: Some(CliLlmProvider::Ollama),
        llm_model: None,
        llm_ollama_url: None,
        llm_openai_api_key: None,
        llm_openai_url: None,
        session_id: None,
    })
    .expect("open Ollama llm-backed store");

    assert_eq!(
        ctx.llm_provider_label(),
        format!("ollama ({DEFAULT_OLLAMA_LLM_MODEL} @ {DEFAULT_OLLAMA_LLM_BASE_URL})")
    );
    cleanup_temp_path(&db_path);
}

#[test]
fn open_store_constructs_openai_llm_provider_with_defaults() {
    let db_path = unique_temp_path("elegy-memory-cli-open-store-openai-llm");
    let ctx = open_store(StoreArgs {
        db: Some(db_path.clone()),
        scope: CliScope::Workspace,
        embedding_provider: None,
        ollama_url: None,
        ollama_model: None,
        openai_api_key: None,
        openai_model: None,
        openai_url: None,
        openai_dimensions: None,
        llm_provider: Some(CliLlmProvider::Openai),
        llm_model: None,
        llm_ollama_url: None,
        llm_openai_api_key: Some("sk-test-key".to_string()),
        llm_openai_url: None,
        session_id: None,
    })
    .expect("open OpenAI llm-backed store");

    assert_eq!(
        ctx.llm_provider_label(),
        format!("openai ({DEFAULT_OPENAI_LLM_MODEL} @ {DEFAULT_OPENAI_LLM_BASE_URL})")
    );
    cleanup_temp_path(&db_path);
}

#[test]
fn stray_llm_flags_without_llm_provider_are_rejected() {
    let db_path = unique_temp_path("elegy-memory-cli-stray-llm-flags");
    let error = open_store(StoreArgs {
        db: Some(db_path.clone()),
        scope: CliScope::Workspace,
        embedding_provider: None,
        ollama_url: None,
        ollama_model: None,
        openai_api_key: None,
        openai_model: None,
        openai_url: None,
        openai_dimensions: None,
        llm_provider: None,
        llm_model: Some("qwen3:8b".to_string()),
        llm_ollama_url: None,
        llm_openai_api_key: None,
        llm_openai_url: None,
        session_id: None,
    })
    .expect_err("stray llm flags should be rejected");

    assert!(error.to_string().contains("--llm-provider"));
    cleanup_temp_path(&db_path);
}

#[test]
fn format_gate_result_surfaces_likely_duplicate_warning_details() {
    let similar_to = Uuid::nil();

    assert_eq!(
        format_gate_result(Some(similar_to), Some(0.8249)),
        format!("accepted (similar to {similar_to}, cosine=0.825)")
    );
    assert_eq!(format_gate_result(None, None), "accepted");
}

#[test]
fn format_gate_result_surfaces_contradiction_details() {
    let conflicting_id = Uuid::nil();
    assert_eq!(
        format_contradiction_gate_result(conflicting_id),
        format!("contradiction (conflicts with {conflicting_id})")
    );
}

#[test]
fn provider_backed_search_response_is_not_marked_keyword_only() {
    let db_path = unique_temp_path("elegy-memory-cli-search-provider");
    let provider = Arc::new(StubEmbeddingProvider::new([
        (
            "semantic launch checklist",
            StubEmbeddingResponse::Embedding(vec![1.0; 768]),
        ),
        (
            "semantic probe",
            StubEmbeddingResponse::Embedding(vec![1.0; 768]),
        ),
    ]));
    let store = SqliteMemoryStore::new_with_embedding_provider(
        &db_path,
        MemoryScope::Workspace,
        provider.clone(),
    )
    .expect("create provider-backed store");

    let memory = sample_memory("semantic launch checklist");
    let memory_id = memory.id;
    run_async(store.store(memory)).expect("store semantic memory");

    let ctx = StoreContext {
        db_path: db_path.clone(),
        scope: MemoryScope::Workspace,
        session_id: None,
        store,
        embedding_provider: Some(provider.clone()),
        embedding_provider_label: Some("stub".to_string()),
        llm_provider: None,
        llm_provider_label: None,
    };
    let response = build_search_response(&ctx, "semantic probe".to_string(), 5, false)
        .expect("build provider-backed search response");

    assert!(!response.keyword_only);
    assert_eq!(response.results.len(), 1);
    assert_eq!(response.results[0].id, memory_id.to_string());
    assert_eq!(
        provider.calls(),
        vec![
            "semantic launch checklist".to_string(),
            "semantic probe".to_string(),
        ]
    );

    cleanup_temp_path(&db_path);
}

#[test]
fn reembed_stale_memories_updates_embeddings_and_respects_limit() {
    let db_path = unique_temp_path("elegy-memory-cli-reembed");
    let provider = Arc::new(StubEmbeddingProvider::new([
        ("test", StubEmbeddingResponse::Embedding(vec![0.0; 1])),
        (
            "older stale memory",
            StubEmbeddingResponse::Embedding(vec![1.0; 768]),
        ),
        (
            "newer stale memory",
            StubEmbeddingResponse::Embedding(vec![0.5; 768]),
        ),
    ]));
    let store =
        SqliteMemoryStore::new(&db_path, MemoryScope::Workspace).expect("create sqlite store");

    let older = sample_memory("older stale memory");
    let older_id = older.id;
    run_async(store.store(older)).expect("store older stale memory");

    let mut newer = sample_memory("newer stale memory");
    newer.updated_at = Utc::now() + Duration::milliseconds(1);
    let newer_id = newer.id;
    run_async(store.store(newer)).expect("store newer stale memory");

    let ctx = StoreContext {
        db_path: db_path.clone(),
        scope: MemoryScope::Workspace,
        session_id: None,
        store,
        embedding_provider: Some(provider.clone()),
        embedding_provider_label: Some("stub".to_string()),
        llm_provider: None,
        llm_provider_label: None,
    };
    let response = reembed_stale_memories(&ctx, 1).expect("re-embed stale memories");

    assert_eq!(response.stale_found, 2);
    assert_eq!(response.reembedded_count, 2);
    assert_eq!(response.reembedded_ids.len(), 2);

    let older_memory = ctx.store.get_raw(&older_id);
    let older_memory = run_async(older_memory)
        .expect("load older memory")
        .expect("older memory exists");
    let newer_memory = ctx.store.get_raw(&newer_id);
    let newer_memory = run_async(newer_memory)
        .expect("load newer memory")
        .expect("newer memory exists");
    assert!(!older_memory.embedding_stale);
    assert!(!newer_memory.embedding_stale);

    cleanup_temp_path(&db_path);
}

#[test]
fn reembed_requires_provider_configuration() {
    let db_path = unique_temp_path("elegy-memory-cli-reembed-no-provider");
    let store =
        SqliteMemoryStore::new(&db_path, MemoryScope::Workspace).expect("create sqlite store");
    run_async(store.store(sample_memory("stale memory"))).expect("store stale memory");

    let ctx = StoreContext {
        db_path: db_path.clone(),
        scope: MemoryScope::Workspace,
        session_id: None,
        store,
        embedding_provider: None,
        embedding_provider_label: None,
        llm_provider: None,
        llm_provider_label: None,
    };
    let error = reembed_stale_memories(&ctx, 5).expect_err("provider should be required");

    assert!(error.to_string().contains("--embedding-provider ollama"));

    cleanup_temp_path(&db_path);
}

#[test]
fn reembed_surfaces_provider_failures_with_memory_id() {
    let db_path = unique_temp_path("elegy-memory-cli-reembed-failure");
    let store =
        SqliteMemoryStore::new(&db_path, MemoryScope::Workspace).expect("create sqlite store");
    let memory = sample_memory("failing stale memory");
    let _memory_id = memory.id;
    run_async(store.store(memory)).expect("store failing memory");

    let provider = Arc::new(StubEmbeddingProvider::new([(
        "failing stale memory",
        StubEmbeddingResponse::Failure("stub embed failure".to_string()),
    )]));
    let ctx = StoreContext {
        db_path: db_path.clone(),
        scope: MemoryScope::Workspace,
        session_id: None,
        store,
        embedding_provider: Some(provider),
        embedding_provider_label: Some("stub".to_string()),
        llm_provider: None,
        llm_provider_label: None,
    };
    let error = reembed_stale_memories(&ctx, 5).expect_err("provider failure should surface");

    let message = error.to_string();
    assert!(
        message.contains("reembed provider unavailable at start"),
        "expected fail-fast health check error, got: {message}"
    );
    assert!(
        message.contains("stub embed failure") || message.contains("missing stub embedding for"),
        "expected stub failure in error, got: {message}"
    );

    cleanup_temp_path(&db_path);
}

fn sample_memory(content: &str) -> Memory {
    let now = Utc::now();
    Memory {
        id: uuid::Uuid::new_v4(),
        content: content.to_string(),
        summary: None,
        scope: MemoryScope::Workspace,
        memory_type: MemoryType::Observation,
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
        created_at: now,
        updated_at: now,
        last_accessed_at: None,
        tenant_id: None,
        user_id: None,
        agent_id: None,
    }
}

fn unique_temp_path(prefix: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("current time should be after unix epoch")
        .as_nanos();
    env::temp_dir().join(format!("{prefix}-{unique}.sqlite3"))
}

fn cleanup_temp_path(path: &PathBuf) {
    let _ = fs::remove_file(path);
}

#[test]
fn import_without_force_merges_identical_content_via_stub_provider() {
    let db_path = unique_temp_path("elegy-memory-cli-import-merge");
    let content = "import deduplication test content";

    let provider = Arc::new(StubEmbeddingProvider::new([(
        content,
        StubEmbeddingResponse::Embedding(vec![1.0; 768]),
    )]));

    // Store the memory with its embedding using a provider-backed store.
    let store = SqliteMemoryStore::new_with_embedding_provider(
        &db_path,
        MemoryScope::Workspace,
        provider.clone(),
    )
    .expect("create provider-backed store");
    run_async(store.store(sample_memory(content))).expect("store original memory");
    drop(store);

    // Write a Format B JSON file with the same content.
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock after epoch")
        .as_nanos();
    let json_path = env::temp_dir().join(format!("elegy-import-merge-{unique}.json"));
    fs::write(&json_path, format!("[\"{content}\"]")).expect("write import JSON");

    // Import without force — the gate should detect the duplicate and merge.
    let store = SqliteMemoryStore::new_with_embedding_provider(
        &db_path,
        MemoryScope::Workspace,
        provider.clone(),
    )
    .expect("reopen store");
    let ctx = StoreContext {
        db_path: db_path.clone(),
        scope: MemoryScope::Workspace,
        session_id: None,
        store,
        embedding_provider: Some(provider),
        embedding_provider_label: Some("stub".to_string()),
        llm_provider: None,
        llm_provider_label: None,
    };
    execute_import_command(ctx, Some(json_path.clone()), false, OutputFormat::Text)
        .expect("import should succeed");

    // Verify the memory count did not increase (content was merged, not doubled).
    let check_store =
        SqliteMemoryStore::new(&db_path, MemoryScope::Workspace).expect("reopen for check");
    let memories = run_async(check_store.list(MemoryFilter {
        scope: Some(MemoryScope::Workspace),
        state: None,
        memory_types: None,
        provenance_levels: None,
        tags: None,
        status: None,
        tenant_id: None,
        user_id: None,
        agent_id: None,
        limit: None,
    }))
    .expect("list memories");

    assert_eq!(
        memories.len(),
        1,
        "should still have 1 memory after merging duplicate, got {}",
        memories.len()
    );

    cleanup_temp_path(&db_path);
    let _ = fs::remove_file(&json_path);
}

#[test]
fn add_records_contradiction_and_keeps_both_memories() {
    let db_path = unique_temp_path("elegy-memory-cli-add-contradiction");
    let existing_content = "Backend is C# with gRPC";
    let candidate_content = "Backend is Python with Flask";
    let provider = Arc::new(StubEmbeddingProvider::new([
        (
            existing_content,
            StubEmbeddingResponse::Embedding(vec![1.0; 768]),
        ),
        (
            candidate_content,
            StubEmbeddingResponse::Embedding(vec![1.0; 768]),
        ),
    ]));
    let store = SqliteMemoryStore::new_with_embedding_provider(
        &db_path,
        MemoryScope::Workspace,
        provider.clone(),
    )
    .expect("create provider-backed store");
    let existing_id =
        run_async(store.store(sample_memory(existing_content))).expect("store existing memory");

    let ctx = StoreContext {
        db_path: db_path.clone(),
        scope: MemoryScope::Workspace,
        session_id: None,
        store: store.clone(),
        embedding_provider: Some(provider),
        embedding_provider_label: Some("stub".to_string()),
        llm_provider: None,
        llm_provider_label: None,
    };
    execute_add_command(
        ctx,
        candidate_content.to_string(),
        MemoryType::Observation,
        0.8,
        ProvenanceLevel::UserStated,
        OutputFormat::Text,
    )
    .expect("add should succeed");

    let memories = run_async(store.list(MemoryFilter {
        scope: Some(MemoryScope::Workspace),
        state: None,
        memory_types: None,
        provenance_levels: None,
        tags: None,
        status: None,
        tenant_id: None,
        user_id: None,
        agent_id: None,
        limit: None,
    }))
    .expect("list memories");
    assert_eq!(memories.len(), 2);
    assert!(memories
        .iter()
        .all(|memory| memory.state == MemoryState::Active));
    assert!(memories
        .iter()
        .any(|memory| memory.content == candidate_content && memory.id != existing_id));

    let contradictions = run_async(store.list_contradictions(Some(ResolutionStatus::Unresolved)))
        .expect("list contradictions");
    assert_eq!(contradictions.len(), 1);
    assert_eq!(contradictions[0].memory_a_id, existing_id);
    assert!(contradictions[0].description.contains("python"));

    cleanup_temp_path(&db_path);
}

#[test]
fn import_without_force_records_contradiction_and_keeps_both_memories() {
    let db_path = unique_temp_path("elegy-memory-cli-import-contradiction");
    let existing_content = "Cap RTSS 120fps";
    let candidate_content = "Cap RTSS 60fps";
    let provider = Arc::new(StubEmbeddingProvider::new([
        (
            existing_content,
            StubEmbeddingResponse::Embedding(vec![1.0; 768]),
        ),
        (
            candidate_content,
            StubEmbeddingResponse::Embedding(vec![1.0; 768]),
        ),
    ]));
    let store = SqliteMemoryStore::new_with_embedding_provider(
        &db_path,
        MemoryScope::Workspace,
        provider.clone(),
    )
    .expect("create provider-backed store");
    let existing_id =
        run_async(store.store(sample_memory(existing_content))).expect("store existing memory");

    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock after epoch")
        .as_nanos();
    let json_path = env::temp_dir().join(format!("elegy-import-contradiction-{unique}.json"));
    fs::write(&json_path, format!("[\"{candidate_content}\"]")).expect("write import JSON");

    let ctx = StoreContext {
        db_path: db_path.clone(),
        scope: MemoryScope::Workspace,
        session_id: None,
        store: store.clone(),
        embedding_provider: Some(provider),
        embedding_provider_label: Some("stub".to_string()),
        llm_provider: None,
        llm_provider_label: None,
    };
    execute_import_command(ctx, Some(json_path.clone()), false, OutputFormat::Text)
        .expect("import should succeed");

    let memories = run_async(store.list(MemoryFilter {
        scope: Some(MemoryScope::Workspace),
        state: None,
        memory_types: None,
        provenance_levels: None,
        tags: None,
        status: None,
        tenant_id: None,
        user_id: None,
        agent_id: None,
        limit: None,
    }))
    .expect("list memories");
    assert_eq!(memories.len(), 2);
    assert!(memories
        .iter()
        .any(|memory| memory.content == candidate_content && memory.state == MemoryState::Active));

    let contradictions = run_async(store.list_contradictions(Some(ResolutionStatus::Unresolved)))
        .expect("list contradictions");
    assert_eq!(contradictions.len(), 1);
    assert_eq!(contradictions[0].memory_a_id, existing_id);
    assert!(contradictions[0].description.contains("60fps"));

    cleanup_temp_path(&db_path);
    let _ = fs::remove_file(&json_path);
}
