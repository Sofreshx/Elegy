use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};
use std::{env, fs};

fn unique_temp_dir(prefix: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("current time should be after unix epoch")
        .as_nanos();
    let dir = env::temp_dir().join(format!("{prefix}-{unique}"));
    fs::create_dir_all(&dir).expect("create temp directory");
    dir
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
        results
            .iter()
            .any(|result| result["preview"]
                .as_str()
                .is_some_and(|preview| preview.contains("Apollo"))),
        "expected at least one Apollo keyword match, got {search_json}"
    );
}
