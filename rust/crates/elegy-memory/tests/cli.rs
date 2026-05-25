use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::process::Command;
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};
use std::{env, fs};

use chrono::{Duration, Utc};
use elegy_memory::{
    Memory, MemoryScope, MemoryState, MemoryStore, MemoryType, ProvenanceLevel, ResolutionStatus,
    SensitivityLevel, SqliteMemoryStore,
};
use rusqlite::Connection;

fn unique_temp_dir(prefix: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("current time should be after unix epoch")
        .as_nanos();
    let dir = env::temp_dir().join(format!("{prefix}-{unique}"));
    fs::create_dir_all(&dir).expect("create temp directory");
    dir
}

fn spawn_fixed_response_server(status_line: &str, body: &str, response_count: usize) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind local test listener");
    let address = listener
        .local_addr()
        .expect("listener should report address");
    let status_line = status_line.to_string();
    let body = body.to_string();

    thread::spawn(move || {
        for _ in 0..response_count {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buffer = [0_u8; 4096];
                let _ = stream.read(&mut buffer);
                let response = format!(
                    "HTTP/1.1 {status_line}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body,
                );
                let _ = stream.write_all(response.as_bytes());
            } else {
                break;
            }
        }
    });

    format!("http://{address}")
}

fn seed_contradiction(
    store: &SqliteMemoryStore,
    runtime: &tokio::runtime::Runtime,
) -> (String, uuid::Uuid, uuid::Uuid) {
    let existing = sample_memory("Backend is C# with gRPC");
    let existing_id = existing.id;
    let candidate = sample_memory("Backend is Python with Flask");
    let candidate_id = candidate.id;

    runtime
        .block_on(store.store(existing))
        .expect("store existing memory");
    runtime
        .block_on(store.store(candidate))
        .expect("store contradictory memory");
    runtime
        .block_on(store.record_contradiction(
            &existing_id,
            &candidate_id,
            "Conflicting technology values detected for backend: c#, grpc vs flask, python",
        ))
        .expect("record contradiction");

    let contradiction_id = runtime
        .block_on(store.list_contradictions(None))
        .expect("list contradictions")
        .into_iter()
        .next()
        .expect("seeded contradiction")
        .id;

    (contradiction_id, existing_id, candidate_id)
}

#[test]
fn add_and_list_memory_via_mvp_cli() {
    let temp_dir = unique_temp_dir("elegy-memory-cli-add-list");
    let db_path = temp_dir.join("memory.sqlite3");

    let add = Command::new(env!("CARGO_BIN_EXE_elegy-memory"))
        .args([
            "--format",
            "json",
            "add",
            "--db",
            db_path.to_str().expect("utf-8 db path"),
            "--type",
            "observation",
            "--importance",
            "0.8",
            "--provenance",
            "user-stated",
            "Remember the launch checklist for Apollo.",
        ])
        .output()
        .expect("run elegy-memory add");

    assert!(
        add.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&add.stderr)
    );

    let add_json: serde_json::Value =
        serde_json::from_slice(&add.stdout).expect("parse add response as json");
    let memory_id = add_json["data"]["memory"]["id"]
        .as_str()
        .expect("memory id in add response")
        .to_string();

    let list = Command::new(env!("CARGO_BIN_EXE_elegy-memory"))
        .args([
            "--format",
            "json",
            "list",
            "--db",
            db_path.to_str().expect("utf-8 db path"),
            "--limit",
            "10",
        ])
        .output()
        .expect("run elegy-memory list");

    assert!(
        list.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&list.stderr)
    );

    let list_json: serde_json::Value =
        serde_json::from_slice(&list.stdout).expect("parse list response as json");
    let memories = list_json["data"]["memories"]
        .as_array()
        .expect("memories array in list response");

    assert!(
        memories
            .iter()
            .any(|memory| memory["id"].as_str() == Some(memory_id.as_str())),
        "expected list output to contain memory id {memory_id}, got {list_json}"
    );
}

#[test]
fn search_returns_keyword_matches_from_cli() {
    let temp_dir = unique_temp_dir("elegy-memory-cli-search");
    let db_path = temp_dir.join("memory.sqlite3");

    let add = Command::new(env!("CARGO_BIN_EXE_elegy-memory"))
        .args([
            "add",
            "--db",
            db_path.to_str().expect("utf-8 db path"),
            "--importance",
            "0.9",
            "Apollo launch checklist with rollback notes",
        ])
        .output()
        .expect("run elegy-memory add");
    assert!(
        add.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&add.stderr)
    );

    let search = Command::new(env!("CARGO_BIN_EXE_elegy-memory"))
        .args([
            "--format",
            "json",
            "search",
            "--db",
            db_path.to_str().expect("utf-8 db path"),
            "Apollo rollback",
            "--limit",
            "5",
        ])
        .output()
        .expect("run elegy-memory search");

    assert!(
        search.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&search.stderr)
    );

    let search_json: serde_json::Value =
        serde_json::from_slice(&search.stdout).expect("parse search response as json");
    let results = search_json["data"]["results"]
        .as_array()
        .expect("results array in search response");

    assert_eq!(search_json["data"]["keywordOnly"].as_bool(), Some(true));
    assert!(
        results.iter().any(|result| result["preview"]
            .as_str()
            .is_some_and(|preview| preview.contains("Apollo"))),
        "expected at least one Apollo keyword match, got {search_json}"
    );
}

#[test]
fn search_with_session_scope_cascades_to_higher_scopes() {
    let temp_dir = unique_temp_dir("elegy-memory-cli-search-scope-cascade");
    let db_path = temp_dir.join("memory.sqlite3");

    for args in [
        vec![
            "add",
            "--db",
            db_path.to_str().expect("utf-8 db path"),
            "--scope",
            "session",
            "Cascade visibility session memory",
        ],
        vec![
            "add",
            "--db",
            db_path.to_str().expect("utf-8 db path"),
            "--scope",
            "workspace",
            "Cascade visibility workspace memory",
        ],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_elegy-memory"))
            .args(args)
            .output()
            .expect("seed scoped memory");
        assert!(
            output.status.success(),
            "stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let search = Command::new(env!("CARGO_BIN_EXE_elegy-memory"))
        .args([
            "--format",
            "json",
            "search",
            "--db",
            db_path.to_str().expect("utf-8 db path"),
            "--scope",
            "session",
            "Cascade visibility",
            "--limit",
            "10",
        ])
        .output()
        .expect("run elegy-memory cascading search");

    assert!(
        search.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&search.stderr)
    );

    let search_json: serde_json::Value =
        serde_json::from_slice(&search.stdout).expect("parse search response");
    assert_eq!(
        search_json["data"]["results"].as_array().map(Vec::len),
        Some(2)
    );
}

#[test]
fn export_writes_utf8_memory_content_to_file() {
    let temp_dir = unique_temp_dir("elegy-memory-cli-export-utf8");
    let db_path = temp_dir.join("memory.sqlite3");
    let export_path = temp_dir.join("export.json");
    let content = "café résumé naïve";

    let add = Command::new(env!("CARGO_BIN_EXE_elegy-memory"))
        .args([
            "add",
            "--db",
            db_path.to_str().expect("utf-8 db path"),
            content,
        ])
        .output()
        .expect("run elegy-memory add");
    assert!(
        add.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&add.stderr)
    );

    let export = Command::new(env!("CARGO_BIN_EXE_elegy-memory"))
        .args([
            "--format",
            "json",
            "export",
            "--db",
            db_path.to_str().expect("utf-8 db path"),
            "--output",
            export_path.to_str().expect("utf-8 export path"),
        ])
        .output()
        .expect("run elegy-memory export");

    assert!(
        export.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&export.stderr)
    );

    let exported = fs::read_to_string(&export_path).expect("read export file as utf-8");
    assert!(
        exported.contains(content),
        "expected exported file to contain literal UTF-8 content, got {exported}"
    );

    let export_json: serde_json::Value =
        serde_json::from_str(&exported).expect("parse export file as json");
    let memories = export_json["memories"]
        .as_array()
        .expect("memories array in export file");
    assert!(
        memories
            .iter()
            .any(|memory| memory["content"].as_str() == Some(content)),
        "expected export file to contain memory content `{content}`, got {export_json}"
    );
}

#[test]
fn export_all_scopes_includes_memories_from_every_scope() {
    let temp_dir = unique_temp_dir("elegy-memory-cli-export-all-scopes");
    let db_path = temp_dir.join("memory.sqlite3");
    let export_path = temp_dir.join("export-all.json");

    for (scope, content) in [
        ("session", "export session memory"),
        ("workspace", "export workspace memory"),
        ("user", "export user memory"),
        ("agent", "export agent memory"),
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_elegy-memory"))
            .args([
                "add",
                "--db",
                db_path.to_str().expect("utf-8 db path"),
                "--scope",
                scope,
                content,
            ])
            .output()
            .expect("seed scoped export memory");
        assert!(
            output.status.success(),
            "stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let export = Command::new(env!("CARGO_BIN_EXE_elegy-memory"))
        .args([
            "export",
            "--db",
            db_path.to_str().expect("utf-8 db path"),
            "--all-scopes",
            "--output",
            export_path.to_str().expect("utf-8 export path"),
        ])
        .output()
        .expect("run all-scope export");
    assert!(
        export.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&export.stderr)
    );

    let export_json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(export_path).expect("read export file"))
            .expect("parse export json");
    assert_eq!(export_json["scope"].as_str(), Some("all"));
    assert_eq!(export_json["memories"].as_array().map(Vec::len), Some(4));
}

#[test]
fn manual_promote_command_moves_memory_to_requested_scope() {
    let temp_dir = unique_temp_dir("elegy-memory-cli-manual-promote");
    let db_path = temp_dir.join("memory.sqlite3");
    let add = Command::new(env!("CARGO_BIN_EXE_elegy-memory"))
        .args([
            "--format",
            "json",
            "add",
            "--db",
            db_path.to_str().expect("utf-8 db path"),
            "--scope",
            "session",
            "manual promote memory",
        ])
        .output()
        .expect("add promotable memory");
    assert!(
        add.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&add.stderr)
    );
    let add_json: serde_json::Value =
        serde_json::from_slice(&add.stdout).expect("parse add response");
    let memory_id = add_json["data"]["memory"]["id"]
        .as_str()
        .expect("memory id")
        .to_string();

    let promote = Command::new(env!("CARGO_BIN_EXE_elegy-memory"))
        .args([
            "--format",
            "json",
            "promote",
            "--db",
            db_path.to_str().expect("utf-8 db path"),
            "--scope",
            "session",
            "--id",
            &memory_id,
            "--to",
            "workspace",
        ])
        .output()
        .expect("run manual promote");
    assert!(
        promote.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&promote.stderr)
    );

    let list = Command::new(env!("CARGO_BIN_EXE_elegy-memory"))
        .args([
            "--format",
            "json",
            "list",
            "--db",
            db_path.to_str().expect("utf-8 db path"),
            "--scope",
            "workspace",
            "--limit",
            "10",
        ])
        .output()
        .expect("list promoted memory");
    assert!(
        list.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&list.stderr)
    );
    let list_json: serde_json::Value =
        serde_json::from_slice(&list.stdout).expect("parse list response");
    assert_eq!(list_json["data"]["count"].as_u64(), Some(1));
}

#[test]
fn reembed_requires_a_configured_provider_from_cli() {
    let temp_dir = unique_temp_dir("elegy-memory-cli-reembed-provider-required");
    let db_path = temp_dir.join("memory.sqlite3");

    let add = Command::new(env!("CARGO_BIN_EXE_elegy-memory"))
        .args([
            "add",
            "--db",
            db_path.to_str().expect("utf-8 db path"),
            "Memory that will remain stale without a provider",
        ])
        .output()
        .expect("run elegy-memory add");
    assert!(
        add.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&add.stderr)
    );

    let reembed = Command::new(env!("CARGO_BIN_EXE_elegy-memory"))
        .args([
            "reembed",
            "--db",
            db_path.to_str().expect("utf-8 db path"),
            "--limit",
            "5",
        ])
        .output()
        .expect("run elegy-memory reembed");

    assert!(
        !reembed.status.success(),
        "stdout: {}",
        String::from_utf8_lossy(&reembed.stdout)
    );
    assert!(
        String::from_utf8_lossy(&reembed.stderr).contains("reembed requires an embedding provider"),
        "expected provider-required error, stderr: {}",
        String::from_utf8_lossy(&reembed.stderr)
    );
}

#[test]
fn contradictions_command_lists_records() {
    let temp_dir = unique_temp_dir("elegy-memory-cli-contradictions");
    let db_path = temp_dir.join("memory.sqlite3");
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("build runtime");
    let store =
        SqliteMemoryStore::new(&db_path, MemoryScope::Workspace).expect("create sqlite store");
    let existing = sample_memory("Backend is C# with gRPC");
    let existing_id = existing.id;
    let candidate = sample_memory("Backend is Python with Flask");
    let candidate_id = candidate.id;

    runtime
        .block_on(store.store(existing))
        .expect("store existing memory");
    runtime
        .block_on(store.store(candidate))
        .expect("store contradictory memory");
    runtime
        .block_on(store.record_contradiction(
            &existing_id,
            &candidate_id,
            "Conflicting technology values detected for backend: c#, grpc vs flask, python",
        ))
        .expect("record contradiction");

    let contradictions = Command::new(env!("CARGO_BIN_EXE_elegy-memory"))
        .args([
            "--format",
            "json",
            "contradictions",
            "--db",
            db_path.to_str().expect("utf-8 db path"),
        ])
        .output()
        .expect("run elegy-memory contradictions");

    assert!(
        contradictions.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&contradictions.stderr)
    );

    let contradictions_json: serde_json::Value =
        serde_json::from_slice(&contradictions.stdout).expect("parse contradictions response");
    let rows = contradictions_json["data"]
        .as_array()
        .expect("contradictions array in response");
    assert_eq!(rows.len(), 1);
    assert_eq!(
        rows[0]["memoryAId"].as_str(),
        Some(existing_id.to_string().as_str())
    );
    assert_eq!(
        rows[0]["memoryBId"].as_str(),
        Some(candidate_id.to_string().as_str())
    );
    assert!(
        rows[0]["description"]
            .as_str()
            .is_some_and(|description| description.contains("Conflicting technology values")),
        "expected contradiction description in {contradictions_json}"
    );
}

#[test]
fn contradictions_resolve_keep_dormants_other_memory_and_marks_resolved() {
    let temp_dir = unique_temp_dir("elegy-memory-cli-contradictions-resolve-keep");
    let db_path = temp_dir.join("memory.sqlite3");
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("build runtime");
    let store =
        SqliteMemoryStore::new(&db_path, MemoryScope::Workspace).expect("create sqlite store");
    let (contradiction_id, keep_id, dormant_id) = seed_contradiction(&store, &runtime);

    let resolve = Command::new(env!("CARGO_BIN_EXE_elegy-memory"))
        .args([
            "--format",
            "json",
            "contradictions",
            "resolve",
            "--db",
            db_path.to_str().expect("utf-8 db path"),
            "--id",
            contradiction_id.as_str(),
            "--keep",
            &keep_id.to_string(),
        ])
        .output()
        .expect("run elegy-memory contradictions resolve");

    assert!(
        resolve.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&resolve.stderr)
    );

    let resolve_json: serde_json::Value =
        serde_json::from_slice(&resolve.stdout).expect("parse resolve response");
    assert_eq!(
        resolve_json["data"]["dormantMemoryId"].as_str(),
        Some(dormant_id.to_string().as_str())
    );

    let kept_memory = runtime
        .block_on(store.get_raw(&keep_id))
        .expect("load kept memory")
        .expect("kept memory exists");
    assert_eq!(kept_memory.state, MemoryState::Active);

    let dormant_memory = runtime
        .block_on(store.get_raw(&dormant_id))
        .expect("load dormant memory")
        .expect("dormant memory exists");
    assert_eq!(dormant_memory.state, MemoryState::Dormant);

    let contradictions = runtime
        .block_on(store.list_contradictions(None))
        .expect("list all contradictions");
    assert_eq!(contradictions.len(), 1);
    assert_eq!(
        contradictions[0].resolution_status,
        ResolutionStatus::ResolvedByUser
    );
    assert!(contradictions[0].resolved_at.is_some());

    let unresolved = runtime
        .block_on(store.list_contradictions(Some(ResolutionStatus::Unresolved)))
        .expect("list unresolved contradictions");
    assert!(unresolved.is_empty());
}

#[test]
fn contradictions_resolve_keep_both_leaves_both_active_and_marks_resolved() {
    let temp_dir = unique_temp_dir("elegy-memory-cli-contradictions-resolve-keep-both");
    let db_path = temp_dir.join("memory.sqlite3");
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("build runtime");
    let store =
        SqliteMemoryStore::new(&db_path, MemoryScope::Workspace).expect("create sqlite store");
    let (contradiction_id, memory_a_id, memory_b_id) = seed_contradiction(&store, &runtime);

    let resolve = Command::new(env!("CARGO_BIN_EXE_elegy-memory"))
        .args([
            "--format",
            "json",
            "contradictions",
            "resolve",
            "--db",
            db_path.to_str().expect("utf-8 db path"),
            "--id",
            contradiction_id.as_str(),
            "--keep-both",
        ])
        .output()
        .expect("run elegy-memory contradictions resolve --keep-both");

    assert!(
        resolve.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&resolve.stderr)
    );

    let resolve_json: serde_json::Value =
        serde_json::from_slice(&resolve.stdout).expect("parse resolve response");
    assert_eq!(resolve_json["data"]["keptBoth"].as_bool(), Some(true));

    let memory_a = runtime
        .block_on(store.get_raw(&memory_a_id))
        .expect("load memory a")
        .expect("memory a exists");
    assert_eq!(memory_a.state, MemoryState::Active);

    let memory_b = runtime
        .block_on(store.get_raw(&memory_b_id))
        .expect("load memory b")
        .expect("memory b exists");
    assert_eq!(memory_b.state, MemoryState::Active);

    let contradictions = runtime
        .block_on(store.list_contradictions(None))
        .expect("list all contradictions");
    assert_eq!(
        contradictions[0].resolution_status,
        ResolutionStatus::ResolvedByUser
    );
    assert!(contradictions[0].resolved_at.is_some());
}

#[test]
fn contradictions_resolve_nonexistent_contradiction_returns_clear_error() {
    let temp_dir = unique_temp_dir("elegy-memory-cli-contradictions-resolve-missing");
    let db_path = temp_dir.join("memory.sqlite3");
    let missing_id = uuid::Uuid::new_v4().to_string();

    let resolve = Command::new(env!("CARGO_BIN_EXE_elegy-memory"))
        .args([
            "contradictions",
            "resolve",
            "--db",
            db_path.to_str().expect("utf-8 db path"),
            "--id",
            missing_id.as_str(),
            "--keep-both",
        ])
        .output()
        .expect("run elegy-memory contradictions resolve missing");

    assert!(
        !resolve.status.success(),
        "stdout: {}",
        String::from_utf8_lossy(&resolve.stdout)
    );
    assert!(
        String::from_utf8_lossy(&resolve.stderr)
            .contains(&format!("contradiction not found: {missing_id}")),
        "expected clear contradiction-not-found error, stderr: {}",
        String::from_utf8_lossy(&resolve.stderr)
    );
}

#[test]
fn health_command_text_includes_enhanced_fields_and_summaries() {
    let temp_dir = unique_temp_dir("elegy-memory-cli-health-text");
    let db_path = temp_dir.join("memory.sqlite3");
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("build runtime");
    let store =
        SqliteMemoryStore::new(&db_path, MemoryScope::Workspace).expect("create sqlite store");
    let now = Utc::now();

    let mut oldest = sample_memory("Architecture notes are still relevant");
    oldest.created_at = now - Duration::days(14);
    oldest.updated_at = oldest.created_at;
    oldest.importance_score = 0.9;
    oldest.access_count = 2;
    oldest.embedding_stale = false;
    oldest.memory_type = MemoryType::Fact;
    let oldest_id = oldest.id;

    let mut stale_fact = sample_memory("API latency budget is tracked weekly");
    stale_fact.created_at = now - Duration::days(10);
    stale_fact.updated_at = stale_fact.created_at;
    stale_fact.importance_score = 0.6;
    stale_fact.access_count = 4;
    stale_fact.memory_type = MemoryType::Fact;

    let mut most_accessed = sample_memory("Preferred editor theme is dark");
    most_accessed.created_at = now - Duration::days(3);
    most_accessed.updated_at = most_accessed.created_at;
    most_accessed.importance_score = 0.3;
    most_accessed.access_count = 7;
    most_accessed.memory_type = MemoryType::Preference;
    let most_accessed_id = most_accessed.id;

    let mut decision = sample_memory("Deploy previews after merge approval");
    decision.created_at = now - Duration::days(1);
    decision.updated_at = decision.created_at;
    decision.importance_score = 0.5;
    decision.access_count = 1;
    decision.memory_type = MemoryType::Decision;

    for memory in [oldest, stale_fact, most_accessed, decision] {
        runtime
            .block_on(store.store(memory))
            .expect("store health fixture memory");
    }

    runtime
        .block_on(store.record_contradiction(
            &oldest_id,
            &most_accessed_id,
            "Conflicting deployment guidance detected for editor setup",
        ))
        .expect("record health contradiction");

    let health = Command::new(env!("CARGO_BIN_EXE_elegy-memory"))
        .args(["health", "--db", db_path.to_str().expect("utf-8 db path")])
        .output()
        .expect("run elegy-memory health");

    assert!(
        health.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&health.stderr)
    );

    let stdout = String::from_utf8_lossy(&health.stdout);
    assert!(
        stdout.contains("stale memory previews:"),
        "stdout: {stdout}"
    );
    assert!(
        stdout.contains("contradiction summaries:"),
        "stdout: {stdout}"
    );
    assert!(stdout.contains("average importance:"), "stdout: {stdout}");
    assert!(
        stdout.contains("oldest memory age (days):"),
        "stdout: {stdout}"
    );
    assert!(stdout.contains("most accessed memory:"), "stdout: {stdout}");
    assert!(stdout.contains("database size:"), "stdout: {stdout}");
    assert!(stdout.contains("- fact: 2"), "stdout: {stdout}");
    assert!(stdout.contains("- preference: 1"), "stdout: {stdout}");
    assert!(stdout.contains("- decision: 1"), "stdout: {stdout}");
    assert!(
        stdout.contains(&most_accessed_id.to_string()),
        "stdout: {stdout}"
    );
    assert!(
        stdout.contains("Conflicting deployment guidance detected for editor setup"),
        "stdout: {stdout}"
    );
}

#[test]
fn health_command_json_exposes_enhanced_fields() {
    let temp_dir = unique_temp_dir("elegy-memory-cli-health-json");
    let db_path = temp_dir.join("memory.sqlite3");
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("build runtime");
    let store =
        SqliteMemoryStore::new(&db_path, MemoryScope::Workspace).expect("create sqlite store");
    let now = Utc::now();
    let mut highest_access_count = 0_u32;
    let mut highest_access_preview = String::new();

    for index in 0..4 {
        let mut memory_a = sample_memory(&format!("Service {index} uses stack alpha"));
        memory_a.created_at = now - Duration::days(20 - i64::from(index));
        memory_a.updated_at = memory_a.created_at;
        memory_a.importance_score = 0.2 + index as f32 * 0.1;
        memory_a.access_count = index as u32;
        memory_a.memory_type = if index % 2 == 0 {
            MemoryType::Fact
        } else {
            MemoryType::Observation
        };
        let memory_a_id = memory_a.id;

        let mut memory_b = sample_memory(&format!("Service {index} uses stack beta"));
        memory_b.created_at = now - Duration::days(16 - i64::from(index));
        memory_b.updated_at = memory_b.created_at;
        memory_b.importance_score = 0.4 + index as f32 * 0.1;
        memory_b.access_count = 6 + index as u32;
        memory_b.memory_type = MemoryType::Procedure;
        if memory_b.access_count > highest_access_count {
            highest_access_count = memory_b.access_count;
            highest_access_preview = memory_b.content.clone();
        }
        let memory_b_id = memory_b.id;

        runtime
            .block_on(store.store(memory_a))
            .expect("store health json memory a");
        runtime
            .block_on(store.store(memory_b))
            .expect("store health json memory b");
        runtime
            .block_on(store.record_contradiction(
                &memory_a_id,
                &memory_b_id,
                &format!("Conflicting stack assignment #{index}"),
            ))
            .expect("record health json contradiction");
    }

    let health = Command::new(env!("CARGO_BIN_EXE_elegy-memory"))
        .args([
            "--format",
            "json",
            "health",
            "--db",
            db_path.to_str().expect("utf-8 db path"),
        ])
        .output()
        .expect("run elegy-memory health json");

    assert!(
        health.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&health.stderr)
    );

    let health_json: serde_json::Value =
        serde_json::from_slice(&health.stdout).expect("parse health response");
    let data = &health_json["data"];
    assert_eq!(health_json["command"].as_str(), Some("health"));
    assert!(
        data["averageImportance"].is_number(),
        "health json: {health_json}"
    );
    assert!(
        data["oldestMemoryAgeDays"]
            .as_i64()
            .is_some_and(|days| days >= 16),
        "health json: {health_json}"
    );
    assert!(
        data["databaseSizeHuman"]
            .as_str()
            .is_some_and(|size| !size.is_empty()),
        "health json: {health_json}"
    );
    assert_eq!(
        data["mostAccessedMemory"]["accessCount"].as_u64(),
        Some(u64::from(highest_access_count))
    );
    assert!(
        data["mostAccessedMemory"]["preview"]
            .as_str()
            .is_some_and(|preview| preview.contains(&highest_access_preview)),
        "health json: {health_json}"
    );
    assert_eq!(
        data["staleMemories"].as_array().map(Vec::len),
        Some(3),
        "health json: {health_json}"
    );
    assert_eq!(
        data["contradictionSummaries"].as_array().map(Vec::len),
        Some(3),
        "health json: {health_json}"
    );
    assert_eq!(
        data["report"]["unresolvedContradictions"].as_u64(),
        Some(4),
        "health json: {health_json}"
    );
    let per_scope_reports = data["perScopeReports"]
        .as_array()
        .expect("per-scope reports array");
    assert_eq!(per_scope_reports.len(), 4, "health json: {health_json}");
    assert_eq!(per_scope_reports[0]["scope"].as_str(), Some("Session"));
    assert_eq!(per_scope_reports[1]["scope"].as_str(), Some("Workspace"));
    assert_eq!(per_scope_reports[1]["activeCount"].as_u64(), Some(8));
    assert_eq!(per_scope_reports[2]["scope"].as_str(), Some("User"));
    assert_eq!(per_scope_reports[2]["activeCount"].as_u64(), Some(0));
    assert_eq!(per_scope_reports[3]["scope"].as_str(), Some("Agent"));
    assert_eq!(per_scope_reports[3]["activeCount"].as_u64(), Some(0));
}

#[test]
fn search_with_session_id_auto_promotes_session_memory_after_three_distinct_sessions() {
    let temp_dir = unique_temp_dir("elegy-memory-cli-session-id-promotion");
    let db_path = temp_dir.join("memory.sqlite3");

    let add = Command::new(env!("CARGO_BIN_EXE_elegy-memory"))
        .args([
            "--format",
            "json",
            "add",
            "--db",
            db_path.to_str().expect("utf-8 db path"),
            "--scope",
            "session",
            "session promotion candidate",
        ])
        .output()
        .expect("add session-scoped memory");
    assert!(
        add.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&add.stderr)
    );
    let add_json: serde_json::Value =
        serde_json::from_slice(&add.stdout).expect("parse add response");
    let memory_id = add_json["data"]["memory"]["id"]
        .as_str()
        .expect("memory id")
        .to_string();

    for session_id in [
        uuid::Uuid::new_v4().to_string(),
        uuid::Uuid::new_v4().to_string(),
        uuid::Uuid::new_v4().to_string(),
    ] {
        let search = Command::new(env!("CARGO_BIN_EXE_elegy-memory"))
            .args([
                "--format",
                "json",
                "search",
                "--db",
                db_path.to_str().expect("utf-8 db path"),
                "--scope",
                "session",
                "--session-id",
                &session_id,
                "session promotion",
                "--limit",
                "5",
            ])
            .output()
            .expect("search session-scoped memory");
        assert!(
            search.status.success(),
            "stderr: {}",
            String::from_utf8_lossy(&search.stderr)
        );
        let search_json: serde_json::Value =
            serde_json::from_slice(&search.stdout).expect("parse search response");
        assert!(
            search_json["data"]["results"]
                .as_array()
                .is_some_and(|results| {
                    results
                        .iter()
                        .any(|result| result["id"].as_str() == Some(memory_id.as_str()))
                }),
            "expected search to find promoted candidate, got {search_json}"
        );
    }

    let session_list = Command::new(env!("CARGO_BIN_EXE_elegy-memory"))
        .args([
            "--format",
            "json",
            "list",
            "--db",
            db_path.to_str().expect("utf-8 db path"),
            "--scope",
            "session",
            "--limit",
            "10",
        ])
        .output()
        .expect("list session scope after promotion");
    assert!(
        session_list.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&session_list.stderr)
    );
    let session_list_json: serde_json::Value =
        serde_json::from_slice(&session_list.stdout).expect("parse session list response");
    assert_eq!(session_list_json["data"]["count"].as_u64(), Some(0));

    let workspace_list = Command::new(env!("CARGO_BIN_EXE_elegy-memory"))
        .args([
            "--format",
            "json",
            "list",
            "--db",
            db_path.to_str().expect("utf-8 db path"),
            "--scope",
            "workspace",
            "--limit",
            "10",
        ])
        .output()
        .expect("list workspace scope after promotion");
    assert!(
        workspace_list.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&workspace_list.stderr)
    );
    let workspace_list_json: serde_json::Value =
        serde_json::from_slice(&workspace_list.stdout).expect("parse workspace list response");
    assert_eq!(workspace_list_json["data"]["count"].as_u64(), Some(1));
    assert!(
        workspace_list_json["data"]["memories"]
            .as_array()
            .is_some_and(|memories| {
                memories
                    .iter()
                    .any(|memory| memory["id"].as_str() == Some(memory_id.as_str()))
            }),
        "expected promoted memory in workspace scope, got {workspace_list_json}"
    );
}

#[test]
fn consolidate_cross_scope_promotes_survivor_and_deletes_duplicate() {
    let temp_dir = unique_temp_dir("elegy-memory-cli-consolidate-cross-scope");
    let db_path = temp_dir.join("memory.sqlite3");
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("build runtime");
    let workspace_store =
        SqliteMemoryStore::new(&db_path, MemoryScope::Workspace).expect("create workspace store");
    let user_store =
        SqliteMemoryStore::new(&db_path, MemoryScope::User).expect("create user store");

    let mut survivor = sample_memory("cross-scope survivor memory");
    survivor.importance_score = 0.9;
    let survivor_id = survivor.id;
    runtime
        .block_on(workspace_store.store(survivor))
        .expect("store workspace survivor");
    runtime
        .block_on(workspace_store.store_embedding(&survivor_id, &axis_embedding()))
        .expect("store workspace survivor embedding");

    let mut duplicate = sample_memory("cross-scope duplicate memory");
    duplicate.scope = MemoryScope::User;
    duplicate.importance_score = 0.4;
    let duplicate_id = duplicate.id;
    runtime
        .block_on(user_store.store(duplicate))
        .expect("store user duplicate");
    runtime
        .block_on(user_store.store_embedding(&duplicate_id, &cosine_embedding(0.95)))
        .expect("store user duplicate embedding");

    let consolidate = Command::new(env!("CARGO_BIN_EXE_elegy-memory"))
        .args([
            "--format",
            "json",
            "consolidate",
            "--db",
            db_path.to_str().expect("utf-8 db path"),
            "--scope",
            "workspace",
            "--cross-scope",
            "--consolidate-limit",
            "10",
        ])
        .output()
        .expect("run cross-scope consolidate");
    assert!(
        consolidate.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&consolidate.stderr)
    );
    let consolidate_json: serde_json::Value =
        serde_json::from_slice(&consolidate.stdout).expect("parse consolidate response");
    assert_eq!(consolidate_json["command"].as_str(), Some("consolidate"));
    assert_eq!(consolidate_json["data"]["mergedCount"].as_u64(), Some(1));
    assert_eq!(
        consolidate_json["data"]["mergedIds"]
            .as_array()
            .map(Vec::len),
        Some(1)
    );
    assert_eq!(
        consolidate_json["data"]["mergedIds"][0].as_str(),
        Some(survivor_id.to_string().as_str())
    );

    let workspace_list = Command::new(env!("CARGO_BIN_EXE_elegy-memory"))
        .args([
            "--format",
            "json",
            "list",
            "--db",
            db_path.to_str().expect("utf-8 db path"),
            "--scope",
            "workspace",
            "--limit",
            "10",
        ])
        .output()
        .expect("list workspace scope after consolidation");
    let workspace_list_json: serde_json::Value =
        serde_json::from_slice(&workspace_list.stdout).expect("parse workspace list response");
    assert_eq!(workspace_list_json["data"]["count"].as_u64(), Some(0));

    let user_list = Command::new(env!("CARGO_BIN_EXE_elegy-memory"))
        .args([
            "--format",
            "json",
            "list",
            "--db",
            db_path.to_str().expect("utf-8 db path"),
            "--scope",
            "user",
            "--limit",
            "10",
        ])
        .output()
        .expect("list user scope after consolidation");
    assert!(
        user_list.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&user_list.stderr)
    );
    let user_list_json: serde_json::Value =
        serde_json::from_slice(&user_list.stdout).expect("parse user list response");
    assert_eq!(user_list_json["data"]["count"].as_u64(), Some(1));
    assert!(
        user_list_json["data"]["memories"]
            .as_array()
            .is_some_and(|memories| {
                memories
                    .iter()
                    .any(|memory| memory["id"].as_str() == Some(survivor_id.to_string().as_str()))
            }),
        "expected consolidated survivor in user scope, got {user_list_json}"
    );
    assert!(
        runtime
            .block_on(user_store.get_raw(&duplicate_id))
            .expect("load dormant duplicate")
            .is_some_and(|memory| memory.state == MemoryState::Dormant),
        "expected duplicate {duplicate_id} to be dormant after consolidation"
    );
}

#[test]
fn consolidate_with_ollama_llm_merges_and_updates_survivor_content() {
    let temp_dir = unique_temp_dir("elegy-memory-cli-consolidate-llm");
    let db_path = temp_dir.join("memory.sqlite3");
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("build runtime");
    let store =
        SqliteMemoryStore::new(&db_path, MemoryScope::Workspace).expect("create workspace store");

    let mut survivor = sample_memory("Project uses Rust");
    survivor.importance_score = 0.9;
    let survivor_id = survivor.id;
    runtime
        .block_on(store.store(survivor))
        .expect("store survivor");
    runtime
        .block_on(store.store_embedding(&survivor_id, &axis_embedding()))
        .expect("store survivor embedding");

    let mut duplicate = sample_memory("Project uses Rust and Tauri");
    duplicate.importance_score = 0.4;
    let duplicate_id = duplicate.id;
    runtime
        .block_on(store.store(duplicate))
        .expect("store duplicate");
    runtime
        .block_on(store.store_embedding(&duplicate_id, &cosine_embedding(0.95)))
        .expect("store duplicate embedding");

    let llm_url = spawn_fixed_response_server(
        "200 OK",
        "{\"response\":\"Project uses Rust and Tauri for the app.\",\"done\":true}",
        1,
    );

    let consolidate = Command::new(env!("CARGO_BIN_EXE_elegy-memory"))
        .args([
            "--format",
            "json",
            "consolidate",
            "--db",
            db_path.to_str().expect("utf-8 db path"),
            "--scope",
            "workspace",
            "--llm-provider",
            "ollama",
            "--llm-ollama-url",
            llm_url.as_str(),
            "--consolidate-limit",
            "10",
        ])
        .output()
        .expect("run llm consolidate");
    assert!(
        consolidate.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&consolidate.stderr)
    );
    let consolidate_json: serde_json::Value =
        serde_json::from_slice(&consolidate.stdout).expect("parse consolidate response");
    assert!(
        consolidate_json["data"]["strategy"]
            .as_str()
            .is_some_and(|strategy| strategy.starts_with("llm (ollama (qwen3:8b @ ")),
        "expected llm strategy label, got {consolidate_json}"
    );
    assert_eq!(consolidate_json["data"]["mergedCount"].as_u64(), Some(1));
    assert_eq!(
        consolidate_json["data"]["contradictionCount"].as_u64(),
        Some(0)
    );

    let inspect = Command::new(env!("CARGO_BIN_EXE_elegy-memory"))
        .args([
            "--format",
            "json",
            "inspect",
            "--db",
            db_path.to_str().expect("utf-8 db path"),
            survivor_id.to_string().as_str(),
        ])
        .output()
        .expect("inspect consolidated survivor");
    assert!(
        inspect.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&inspect.stderr)
    );
    let inspect_json: serde_json::Value =
        serde_json::from_slice(&inspect.stdout).expect("parse inspect response");
    assert_eq!(
        inspect_json["data"]["memory"]["content"].as_str(),
        Some("Project uses Rust and Tauri for the app.")
    );
    assert!(
        runtime
            .block_on(store.get_raw(&duplicate_id))
            .expect("load dormant duplicate")
            .is_some_and(|memory| memory.state == MemoryState::Dormant),
        "expected duplicate {duplicate_id} to be dormant after consolidation"
    );
}

#[test]
fn consolidate_dry_run_reports_merge_without_modifying_store() {
    let temp_dir = unique_temp_dir("elegy-memory-cli-consolidate-dry-run");
    let db_path = temp_dir.join("memory.sqlite3");
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("build runtime");
    let store =
        SqliteMemoryStore::new(&db_path, MemoryScope::Workspace).expect("create workspace store");

    let survivor = sample_memory("Dry run survivor memory");
    let survivor_id = survivor.id;
    runtime
        .block_on(store.store(survivor))
        .expect("store survivor");
    runtime
        .block_on(store.store_embedding(&survivor_id, &axis_embedding()))
        .expect("store survivor embedding");

    let duplicate = sample_memory("Dry run duplicate memory");
    let duplicate_id = duplicate.id;
    runtime
        .block_on(store.store(duplicate))
        .expect("store duplicate");
    runtime
        .block_on(store.store_embedding(&duplicate_id, &cosine_embedding(0.95)))
        .expect("store duplicate embedding");

    let consolidate = Command::new(env!("CARGO_BIN_EXE_elegy-memory"))
        .args([
            "--format",
            "json",
            "consolidate",
            "--db",
            db_path.to_str().expect("utf-8 db path"),
            "--scope",
            "workspace",
            "--dry-run",
            "--consolidate-limit",
            "1",
        ])
        .output()
        .expect("run dry-run consolidate");
    assert!(
        consolidate.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&consolidate.stderr)
    );
    let consolidate_json: serde_json::Value =
        serde_json::from_slice(&consolidate.stdout).expect("parse dry-run consolidate response");
    assert_eq!(consolidate_json["data"]["dryRun"].as_bool(), Some(true));
    assert_eq!(consolidate_json["data"]["mergedCount"].as_u64(), Some(1));

    let memories = runtime
        .block_on(store.list(elegy_memory::MemoryFilter {
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
        .expect("list memories after dry-run");
    assert_eq!(memories.len(), 2, "dry-run should not delete either memory");
}

#[test]
fn ollama_offline_add_succeeds_and_warns_about_degraded_storage() {
    let temp_dir = unique_temp_dir("elegy-memory-cli-add-offline-ollama");
    let db_path = temp_dir.join("memory.sqlite3");

    let closed_port = std::net::TcpListener::bind("127.0.0.1:0")
        .expect("bind ephemeral port")
        .local_addr()
        .expect("read ephemeral listener address")
        .port();
    let ollama_url = format!("http://127.0.0.1:{closed_port}");

    let add = Command::new(env!("CARGO_BIN_EXE_elegy-memory"))
        .args([
            "--format",
            "json",
            "add",
            "--db",
            db_path.to_str().expect("utf-8 db path"),
            "--embedding-provider",
            "ollama",
            "--ollama-url",
            &ollama_url,
            "Remember the offline Ollama fallback.",
        ])
        .output()
        .expect("run elegy-memory add");

    assert!(
        add.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&add.stderr)
    );

    let add_json: serde_json::Value =
        serde_json::from_slice(&add.stdout).expect("parse add response as json");
    assert_eq!(add_json["command"].as_str(), Some("add"));
    assert!(
        String::from_utf8_lossy(&add.stderr).contains(&format!(
            "Ollama not reachable at {ollama_url}, storing without embeddings. Run reembed later."
        )),
        "expected offline warning, stderr: {}",
        String::from_utf8_lossy(&add.stderr)
    );
}

#[test]
fn list_excludes_dormant_by_default_and_includes_with_flag() {
    let temp_dir = unique_temp_dir("elegy-memory-cli-list-include-dormant");
    let db_path = temp_dir.join("memory.sqlite3");
    let db = db_path.to_str().expect("utf-8 db path");

    // Seed two memories via the store API so we can make one dormant.
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("build runtime");
    let store =
        SqliteMemoryStore::new(&db_path, MemoryScope::Workspace).expect("create sqlite store");

    let active = sample_memory("Active memory for list test");
    let active_id = active.id;
    let dormant = sample_memory("Dormant memory for list test");
    let dormant_id = dormant.id;

    runtime
        .block_on(store.store(active))
        .expect("store active memory");
    runtime
        .block_on(store.store(dormant))
        .expect("store dormant memory");
    runtime
        .block_on(store.make_dormant(&dormant_id))
        .expect("make memory dormant");

    // Default list (no --include-dormant): should only contain the active memory.
    let list_default = Command::new(env!("CARGO_BIN_EXE_elegy-memory"))
        .args(["--format", "json", "list", "--db", db, "--limit", "50"])
        .output()
        .expect("run list without --include-dormant");
    assert!(
        list_default.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&list_default.stderr)
    );
    let default_json: serde_json::Value =
        serde_json::from_slice(&list_default.stdout).expect("parse default list json");
    let default_memories = default_json["data"]["memories"]
        .as_array()
        .expect("memories array");
    assert!(
        default_memories
            .iter()
            .any(|m| m["id"].as_str() == Some(&active_id.to_string())),
        "expected active memory {active_id} in default list, got {default_json}"
    );
    assert!(
        !default_memories
            .iter()
            .any(|m| m["id"].as_str() == Some(&dormant_id.to_string())),
        "dormant memory {dormant_id} should NOT appear in default list, got {default_json}"
    );

    // List with --include-dormant: should contain both memories.
    let list_dormant = Command::new(env!("CARGO_BIN_EXE_elegy-memory"))
        .args([
            "--format",
            "json",
            "list",
            "--db",
            db,
            "--include-dormant",
            "--limit",
            "50",
        ])
        .output()
        .expect("run list with --include-dormant");
    assert!(
        list_dormant.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&list_dormant.stderr)
    );
    let dormant_json: serde_json::Value =
        serde_json::from_slice(&list_dormant.stdout).expect("parse include-dormant list json");
    let dormant_memories = dormant_json["data"]["memories"]
        .as_array()
        .expect("memories array");
    assert!(
        dormant_memories
            .iter()
            .any(|m| m["id"].as_str() == Some(&active_id.to_string())),
        "expected active memory {active_id} in dormant list, got {dormant_json}"
    );
    assert!(
        dormant_memories
            .iter()
            .any(|m| m["id"].as_str() == Some(&dormant_id.to_string())),
        "expected dormant memory {dormant_id} in dormant list, got {dormant_json}"
    );
}

#[test]
fn correct_text_surfaces_archived_outcome_details() {
    let temp_dir = unique_temp_dir("elegy-memory-cli-correct-text-archive");
    let db_path = temp_dir.join("memory.sqlite3");
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("build runtime");
    let store =
        SqliteMemoryStore::new(&db_path, MemoryScope::Workspace).expect("create workspace store");

    let mut memory = sample_memory("Needs correction but low salience");
    memory.importance_score = 0.1;
    let memory_id = memory.id;
    runtime
        .block_on(store.store(memory))
        .expect("store low-salience memory");

    let correct = Command::new(env!("CARGO_BIN_EXE_elegy-memory"))
        .args([
            "correct",
            "--db",
            db_path.to_str().expect("utf-8 db path"),
            "--by",
            "operator",
            "--reason",
            "low-salience correction",
            memory_id.to_string().as_str(),
            "Still low salience after correction",
        ])
        .output()
        .expect("run correct text command");

    assert!(
        correct.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&correct.stderr)
    );
    let stdout = String::from_utf8_lossy(&correct.stdout);
    assert!(stdout.contains("Disposition: archived"), "stdout: {stdout}");
    assert!(
        stdout.contains("Current state: dormant"),
        "stdout: {stdout}"
    );
    assert!(
        stdout.contains("Outcome: applied, then archived by the safety gate"),
        "stdout: {stdout}"
    );
}

#[test]
fn inspect_text_includes_correction_history_details() {
    let temp_dir = unique_temp_dir("elegy-memory-cli-inspect-text-corrections");
    let db_path = temp_dir.join("memory.sqlite3");
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("build runtime");
    let store =
        SqliteMemoryStore::new(&db_path, MemoryScope::Workspace).expect("create workspace store");

    let target = sample_memory("Canonical backend is Rust with Axum");
    let target_id = target.id;
    runtime.block_on(store.store(target)).expect("store target");

    let source = sample_memory("Legacy backend is Ruby on Rails");
    let source_id = source.id;
    runtime.block_on(store.store(source)).expect("store source");

    let correct = Command::new(env!("CARGO_BIN_EXE_elegy-memory"))
        .args([
            "--format",
            "json",
            "correct",
            "--db",
            db_path.to_str().expect("utf-8 db path"),
            "--by",
            "operator",
            "--reason",
            "align with canonical backend memory",
            source_id.to_string().as_str(),
            "Canonical backend is Rust with Axum",
        ])
        .output()
        .expect("run merge correction");
    assert!(
        correct.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&correct.stderr)
    );

    let inspect = Command::new(env!("CARGO_BIN_EXE_elegy-memory"))
        .args([
            "inspect",
            "--db",
            db_path.to_str().expect("utf-8 db path"),
            source_id.to_string().as_str(),
        ])
        .output()
        .expect("inspect corrected source");

    assert!(
        inspect.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&inspect.stderr)
    );
    let stdout = String::from_utf8_lossy(&inspect.stdout);
    assert!(stdout.contains("correction history: 1"), "stdout: {stdout}");
    assert!(stdout.contains("[merged]"), "stdout: {stdout}");
    assert!(
        stdout.contains(&format!("related memory: {target_id} (active)")),
        "stdout: {stdout}"
    );
    assert!(
        stdout.contains(
            "outcome: merged into the related memory; the corrected memory was archived to dormant"
        ),
        "stdout: {stdout}"
    );
}

#[test]
fn inspect_json_includes_correction_history_details() {
    let temp_dir = unique_temp_dir("elegy-memory-cli-inspect-json-corrections");
    let db_path = temp_dir.join("memory.sqlite3");
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("build runtime");
    let store =
        SqliteMemoryStore::new(&db_path, MemoryScope::Workspace).expect("create workspace store");

    let target = sample_memory("Canonical deployment runs on Fly.io");
    let target_id = target.id;
    runtime.block_on(store.store(target)).expect("store target");

    let source = sample_memory("Canonical deployment runs on Render");
    let source_id = source.id;
    runtime.block_on(store.store(source)).expect("store source");

    let correct = Command::new(env!("CARGO_BIN_EXE_elegy-memory"))
        .args([
            "--format",
            "json",
            "correct",
            "--db",
            db_path.to_str().expect("utf-8 db path"),
            "--by",
            "operator",
            "--reason",
            "exact duplicate should merge to canonical row",
            source_id.to_string().as_str(),
            "Canonical deployment runs on Fly.io",
        ])
        .output()
        .expect("run correct json command");

    assert!(
        correct.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&correct.stderr)
    );
    let correct_json: serde_json::Value =
        serde_json::from_slice(&correct.stdout).expect("parse correct response");
    assert_eq!(
        correct_json["data"]["correction"]["disposition"].as_str(),
        Some("merged")
    );
    assert_eq!(
        correct_json["data"]["correction"]["relatedMemoryId"].as_str(),
        Some(target_id.to_string().as_str())
    );
    assert_eq!(
        correct_json["data"]["correctedMemoryState"].as_str(),
        Some("dormant")
    );
    assert_eq!(
        correct_json["data"]["correction"]["relatedMemoryState"].as_str(),
        Some("active")
    );

    let inspect = Command::new(env!("CARGO_BIN_EXE_elegy-memory"))
        .args([
            "--format",
            "json",
            "inspect",
            "--db",
            db_path.to_str().expect("utf-8 db path"),
            source_id.to_string().as_str(),
        ])
        .output()
        .expect("inspect corrected source as json");

    assert!(
        inspect.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&inspect.stderr)
    );
    let inspect_json: serde_json::Value =
        serde_json::from_slice(&inspect.stdout).expect("parse inspect response");
    let corrections = inspect_json["data"]["corrections"]
        .as_array()
        .expect("corrections array");
    assert_eq!(corrections.len(), 1);
    assert_eq!(corrections[0]["disposition"].as_str(), Some("merged"));
    assert_eq!(
        corrections[0]["relatedMemoryId"].as_str(),
        Some(target_id.to_string().as_str())
    );
    assert!(
        corrections[0]["outcome"]
            .as_str()
            .is_some_and(|outcome| outcome.contains("merged into the related memory")),
        "inspect json: {inspect_json}"
    );
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
        custom_metadata: Default::default(),
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

fn axis_embedding() -> Vec<f32> {
    let mut embedding = vec![0.0; 768];
    embedding[0] = 1.0;
    embedding
}

fn cosine_embedding(similarity: f32) -> Vec<f32> {
    let mut embedding = vec![0.0; 768];
    embedding[0] = similarity;
    embedding[1] = (1.0_f32 - similarity.powi(2)).sqrt();
    embedding
}

#[test]
fn openai_offline_add_succeeds_and_warns_about_degraded_storage() {
    let temp_dir = unique_temp_dir("elegy-memory-cli-add-offline-openai");
    let db_path = temp_dir.join("memory.sqlite3");

    let closed_port = std::net::TcpListener::bind("127.0.0.1:0")
        .expect("bind ephemeral port")
        .local_addr()
        .expect("read ephemeral listener address")
        .port();
    let openai_url = format!("http://127.0.0.1:{closed_port}");

    let add = Command::new(env!("CARGO_BIN_EXE_elegy-memory"))
        .args([
            "--format",
            "json",
            "add",
            "--db",
            db_path.to_str().expect("utf-8 db path"),
            "--embedding-provider",
            "openai",
            "--openai-api-key",
            "sk-test-key",
            "--openai-url",
            &openai_url,
            "Remember the offline OpenAI fallback.",
        ])
        .output()
        .expect("run elegy-memory add");

    assert!(
        add.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&add.stderr)
    );

    let add_json: serde_json::Value =
        serde_json::from_slice(&add.stdout).expect("parse add response as json");
    assert_eq!(add_json["command"].as_str(), Some("add"));
    assert!(
        String::from_utf8_lossy(&add.stderr).contains(&format!(
            "OpenAI not reachable at {openai_url}, storing without embeddings. Run reembed later."
        )),
        "expected offline warning, stderr: {}",
        String::from_utf8_lossy(&add.stderr)
    );
}

#[test]
fn openai_invalid_api_key_add_succeeds_and_warns_about_degraded_storage() {
    let temp_dir = unique_temp_dir("elegy-memory-cli-add-openai-invalid-key");
    let db_path = temp_dir.join("memory.sqlite3");
    let openai_url = spawn_fixed_response_server(
        "401 Unauthorized",
        r#"{"error":{"message":"Incorrect API key provided","type":"invalid_request_error","code":"invalid_api_key"}}"#,
        2,
    );

    let add = Command::new(env!("CARGO_BIN_EXE_elegy-memory"))
        .args([
            "--format",
            "json",
            "add",
            "--db",
            db_path.to_str().expect("utf-8 db path"),
            "--embedding-provider",
            "openai",
            "--openai-api-key",
            "fake-key",
            "--openai-url",
            &openai_url,
            "Remember the invalid OpenAI key fallback.",
        ])
        .output()
        .expect("run elegy-memory add");

    assert!(
        add.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&add.stderr)
    );

    let add_json: serde_json::Value =
        serde_json::from_slice(&add.stdout).expect("parse add response as json");
    assert_eq!(add_json["command"].as_str(), Some("add"));
    assert!(
        String::from_utf8_lossy(&add.stderr).contains(
            "OpenAI embeddings unavailable (401 Unauthorized: invalid API key), storing without embeddings. Run reembed later."
        ),
        "expected invalid API key warning, stderr: {}",
        String::from_utf8_lossy(&add.stderr)
    );
}

// ── WU8: import command tests ────────────────────────────────────────────────

#[test]
fn import_from_export_file_restores_memories_after_purge() {
    let temp_dir = unique_temp_dir("elegy-memory-cli-import-roundtrip");
    let db_path = temp_dir.join("memory.sqlite3");
    let export_path = temp_dir.join("export.json");

    // Add two memories.
    for content in &[
        "The Apollo launch checklist step one.",
        "The Apollo launch checklist step two.",
    ] {
        let add = Command::new(env!("CARGO_BIN_EXE_elegy-memory"))
            .args([
                "add",
                "--db",
                db_path.to_str().expect("utf-8 db path"),
                "--importance",
                "0.8",
                content,
            ])
            .output()
            .expect("run add");
        assert!(
            add.status.success(),
            "add failed: {}",
            String::from_utf8_lossy(&add.stderr)
        );
    }

    // Export to a file.
    let export = Command::new(env!("CARGO_BIN_EXE_elegy-memory"))
        .args([
            "export",
            "--db",
            db_path.to_str().expect("utf-8 db path"),
            "--output",
            export_path.to_str().expect("utf-8 export path"),
        ])
        .output()
        .expect("run export");
    assert!(
        export.status.success(),
        "export failed: {}",
        String::from_utf8_lossy(&export.stderr)
    );

    // Purge everything.
    let purge = Command::new(env!("CARGO_BIN_EXE_elegy-memory"))
        .args([
            "purge",
            "--db",
            db_path.to_str().expect("utf-8 db path"),
            "--yes",
        ])
        .output()
        .expect("run purge");
    assert!(
        purge.status.success(),
        "purge failed: {}",
        String::from_utf8_lossy(&purge.stderr)
    );

    // List should be empty now.
    let list_empty = Command::new(env!("CARGO_BIN_EXE_elegy-memory"))
        .args([
            "--format",
            "json",
            "list",
            "--db",
            db_path.to_str().expect("utf-8 db path"),
        ])
        .output()
        .expect("run list after purge");
    let list_json: serde_json::Value =
        serde_json::from_slice(&list_empty.stdout).expect("parse list json");
    assert_eq!(
        list_json["data"]["count"].as_u64(),
        Some(0),
        "expected 0 memories after purge, got {list_json}"
    );

    // Import from the export file.
    let import = Command::new(env!("CARGO_BIN_EXE_elegy-memory"))
        .args([
            "--format",
            "json",
            "import",
            "--db",
            db_path.to_str().expect("utf-8 db path"),
            "--input",
            export_path.to_str().expect("utf-8 export path"),
        ])
        .output()
        .expect("run import");
    assert!(
        import.status.success(),
        "import failed: stderr={}  stdout={}",
        String::from_utf8_lossy(&import.stderr),
        String::from_utf8_lossy(&import.stdout)
    );

    let import_json: serde_json::Value =
        serde_json::from_slice(&import.stdout).expect("parse import json");
    assert_eq!(import_json["command"].as_str(), Some("import"));
    assert_eq!(
        import_json["data"]["total"].as_u64(),
        Some(2),
        "expected total=2, got {import_json}"
    );
    assert_eq!(
        import_json["data"]["imported"].as_u64(),
        Some(2),
        "expected imported=2, got {import_json}"
    );
    assert_eq!(
        import_json["data"]["skipped"].as_u64(),
        Some(0),
        "expected skipped=0, got {import_json}"
    );

    // List should show both memories again.
    let list_restored = Command::new(env!("CARGO_BIN_EXE_elegy-memory"))
        .args([
            "--format",
            "json",
            "list",
            "--db",
            db_path.to_str().expect("utf-8 db path"),
            "--limit",
            "10",
        ])
        .output()
        .expect("run list after import");
    let list_json: serde_json::Value =
        serde_json::from_slice(&list_restored.stdout).expect("parse list json");
    assert_eq!(
        list_json["data"]["count"].as_u64(),
        Some(2),
        "expected 2 memories restored after import, got {list_json}"
    );
    assert!(
        list_json["data"]["memories"]
            .as_array()
            .expect("memories array")
            .iter()
            .any(|m| m["preview"].as_str().is_some_and(|p| p.contains("Apollo"))),
        "expected at least one Apollo memory restored, got {list_json}"
    );
}

#[test]
fn import_from_export_preserves_dormant_state_after_contradiction_resolution() {
    let temp_dir = unique_temp_dir("elegy-memory-cli-import-resolved-contradiction");
    let db_path = temp_dir.join("memory.sqlite3");
    let export_path = temp_dir.join("export.json");
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("build runtime");
    let store =
        SqliteMemoryStore::new(&db_path, MemoryScope::Workspace).expect("create sqlite store");
    let (contradiction_id, keep_id, dormant_id) = seed_contradiction(&store, &runtime);

    let resolve = Command::new(env!("CARGO_BIN_EXE_elegy-memory"))
        .args([
            "--format",
            "json",
            "contradictions",
            "resolve",
            "--db",
            db_path.to_str().expect("utf-8 db path"),
            "--id",
            contradiction_id.as_str(),
            "--keep",
            &keep_id.to_string(),
        ])
        .output()
        .expect("run contradictions resolve");
    assert!(
        resolve.status.success(),
        "resolve failed: stderr={} stdout={}",
        String::from_utf8_lossy(&resolve.stderr),
        String::from_utf8_lossy(&resolve.stdout)
    );

    let export = Command::new(env!("CARGO_BIN_EXE_elegy-memory"))
        .args([
            "export",
            "--db",
            db_path.to_str().expect("utf-8 db path"),
            "--output",
            export_path.to_str().expect("utf-8 export path"),
        ])
        .output()
        .expect("run export");
    assert!(
        export.status.success(),
        "export failed: {}",
        String::from_utf8_lossy(&export.stderr)
    );

    let purge = Command::new(env!("CARGO_BIN_EXE_elegy-memory"))
        .args([
            "purge",
            "--db",
            db_path.to_str().expect("utf-8 db path"),
            "--yes",
        ])
        .output()
        .expect("run purge");
    assert!(
        purge.status.success(),
        "purge failed: {}",
        String::from_utf8_lossy(&purge.stderr)
    );

    let openai_url = spawn_fixed_response_server(
        "200 OK",
        r#"{"object":"list","data":[{"object":"embedding","embedding":[1.0],"index":0}],"model":"text-embedding-3-small","usage":{"prompt_tokens":1,"total_tokens":1}}"#,
        6,
    );

    let import = Command::new(env!("CARGO_BIN_EXE_elegy-memory"))
        .args([
            "--format",
            "json",
            "import",
            "--db",
            db_path.to_str().expect("utf-8 db path"),
            "--embedding-provider",
            "openai",
            "--openai-api-key",
            "sk-test-key",
            "--openai-url",
            &openai_url,
            "--openai-dimensions",
            "1",
            "--input",
            export_path.to_str().expect("utf-8 export path"),
        ])
        .output()
        .expect("run import");
    assert!(
        import.status.success(),
        "import failed: stderr={} stdout={}",
        String::from_utf8_lossy(&import.stderr),
        String::from_utf8_lossy(&import.stdout)
    );

    let import_json: serde_json::Value =
        serde_json::from_slice(&import.stdout).expect("parse import json");
    assert_eq!(import_json["data"]["imported"].as_u64(), Some(2));
    assert_eq!(import_json["data"]["contradictions"].as_u64(), Some(0));
    assert_eq!(import_json["data"]["skipped"].as_u64(), Some(0));

    let list_dormant = Command::new(env!("CARGO_BIN_EXE_elegy-memory"))
        .args([
            "--format",
            "json",
            "list",
            "--db",
            db_path.to_str().expect("utf-8 db path"),
            "--state",
            "dormant",
            "--limit",
            "10",
        ])
        .output()
        .expect("run dormant list after import");
    assert!(
        list_dormant.status.success(),
        "dormant list failed: {}",
        String::from_utf8_lossy(&list_dormant.stderr)
    );
    let list_dormant_json: serde_json::Value =
        serde_json::from_slice(&list_dormant.stdout).expect("parse dormant list json");
    assert_eq!(list_dormant_json["data"]["count"].as_u64(), Some(1));
    assert!(
        list_dormant_json["data"]["memories"]
            .as_array()
            .expect("dormant memories array")
            .iter()
            .any(|memory| memory["id"].as_str() == Some(dormant_id.to_string().as_str())),
        "expected dormant loser {dormant_id} to remain dormant after import, got {list_dormant_json}"
    );

    let contradictions = Command::new(env!("CARGO_BIN_EXE_elegy-memory"))
        .args([
            "--format",
            "json",
            "contradictions",
            "--db",
            db_path.to_str().expect("utf-8 db path"),
        ])
        .output()
        .expect("run contradictions after import");
    assert!(
        contradictions.status.success(),
        "contradictions failed: {}",
        String::from_utf8_lossy(&contradictions.stderr)
    );
    let contradictions_json: serde_json::Value =
        serde_json::from_slice(&contradictions.stdout).expect("parse contradictions json");
    assert_eq!(
        contradictions_json["data"].as_array().map(Vec::len),
        Some(0),
        "expected resolved contradiction to stay resolved after import, got {contradictions_json}"
    );
}

#[test]
fn import_simplified_format_bare_strings_and_objects() {
    let temp_dir = unique_temp_dir("elegy-memory-cli-import-simplified");
    let db_path = temp_dir.join("memory.sqlite3");
    let import_path = temp_dir.join("simple.json");

    let json_payload = r#"[
        "A plain string memory.",
        { "content": "A structured memory.", "type": "fact", "importance": 0.7, "provenance": "user-stated" },
        { "content": "A preference memory.", "type": "preference", "provenance": "AgentObserved" }
    ]"#;
    fs::write(&import_path, json_payload).expect("write import file");

    let import = Command::new(env!("CARGO_BIN_EXE_elegy-memory"))
        .args([
            "--format",
            "json",
            "import",
            "--db",
            db_path.to_str().expect("utf-8 db path"),
            "--input",
            import_path.to_str().expect("utf-8 import path"),
        ])
        .output()
        .expect("run import");

    assert!(
        import.status.success(),
        "import failed: stderr={}  stdout={}",
        String::from_utf8_lossy(&import.stderr),
        String::from_utf8_lossy(&import.stdout)
    );

    let import_json: serde_json::Value =
        serde_json::from_slice(&import.stdout).expect("parse import json");
    assert_eq!(import_json["data"]["total"].as_u64(), Some(3));
    assert_eq!(import_json["data"]["imported"].as_u64(), Some(3));
    assert_eq!(import_json["data"]["skipped"].as_u64(), Some(0));

    // Verify all three memories landed in the store.
    let list = Command::new(env!("CARGO_BIN_EXE_elegy-memory"))
        .args([
            "--format",
            "json",
            "list",
            "--db",
            db_path.to_str().expect("utf-8 db path"),
            "--limit",
            "10",
        ])
        .output()
        .expect("run list");
    let list_json: serde_json::Value =
        serde_json::from_slice(&list.stdout).expect("parse list json");
    assert_eq!(
        list_json["data"]["count"].as_u64(),
        Some(3),
        "expected 3 memories, got {list_json}"
    );
    let memories = list_json["data"]["memories"]
        .as_array()
        .expect("memories array");
    let find_memory = |preview: &str| {
        memories
            .iter()
            .find(|memory| memory["preview"].as_str() == Some(preview))
            .unwrap_or_else(|| panic!("expected memory with preview `{preview}`, got {list_json}"))
    };

    let plain = find_memory("A plain string memory.");
    assert_eq!(plain["memoryType"].as_str(), Some("observation"));
    assert_eq!(plain["provenance"].as_str(), Some("imported"));

    let structured = find_memory("A structured memory.");
    assert_eq!(structured["memoryType"].as_str(), Some("fact"));
    assert_eq!(structured["provenance"].as_str(), Some("user-stated"));

    let preference = find_memory("A preference memory.");
    assert_eq!(preference["memoryType"].as_str(), Some("preference"));
    assert_eq!(preference["provenance"].as_str(), Some("agent-observed"));
}

#[test]
fn import_force_bypasses_gate_stores_low_importance_as_active() {
    let temp_dir = unique_temp_dir("elegy-memory-cli-import-force");
    let db_path = temp_dir.join("memory.sqlite3");
    let import_path = temp_dir.join("low_importance.json");

    // An importance of 0.1 is below DEFAULT_SALIENCE_THRESHOLD (0.2).
    // Without --force the gate would archive it (Dormant); with --force it is stored Active.
    let json_payload = r#"[{ "content": "Low importance force-import test.", "importance": 0.1 }]"#;
    fs::write(&import_path, json_payload).expect("write import file");

    // Import WITH --force.
    let import_force = Command::new(env!("CARGO_BIN_EXE_elegy-memory"))
        .args([
            "--format",
            "json",
            "import",
            "--db",
            db_path.to_str().expect("utf-8 db path"),
            "--input",
            import_path.to_str().expect("utf-8 import path"),
            "--force",
        ])
        .output()
        .expect("run import --force");

    assert!(
        import_force.status.success(),
        "forced import failed: {}",
        String::from_utf8_lossy(&import_force.stderr)
    );

    let force_json: serde_json::Value =
        serde_json::from_slice(&import_force.stdout).expect("parse force import json");
    assert_eq!(force_json["data"]["imported"].as_u64(), Some(1));

    // Default list (Active only) must contain the forced memory.
    let list = Command::new(env!("CARGO_BIN_EXE_elegy-memory"))
        .args([
            "--format",
            "json",
            "list",
            "--db",
            db_path.to_str().expect("utf-8 db path"),
        ])
        .output()
        .expect("run list");
    let list_json: serde_json::Value =
        serde_json::from_slice(&list.stdout).expect("parse list json");
    assert_eq!(
        list_json["data"]["count"].as_u64(),
        Some(1),
        "--force should produce 1 active memory, got {list_json}"
    );
}

#[test]
fn import_without_force_routes_through_gate_archives_low_importance() {
    let temp_dir = unique_temp_dir("elegy-memory-cli-import-gate-archive");
    let db_path = temp_dir.join("memory.sqlite3");
    let import_path = temp_dir.join("low_importance.json");

    // An importance of 0.1 is below DEFAULT_SALIENCE_THRESHOLD (0.2) → gate archives it.
    let json_payload = r#"[{ "content": "Low importance gate-archive test.", "importance": 0.1 }]"#;
    fs::write(&import_path, json_payload).expect("write import file");

    let import = Command::new(env!("CARGO_BIN_EXE_elegy-memory"))
        .args([
            "--format",
            "json",
            "import",
            "--db",
            db_path.to_str().expect("utf-8 db path"),
            "--input",
            import_path.to_str().expect("utf-8 import path"),
        ])
        .output()
        .expect("run import without --force");

    assert!(
        import.status.success(),
        "import failed: {}",
        String::from_utf8_lossy(&import.stderr)
    );

    let import_json: serde_json::Value =
        serde_json::from_slice(&import.stdout).expect("parse import json");
    // Gate archived the item → counted as imported (stored as dormant), not skipped.
    assert_eq!(
        import_json["data"]["imported"].as_u64(),
        Some(1),
        "expected 1 imported (archived), got {import_json}"
    );

    // Default list shows Active memories only — archived item must NOT appear.
    let list_active = Command::new(env!("CARGO_BIN_EXE_elegy-memory"))
        .args([
            "--format",
            "json",
            "list",
            "--db",
            db_path.to_str().expect("utf-8 db path"),
            "--state",
            "active",
        ])
        .output()
        .expect("run list active");
    let active_json: serde_json::Value =
        serde_json::from_slice(&list_active.stdout).expect("parse list json");
    assert_eq!(
        active_json["data"]["count"].as_u64(),
        Some(0),
        "gate-archived memory must not appear in active list, got {active_json}"
    );

    // List dormant — gate-archived item must appear there.
    let list_dormant = Command::new(env!("CARGO_BIN_EXE_elegy-memory"))
        .args([
            "--format",
            "json",
            "list",
            "--db",
            db_path.to_str().expect("utf-8 db path"),
            "--state",
            "dormant",
        ])
        .output()
        .expect("run list dormant");
    let dormant_json: serde_json::Value =
        serde_json::from_slice(&list_dormant.stdout).expect("parse list json");
    assert_eq!(
        dormant_json["data"]["count"].as_u64(),
        Some(1),
        "gate-archived memory must appear in dormant list, got {dormant_json}"
    );
}

#[test]
fn import_malformed_json_returns_clear_error() {
    let temp_dir = unique_temp_dir("elegy-memory-cli-import-malformed");
    let db_path = temp_dir.join("memory.sqlite3");
    let bad_path = temp_dir.join("bad.json");

    fs::write(&bad_path, "this is { not valid JSON }}}").expect("write bad json");

    let import = Command::new(env!("CARGO_BIN_EXE_elegy-memory"))
        .args([
            "import",
            "--db",
            db_path.to_str().expect("utf-8 db path"),
            "--input",
            bad_path.to_str().expect("utf-8 bad path"),
        ])
        .output()
        .expect("run import with malformed JSON");

    assert!(
        !import.status.success(),
        "import of malformed JSON should fail, stdout: {}",
        String::from_utf8_lossy(&import.stdout)
    );
    assert!(
        String::from_utf8_lossy(&import.stderr).contains("malformed JSON")
            || String::from_utf8_lossy(&import.stdout).contains("malformed JSON"),
        "expected 'malformed JSON' in output, stderr={} stdout={}",
        String::from_utf8_lossy(&import.stderr),
        String::from_utf8_lossy(&import.stdout)
    );
}

#[test]
fn list_include_dormant_shows_both_active_and_dormant_memories() {
    let temp_dir = unique_temp_dir("elegy-memory-cli-list-include-dormant");
    let db_path = temp_dir.join("memory.sqlite3");
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("build runtime");
    let store =
        SqliteMemoryStore::new(&db_path, MemoryScope::Workspace).expect("create sqlite store");

    let active_memory = sample_memory("Active memory stays visible");
    let dormant_memory = sample_memory("Dormant memory hidden by default");
    let dormant_id = dormant_memory.id;

    runtime
        .block_on(store.store(active_memory))
        .expect("store active memory");
    runtime
        .block_on(store.store(dormant_memory))
        .expect("store dormant memory");
    runtime
        .block_on(store.make_dormant(&dormant_id))
        .expect("make memory dormant");

    // Default list: only active memories.
    let list_default = Command::new(env!("CARGO_BIN_EXE_elegy-memory"))
        .args([
            "--format",
            "json",
            "list",
            "--db",
            db_path.to_str().expect("utf-8 db path"),
        ])
        .output()
        .expect("run default list");
    assert!(
        list_default.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&list_default.stderr)
    );
    let default_json: serde_json::Value =
        serde_json::from_slice(&list_default.stdout).expect("parse default list json");
    assert_eq!(
        default_json["data"]["count"].as_u64(),
        Some(1),
        "default list should show only active memory, got {default_json}"
    );

    // List with --include-dormant: shows both active and dormant.
    let list_with_dormant = Command::new(env!("CARGO_BIN_EXE_elegy-memory"))
        .args([
            "--format",
            "json",
            "list",
            "--db",
            db_path.to_str().expect("utf-8 db path"),
            "--include-dormant",
        ])
        .output()
        .expect("run list --include-dormant");
    assert!(
        list_with_dormant.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&list_with_dormant.stderr)
    );
    let dormant_json: serde_json::Value =
        serde_json::from_slice(&list_with_dormant.stdout).expect("parse include-dormant list json");
    assert_eq!(
        dormant_json["data"]["count"].as_u64(),
        Some(2),
        "list --include-dormant should show both memories, got {dormant_json}"
    );

    // List with --state dormant: only dormant memory.
    let list_state_dormant = Command::new(env!("CARGO_BIN_EXE_elegy-memory"))
        .args([
            "--format",
            "json",
            "list",
            "--db",
            db_path.to_str().expect("utf-8 db path"),
            "--state",
            "dormant",
        ])
        .output()
        .expect("run list --state dormant");
    assert!(
        list_state_dormant.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&list_state_dormant.stderr)
    );
    let state_dormant_json: serde_json::Value =
        serde_json::from_slice(&list_state_dormant.stdout).expect("parse state dormant list json");
    assert_eq!(
        state_dormant_json["data"]["count"].as_u64(),
        Some(1),
        "list --state dormant should show only dormant memory, got {state_dormant_json}"
    );
}

#[test]
fn detect_poisoning_json_surfaces_memory_ids_and_remediation() {
    let temp_dir = unique_temp_dir("elegy-memory-cli-detect-poisoning-json");
    let db_path = temp_dir.join("memory.sqlite3");
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("build runtime");
    let store =
        SqliteMemoryStore::new(&db_path, MemoryScope::Workspace).expect("create sqlite store");

    let mut suspicious = sample_memory("Suspicious imported memory");
    suspicious.provenance = ProvenanceLevel::Imported;
    suspicious.reliability_score = 0.6;
    suspicious.importance_score = 0.96;
    let suspicious_id = suspicious.id;
    runtime
        .block_on(store.store(suspicious))
        .expect("store suspicious memory");

    let connection = Connection::open(&db_path).expect("open sqlite connection");
    connection
        .execute(
            "UPDATE scope_config SET value = '1' WHERE key = 'poison_trust_mismatch_count_threshold'",
            [],
        )
        .expect("lower trust mismatch count threshold");
    connection
        .execute(
            "UPDATE scope_config SET value = '0.95' WHERE key = 'poison_trust_mismatch_importance_threshold'",
            [],
        )
        .expect("lower trust mismatch importance threshold");

    let output = Command::new(env!("CARGO_BIN_EXE_elegy-memory"))
        .args([
            "--format",
            "json",
            "detect-poisoning",
            "--db",
            db_path.to_str().expect("utf-8 db path"),
            "--quarantine",
        ])
        .output()
        .expect("run detect-poisoning");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("parse detect-poisoning json");
    let alerts = json["data"]["alerts"].as_array().expect("alerts array");
    assert_eq!(alerts[0]["alertType"].as_str(), Some("trust_mismatch"));
    assert!(
        alerts.iter().all(|alert| {
            alert["id"].as_str().is_some_and(|id| !id.is_empty())
                && alert["detectedAt"].as_str().is_some_and(|timestamp| {
                    chrono::DateTime::parse_from_rfc3339(timestamp).is_ok()
                })
        }),
        "expected detect-poisoning alerts to expose ids and timestamps, got {json}"
    );
    assert!(
        alerts[0]["severity"]
            .as_f64()
            .is_some_and(|severity| severity > 0.0),
        "expected non-zero severity in {json}"
    );
    assert!(
        alerts.iter().any(|alert| {
            alert["memoryIds"].as_array().is_some_and(|ids| {
                ids.iter()
                    .any(|id| id.as_str() == Some(&suspicious_id.to_string()))
            })
        }),
        "expected detect-poisoning output to surface suspicious id {suspicious_id}, got {json}"
    );
    assert!(
        json["data"]["remediation"]["quarantinedIds"]
            .as_array()
            .is_some_and(|ids| ids
                .iter()
                .any(|id| id.as_str() == Some(&suspicious_id.to_string()))),
        "expected remediation output to include quarantined id {suspicious_id}, got {json}"
    );
    assert!(
        json["data"]["remediation"]["actions"]
            .as_array()
            .is_some_and(|actions| actions.iter().any(|action| {
                action["memoryId"].as_str() == Some(&suspicious_id.to_string())
                    && action["action"].as_str() == Some("quarantined")
                    && action["reason"]
                        .as_str()
                        .is_some_and(|reason| reason.contains("trust_mismatch"))
            })),
        "expected remediation output to explain the quarantine for {suspicious_id}, got {json}"
    );

    let reloaded = runtime
        .block_on(store.get_raw(&suspicious_id))
        .expect("reload suspicious memory")
        .expect("suspicious memory exists");
    assert_eq!(reloaded.state, MemoryState::Dormant);
}

#[test]
fn detect_poisoning_text_surfaces_memory_ids() {
    let temp_dir = unique_temp_dir("elegy-memory-cli-detect-poisoning-text");
    let db_path = temp_dir.join("memory.sqlite3");
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("build runtime");
    let store =
        SqliteMemoryStore::new(&db_path, MemoryScope::Workspace).expect("create sqlite store");

    let mut suspicious = sample_memory("Suspicious text output memory");
    suspicious.provenance = ProvenanceLevel::Imported;
    suspicious.reliability_score = 0.6;
    suspicious.importance_score = 0.96;
    let suspicious_id = suspicious.id;
    runtime
        .block_on(store.store(suspicious))
        .expect("store suspicious memory");

    let connection = Connection::open(&db_path).expect("open sqlite connection");
    connection
        .execute(
            "UPDATE scope_config SET value = '1' WHERE key = 'poison_trust_mismatch_count_threshold'",
            [],
        )
        .expect("lower trust mismatch count threshold");
    connection
        .execute(
            "UPDATE scope_config SET value = '0.95' WHERE key = 'poison_trust_mismatch_importance_threshold'",
            [],
        )
        .expect("lower trust mismatch importance threshold");

    let output = Command::new(env!("CARGO_BIN_EXE_elegy-memory"))
        .args([
            "detect-poisoning",
            "--db",
            db_path.to_str().expect("utf-8 db path"),
        ])
        .output()
        .expect("run detect-poisoning text");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains(&suspicious_id.to_string()),
        "expected text output to contain suspicious id {suspicious_id}, got {stdout}"
    );
    assert!(
        stdout.contains("alert_id:") && stdout.contains("detected_at:"),
        "expected text output to contain alert ids and timestamps, got {stdout}"
    );
    assert!(
        stdout.contains("--quarantine") && stdout.contains("--remediate"),
        "expected text output to include remediation guidance, got {stdout}"
    );
}

#[test]
fn detect_poisoning_help_prefers_quarantine_flag_and_keeps_alias_visible() {
    let output = Command::new(env!("CARGO_BIN_EXE_elegy-memory"))
        .args(["detect-poisoning", "--help"])
        .output()
        .expect("run detect-poisoning --help");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("--quarantine"),
        "expected help output to prefer --quarantine, got {stdout}"
    );
    assert!(
        stdout.contains("remediate"),
        "expected help output to expose the --remediate alias, got {stdout}"
    );
}

#[test]
fn share_import_keeps_existing_active_memory_untouched() {
    let temp_dir = unique_temp_dir("elegy-memory-cli-share-import-safe");
    let db_path = temp_dir.join("memory.sqlite3");
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("build runtime");
    let store =
        SqliteMemoryStore::new(&db_path, MemoryScope::Workspace).expect("create sqlite store");

    let existing = sample_memory("Trusted canonical memory");
    let existing_id = existing.id;
    runtime
        .block_on(store.store(existing))
        .expect("store canonical memory");

    let mut shared = sample_memory("Trusted canonical memory");
    shared.scope = MemoryScope::Agent;
    shared.provenance = ProvenanceLevel::UserStated;
    let import_path = temp_dir.join("shared.json");
    fs::write(
        &import_path,
        serde_json::to_string_pretty(&vec![shared]).expect("serialize shared memories"),
    )
    .expect("write shared import payload");

    let output = Command::new(env!("CARGO_BIN_EXE_elegy-memory"))
        .args([
            "--format",
            "json",
            "share-import",
            "--db",
            db_path.to_str().expect("utf-8 db path"),
            "--input",
            import_path.to_str().expect("utf-8 import path"),
        ])
        .output()
        .expect("run share-import");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("parse share-import json");
    assert_eq!(json["data"]["reviewCount"].as_u64(), Some(0));
    assert_eq!(json["data"]["quarantinedCount"].as_u64(), Some(1));
    assert!(
        json["data"]["outcomes"]
            .as_array()
            .is_some_and(|outcomes| outcomes.iter().any(|outcome| {
                outcome["disposition"].as_str() == Some("quarantine")
                    && outcome["reason"]
                        .as_str()
                        .is_some_and(|reason| reason.contains("review"))
            })),
        "expected share-import output to explain the quarantine disposition, got {json}"
    );
    let imported_id = uuid::Uuid::parse_str(
        json["data"]["newIds"][0]
            .as_str()
            .expect("new shared import id"),
    )
    .expect("parse imported uuid");

    let existing = runtime
        .block_on(store.get_raw(&existing_id))
        .expect("reload canonical memory")
        .expect("canonical memory exists");
    assert_eq!(existing.state, MemoryState::Active);
    assert_eq!(existing.content, "Trusted canonical memory");

    let imported = runtime
        .block_on(store.get_raw(&imported_id))
        .expect("reload imported shared memory")
        .expect("imported shared memory exists");
    assert_eq!(imported.state, MemoryState::Dormant);
    assert_eq!(imported.status.as_deref(), Some("quarantined"));
}

#[test]
fn share_export_json_filters_memories_for_sharing() {
    let temp_dir = unique_temp_dir("elegy-memory-cli-share-export-json");
    let db_path = temp_dir.join("memory.sqlite3");
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("build runtime");
    let store =
        SqliteMemoryStore::new(&db_path, MemoryScope::Workspace).expect("create sqlite store");

    let mut exportable = sample_memory("Shareable workspace memory");
    exportable.reliability_score = 0.8;
    exportable.sensitivity = SensitivityLevel::Low;
    runtime
        .block_on(store.store(exportable))
        .expect("store shareable memory");

    let mut filtered = sample_memory("Secret workspace memory");
    filtered.reliability_score = 0.9;
    filtered.sensitivity = SensitivityLevel::High;
    runtime
        .block_on(store.store(filtered))
        .expect("store filtered memory");

    let output = Command::new(env!("CARGO_BIN_EXE_elegy-memory"))
        .args([
            "--format",
            "json",
            "share-export",
            "--db",
            db_path.to_str().expect("utf-8 db path"),
        ])
        .output()
        .expect("run share-export");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("parse share-export json");
    let memories = json["data"]["memories"]
        .as_array()
        .expect("share-export memories array");
    assert_eq!(
        memories.len(),
        1,
        "expected one exportable memory, got {json}"
    );
    assert_eq!(
        memories[0]["content"].as_str(),
        Some("Shareable workspace memory")
    );
}

#[test]
fn share_import_skips_higher_scope_duplicates_in_json_output() {
    let temp_dir = unique_temp_dir("elegy-memory-cli-share-import-skip");
    let db_path = temp_dir.join("memory.sqlite3");
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("build runtime");
    let workspace_store =
        SqliteMemoryStore::new(&db_path, MemoryScope::Workspace).expect("create workspace store");
    let user_store =
        SqliteMemoryStore::new(&db_path, MemoryScope::User).expect("create user store");

    let mut canonical = sample_memory("Higher scope canonical memory");
    canonical.scope = MemoryScope::User;
    runtime
        .block_on(user_store.store(canonical))
        .expect("store higher-scope canonical memory");

    let mut shared = sample_memory("Higher scope canonical memory");
    shared.scope = MemoryScope::Agent;
    shared.provenance = ProvenanceLevel::UserStated;
    let import_path = temp_dir.join("shared-skip.json");
    fs::write(
        &import_path,
        serde_json::to_string_pretty(&vec![shared]).expect("serialize shared memories"),
    )
    .expect("write shared import payload");

    let output = Command::new(env!("CARGO_BIN_EXE_elegy-memory"))
        .args([
            "--format",
            "json",
            "share-import",
            "--db",
            db_path.to_str().expect("utf-8 db path"),
            "--input",
            import_path.to_str().expect("utf-8 import path"),
        ])
        .output()
        .expect("run share-import");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("parse share-import json");
    assert_eq!(json["data"]["importedCount"].as_u64(), Some(0));
    assert_eq!(json["data"]["skippedCount"].as_u64(), Some(1));
    assert!(
        json["data"]["outcomes"]
            .as_array()
            .is_some_and(|outcomes| outcomes.iter().any(|outcome| {
                outcome["disposition"].as_str() == Some("skip")
                    && outcome["reason"]
                        .as_str()
                        .is_some_and(|reason| reason.contains("higher visible scope"))
            })),
        "expected share-import output to expose the skip reason, got {json}"
    );

    let remaining = runtime
        .block_on(workspace_store.list(elegy_memory::MemoryFilter {
            scope: Some(MemoryScope::Workspace),
            ..Default::default()
        }))
        .expect("list workspace memories after skipped import");
    assert!(
        remaining.is_empty(),
        "skipped shared import must not create workspace rows"
    );
}

#[test]
fn feedback_command_json_reports_live_learning_summary() {
    let temp_dir = unique_temp_dir("elegy-memory-cli-feedback");
    let db_path = temp_dir.join("memory.sqlite3");
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("build runtime");

    let add = Command::new(env!("CARGO_BIN_EXE_elegy-memory"))
        .args([
            "--format",
            "json",
            "add",
            "--db",
            db_path.to_str().expect("utf-8 db path"),
            "Apollo rollout checklist",
        ])
        .output()
        .expect("run add");
    assert!(
        add.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&add.stderr)
    );
    let add_json: serde_json::Value = serde_json::from_slice(&add.stdout).expect("parse add json");
    let memory_id = add_json["data"]["memory"]["id"]
        .as_str()
        .expect("memory id in add response")
        .to_string();

    let feedback = Command::new(env!("CARGO_BIN_EXE_elegy-memory"))
        .args([
            "--format",
            "json",
            "feedback",
            "--db",
            db_path.to_str().expect("utf-8 db path"),
            &memory_id,
            "--query",
            "apollo rollout",
            "--relevant",
        ])
        .output()
        .expect("run feedback");
    assert!(
        feedback.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&feedback.stderr)
    );

    let feedback_json: serde_json::Value =
        serde_json::from_slice(&feedback.stdout).expect("parse feedback json");
    assert_eq!(
        feedback_json["data"]["memoryId"].as_str(),
        Some(memory_id.as_str())
    );
    assert_eq!(feedback_json["data"]["wasRelevant"].as_bool(), Some(true));
    assert_eq!(
        feedback_json["data"]["learning"]["strategy"].as_str(),
        Some("defaults")
    );
    assert_eq!(
        feedback_json["data"]["learning"]["sampleSize"].as_u64(),
        Some(1)
    );
    assert!(
        feedback_json["data"]["learning"]["effectiveWeights"]["similarityWeight"]
            .as_f64()
            .is_some(),
        "expected effective similarity weight in feedback output: {feedback_json}"
    );
    assert!(
        feedback_json["data"]["learning"]["effectiveWeights"]["accessWeight"]
            .as_f64()
            .is_some(),
        "expected live access weight in feedback output: {feedback_json}"
    );

    let store = SqliteMemoryStore::new(&db_path, MemoryScope::Workspace).expect("open store");
    let stored =
        runtime
            .block_on(store.get_raw(
                &uuid::Uuid::parse_str(&memory_id).expect("memory id should be valid uuid"),
            ))
            .expect("load memory")
            .expect("memory should exist");
    assert_eq!(stored.access_count, 1);
}

#[test]
fn weights_command_json_reports_learned_live_config_names() {
    let temp_dir = unique_temp_dir("elegy-memory-cli-weights");
    let db_path = temp_dir.join("memory.sqlite3");
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("build runtime");
    let store = SqliteMemoryStore::new(&db_path, MemoryScope::Workspace).expect("create store");

    for idx in 0..6 {
        let mut relevant = sample_memory(&format!("apollo rollout checklist canonical {idx}"));
        relevant.importance_score = 0.35;
        let relevant_id = relevant.id;
        runtime
            .block_on(store.store(relevant))
            .expect("store relevant memory");

        let mut irrelevant = sample_memory(&format!("apollo archive reference {idx}"));
        irrelevant.importance_score = 0.95;
        let irrelevant_id = irrelevant.id;
        runtime
            .block_on(store.store(irrelevant))
            .expect("store irrelevant memory");

        store
            .record_feedback(&relevant_id, "apollo rollout checklist", true)
            .expect("record relevant feedback");
        store
            .record_feedback(&irrelevant_id, "apollo rollout checklist", false)
            .expect("record irrelevant feedback");
    }

    let output = Command::new(env!("CARGO_BIN_EXE_elegy-memory"))
        .args([
            "--format",
            "json",
            "weights",
            "--db",
            db_path.to_str().expect("utf-8 db path"),
        ])
        .output()
        .expect("run weights");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("parse weights json");
    assert_eq!(
        json["data"]["learning"]["strategy"].as_str(),
        Some("learned")
    );
    assert_eq!(json["data"]["learning"]["sampleSize"].as_u64(), Some(12));
    assert_eq!(
        json["data"]["learning"]["relevantSamples"].as_u64(),
        Some(6)
    );
    assert_eq!(
        json["data"]["learning"]["irrelevantSamples"].as_u64(),
        Some(6)
    );

    let effective = &json["data"]["learning"]["effectiveWeights"];
    assert!(effective["similarityWeight"].as_f64().is_some());
    assert!(effective["recencyWeight"].as_f64().is_some());
    assert!(effective["accessWeight"].as_f64().is_some());
    assert!(effective["priorityWeight"].as_f64().is_some());
    assert!(
        effective["similarityWeight"]
            .as_f64()
            .is_some_and(|value| value > 0.4),
        "expected learned similarity weight above default, got {json}"
    );
}
