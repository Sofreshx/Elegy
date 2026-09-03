use elegy_memory_mcp::memory_tools::{MemoryBinding, MemoryRepository, DEFAULT_NAMESPACE};
use tempfile::TempDir;

fn test_binding(agent_id: &str) -> MemoryBinding {
    MemoryBinding::new(DEFAULT_NAMESPACE, agent_id).expect("binding should build")
}

#[test]
fn opening_a_corrupt_database_file_returns_an_error_not_a_panic() {
    let temp_dir = TempDir::new().expect("tempdir should create");
    let db_path = temp_dir.path().join("memory.db");
    std::fs::write(
        &db_path,
        b"this is not a sqlite database, just garbage bytes",
    )
    .expect("corrupt file should write");

    let result = MemoryRepository::new(&db_path, test_binding("corrupt-db-test-agent"));

    let error = match result {
        Ok(_) => panic!("opening a corrupt database should return an error, not Ok"),
        Err(error) => error,
    };

    let display = format!("{error}");
    let debug = format!("{error:?}");
    assert!(
        !display.is_empty(),
        "error Display representation should not be empty"
    );
    assert!(
        !debug.is_empty(),
        "error Debug representation should not be empty"
    );
}

#[cfg(unix)]
#[test]
fn opening_a_permission_denied_database_file_returns_an_error_not_a_panic() {
    use std::os::unix::fs::PermissionsExt;

    let temp_dir = TempDir::new().expect("tempdir should create");
    let db_path = temp_dir.path().join("memory.db");
    std::fs::write(&db_path, b"").expect("empty file should write");
    std::fs::set_permissions(&db_path, std::fs::Permissions::from_mode(0o000))
        .expect("permissions should set");

    let result = MemoryRepository::new(&db_path, test_binding("permission-denied-test-agent"));

    let error = match result {
        Ok(_) => panic!("permission-denied open should return an error, not Ok"),
        Err(error) => error,
    };

    let display = format!("{error}");
    let debug = format!("{error:?}");
    assert!(
        !display.is_empty(),
        "error Display representation should not be empty"
    );
    assert!(
        !debug.is_empty(),
        "error Debug representation should not be empty"
    );

    // Restore permissions so tempdir cleanup can remove the file.
    std::fs::set_permissions(&db_path, std::fs::Permissions::from_mode(0o644))
        .expect("permission restore should succeed");
}
