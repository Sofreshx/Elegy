//! Manual, synthetic library-path measurement; this is not a default latency gate.
//! Run: cargo test --release -p elegy-memory --test recall_performance -- --ignored --nocapture

use std::{
    collections::HashMap,
    fs,
    path::Path,
    time::{Duration, Instant},
};

use chrono::Utc;
use elegy_memory::{
    recall::{contextual_recall, RecallConfig, RecallMode, RecallRequest},
    Memory, MemoryScope, MemoryState, MemoryStore, MemoryType, ProvenanceLevel, SensitivityLevel,
    SqliteMemoryStore,
};
use rusqlite::{Connection, OpenFlags};
use serde_json::json;
use sha2::{Digest, Sha256};
use uuid::Uuid;

const DISTRACTOR_ROWS: usize = 1_000;
const WARMUP_CALLS: usize = 5;
const MEASURED_CALLS: usize = 100;
const P95_TARGET_MS: f64 = 500.0;
const EXPECTED_CONTENT: &str = "Asterlane deployment uses SQLite.";

fn memory(content: String) -> Memory {
    let now = Utc::now();
    Memory {
        id: Uuid::new_v4(),
        content,
        summary: None,
        scope: MemoryScope::Workspace,
        memory_type: MemoryType::Decision,
        provenance: ProvenanceLevel::UserStated,
        importance_score: 0.8,
        reliability_score: 1.0,
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

/// Include WAL contents so an unchanged main file cannot conceal source writes.
/// Missing and zero-byte WALs both represent no uncheckpointed source data.
fn source_bytes(path: &Path) -> (Vec<u8>, Vec<u8>) {
    let wal = path.with_file_name("source.db-wal");
    let wal_bytes = if wal.exists() {
        fs::read(wal).expect("read source WAL")
    } else {
        Vec::new()
    };
    (fs::read(path).expect("read source database"), wal_bytes)
}

fn source_counters(path: &Path) -> (usize, u64, usize) {
    Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .expect("open source read-only for verification")
        .query_row(
            "SELECT COUNT(*), SUM(access_count), COUNT(last_accessed_at) FROM memories",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("source counters")
}

fn milliseconds(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1_000.0
}

/// The target is reported as a measurement, never asserted on a shared machine.
#[test]
#[ignore = "manual release library benchmark with synthetic local data"]
fn contextual_recall_library_latency_on_synthetic_corpus() {
    // An ignored test can be invoked manually in either build profile. Reject
    // debug measurements explicitly rather than report them as release data.
    #[allow(clippy::assertions_on_constants)]
    {
        assert!(
            !cfg!(debug_assertions),
            "run this manual benchmark with cargo test --release"
        );
    }
    let scratch = tempfile::tempdir().expect("synthetic scratch directory");
    let source_path = scratch.path().join("source.db");
    let state_path = scratch.path().join("recall.db");
    let store = SqliteMemoryStore::new(&source_path, MemoryScope::Workspace).expect("source store");
    let expected = memory(EXPECTED_CONTENT.to_owned());
    let expected_id = expected.id.to_string();
    let mut blocked_ids = Vec::new();

    // Seeding and provider-free public store calls are outside all samples.
    elegy_memory::runtime::block_on(async {
        store.store(expected).await.expect("store expected memory");
        for index in 0..DISTRACTOR_ROWS {
            store
                .store(memory(format!(
                    "Asterlane catalog item {index:04} has synthetic archival notes."
                )))
                .await
                .expect("store lexical distractor");
        }
        for kind in ["scope", "sensitivity", "agent", "tenant"] {
            let mut forbidden = memory(format!(
                "Asterlane deployment forbidden synthetic {kind} record."
            ));
            match kind {
                "sensitivity" => forbidden.sensitivity = SensitivityLevel::Critical,
                "agent" => forbidden.agent_id = Some("synthetic-other-agent".to_owned()),
                "tenant" => forbidden.tenant_id = Some("synthetic-other-tenant".to_owned()),
                _ => {}
            }
            let id = store
                .store(forbidden)
                .await
                .expect("store forbidden memory");
            blocked_ids.push((kind, id.to_string()));
        }
    })
    .expect("seeding runtime");
    drop(store);

    // The store validates its scope; move this one fixture row to the denied
    // scope after seeding without changing its FTS entry or matching content.
    let source = Connection::open(&source_path).expect("seed scope exclusion");
    assert_eq!(
        source
            .execute(
                "UPDATE memories SET scope='user' WHERE id=?1",
                [&blocked_ids[0].1]
            )
            .expect("set denied scope"),
        1
    );
    source
        .execute_batch("PRAGMA wal_checkpoint(TRUNCATE)")
        .expect("checkpoint seed writes");
    drop(source);

    let config = RecallConfig {
        version: 1,
        project_root: scratch.path().to_owned(),
        db_path: source_path.clone(),
        state_path: state_path.clone(),
        domain: "synthetic-performance".to_owned(),
        scopes: vec![MemoryScope::Workspace],
        mode: RecallMode::Inject,
        agent_id: None,
        max_sensitivity: SensitivityLevel::Low,
        max_context_tokens: 600,
        min_similarity: 0.8,
        learning_enabled: false,
        include_recent_context: false,
        transcript_root: None,
    };
    let counters_before = source_counters(&source_path);
    assert_eq!(counters_before, (DISTRACTOR_ROWS + 5, 0, 0));
    let bytes_before = source_bytes(&source_path);
    let mut samples = Vec::with_capacity(MEASURED_CALLS);

    for index in 0..WARMUP_CALLS + MEASURED_CALLS {
        // A new session makes suppression from any earlier sample inapplicable.
        let request = RecallRequest {
            cwd: scratch.path().to_owned(),
            session_id: format!("synthetic-session-{index:03}"),
            turn_id: "first-turn".to_owned(),
            prompt: "Asterlane deployment".to_owned(),
            recent_context: Vec::new(),
            embedding: None,
        };
        let started = Instant::now();
        let result = contextual_recall(&config, &request);
        let elapsed = started.elapsed();
        let response = result.expect("library recall");

        assert_eq!(response.mode, RecallMode::Inject);
        assert_eq!(response.status, "selected");
        assert_eq!(response.recalls.len(), 1, "sample {index}");
        assert_eq!(response.recalls[0].id, expected_id, "sample {index}");
        assert_eq!(response.recalls[0].content, EXPECTED_CONTENT);
        assert_eq!(response.recalls[0].scope, MemoryScope::Workspace);
        assert!(response.additional_context.contains(EXPECTED_CONTENT));
        assert!(response.additional_context.len() <= config.max_context_tokens);
        for (_, id) in &blocked_ids {
            assert!(!response.additional_context.contains(id));
        }
        if index >= WARMUP_CALLS {
            samples.push(elapsed);
        }
    }

    assert_eq!(source_counters(&source_path), counters_before);
    assert_eq!(
        source_bytes(&source_path),
        bytes_before,
        "source data changed"
    );
    let journal = Connection::open_with_flags(&state_path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .expect("journal verification");
    let journal_events: usize = journal
        .query_row("SELECT COUNT(*) FROM events", [], |row| row.get(0))
        .expect("event count");
    assert_eq!(journal_events, WARMUP_CALLS + MEASURED_CALLS);
    let journal_sessions: usize = journal
        .query_row("SELECT COUNT(DISTINCT session) FROM events", [], |row| {
            row.get(0)
        })
        .expect("independent sessions");
    assert_eq!(journal_sessions, journal_events);
    let selected_items: usize = journal
        .query_row(
            "SELECT COUNT(*) FROM items WHERE memory_id=?1",
            [&expected_id],
            |row| row.get(0),
        )
        .expect("selected item count");
    assert_eq!(selected_items, journal_events);

    samples.sort_unstable();
    // Nearest-rank percentiles: ranks 50 and 95 of 100 observations.
    let p50_ms = milliseconds(samples[(MEASURED_CALLS * 50).div_ceil(100) - 1]);
    let p95_ms = milliseconds(samples[(MEASURED_CALLS * 95).div_ceil(100) - 1]);
    let max_ms = milliseconds(*samples.last().expect("measured samples"));
    println!(
        "{}",
        json!({
            "benchmark": "contextual-recall-library-synthetic-v1",
            "path": "in-process lexical recall; includes source open, ranking, and journal commit",
            "corpus": {"rows": counters_before.0, "lexicalDistractors": DISTRACTOR_ROWS,
                "authorizedExpected": 1, "forbiddenExactMatches": blocked_ids.len(),
                "embedding": "none", "minSimilarity": config.min_similarity},
            "sampling": {"warmupCalls": WARMUP_CALLS, "measuredCalls": samples.len(),
                "freshSessionPerCall": true, "sharedJournal": true,
                "percentileMethod": "nearest-rank", "setupAndAssertionsTimed": false},
            "latencyMs": {"p50": p50_ms, "p95": p95_ms, "max": max_ms},
            "p95TargetMs": P95_TARGET_MS,
            "p95TargetMet": p95_ms < P95_TARGET_MS,
            "latencyIsMeasurementOnly": true,
            "source": {"bytesUnchanged": true, "walUnchanged": true,
                "countersUnchanged": true, "sha256": format!("{:x}", Sha256::digest(&bytes_before.0)),
                "databaseBytes": bytes_before.0.len(), "walBytes": bytes_before.1.len()},
            "journal": {"events": journal_events, "sessions": journal_sessions,
                "expectedItems": selected_items},
            "environment": {"os": std::env::consts::OS, "arch": std::env::consts::ARCH,
                "availableParallelism": std::thread::available_parallelism().map(usize::from).ok(),
                "debugAssertions": cfg!(debug_assertions), "sqliteVersion": rusqlite::version()},
            "limits": "single local warm lexical workload; no provider, embedding, host-hook, concurrent writer, or production database measurement"
        })
    );
}
