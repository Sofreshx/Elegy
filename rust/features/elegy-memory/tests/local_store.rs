use elegy_memory::{
    GovernedMemoryRecordImportOptions, LocalMemoryLifecycleState, LocalMemoryQueryOptions,
    LocalMemoryStore, LocalMemoryStoreError, SummaryOnlySessionContextEnvelope,
};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

const SUMMARY_ONLY_FIXTURE_JSON: &str = include_str!(
    "../../../../contracts/fixtures/summary-only-session-context-envelope.minimal.json"
);

fn unique_temp_dir(prefix: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("current time should be after unix epoch")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("{prefix}-{unique}"));
    fs::create_dir_all(&dir).expect("create temp directory");
    dir
}

fn summary_only_fixture() -> SummaryOnlySessionContextEnvelope {
    serde_json::from_str(SUMMARY_ONLY_FIXTURE_JSON)
        .expect("summary-only fixture should deserialize")
}

#[test]
fn local_store_initializes_layout_without_durable_catalog() {
    let root = unique_temp_dir("elegy-memory-init").join("local-store");
    let store = LocalMemoryStore::new(&root);

    let initialized = store.init().expect("local store init should succeed");

    assert!(initialized.paths.artifacts_dir.is_dir());
    assert!(initialized.paths.state_dir.is_dir());
    assert!(initialized.paths.exports_dir.is_dir());
    assert!(!initialized.paths.write_lock_path.exists());
    assert!(!initialized.paths.state_dir.join("catalog.json").exists());

    let records = store
        .list_records(&LocalMemoryQueryOptions::default())
        .expect("store should derive an empty record set from artifacts");
    assert!(records.is_empty());
}

#[test]
fn local_store_is_deterministic_and_active_only_by_default() {
    let root = unique_temp_dir("elegy-memory-deterministic").join("local-store");
    let store = LocalMemoryStore::new(&root);
    store.init().expect("local store init should succeed");

    let envelope = summary_only_fixture();
    let imported_a = store
        .import_summary_only_envelope(
            &envelope,
            GovernedMemoryRecordImportOptions {
                record_id: "record-a".to_string(),
                imported_at_utc: "2026-03-23T00:00:00Z".to_string(),
            },
        )
        .expect("import record-a should succeed");
    let imported_a_repeat = store
        .import_summary_only_envelope(
            &envelope,
            GovernedMemoryRecordImportOptions {
                record_id: "record-a".to_string(),
                imported_at_utc: "2026-03-23T00:00:00Z".to_string(),
            },
        )
        .expect("re-importing same record should be idempotent");
    assert_eq!(imported_a.record, imported_a_repeat.record);

    let mut second_envelope = summary_only_fixture();
    second_envelope.request_id = Some("request-2".to_string());
    second_envelope.run_id = Some("run-2".to_string());
    second_envelope.captured_at_utc = Some("2026-03-22T01:00:00Z".to_string());
    second_envelope.session_context.summary = "Second local summary.".to_string();

    store
        .import_summary_only_envelope(
            &second_envelope,
            GovernedMemoryRecordImportOptions {
                record_id: "record-b".to_string(),
                imported_at_utc: "2026-03-23T01:00:00Z".to_string(),
            },
        )
        .expect("import record-b should succeed");

    let mut third_envelope = summary_only_fixture();
    third_envelope.request_id = Some("request-3".to_string());
    third_envelope.run_id = Some("run-3".to_string());
    third_envelope.captured_at_utc = Some("2026-03-22T02:00:00Z".to_string());
    third_envelope.session_context.summary = "Third local summary.".to_string();

    store
        .import_summary_only_envelope(
            &third_envelope,
            GovernedMemoryRecordImportOptions {
                record_id: "record-c".to_string(),
                imported_at_utc: "2026-03-23T02:00:00Z".to_string(),
            },
        )
        .expect("import record-c should succeed");

    store
        .supersede_record("record-a", "record-b")
        .expect("superseding record-a should succeed");
    store
        .tombstone_record(
            "record-c",
            "2026-03-24T00:00:00Z",
            "Local tombstone for withdrawn artifact.",
        )
        .expect("tombstoning record-c should succeed");

    let default_entries = store
        .list_records(&LocalMemoryQueryOptions::default())
        .expect("default list should succeed");
    assert_eq!(default_entries.len(), 1);
    assert_eq!(default_entries[0].record_id, "record-b");
    assert_eq!(
        default_entries[0].lifecycle_state,
        LocalMemoryLifecycleState::Active
    );

    let all_entries = store
        .list_records(&LocalMemoryQueryOptions {
            include_superseded: true,
            include_tombstoned: true,
        })
        .expect("full list should succeed");
    assert_eq!(
        all_entries
            .iter()
            .map(|entry| entry.record_id.as_str())
            .collect::<Vec<_>>(),
        vec!["record-a", "record-b", "record-c"]
    );
    assert_eq!(
        all_entries
            .iter()
            .map(|entry| entry.lifecycle_state)
            .collect::<Vec<_>>(),
        vec![
            LocalMemoryLifecycleState::Superseded,
            LocalMemoryLifecycleState::Active,
            LocalMemoryLifecycleState::Tombstoned,
        ]
    );

    let shown_default = store.show_record("record-b", &LocalMemoryQueryOptions::default());
    assert!(shown_default.is_ok());
    let hidden_superseded = store.show_record("record-a", &LocalMemoryQueryOptions::default());
    assert!(matches!(
        hidden_superseded,
        Err(LocalMemoryStoreError::RecordExcludedByLifecycle { .. })
    ));

    let exported = store
        .export_summary_only_envelope("record-b", None, &LocalMemoryQueryOptions::default())
        .expect("export should succeed");
    let first_export =
        fs::read_to_string(&exported.output_path).expect("read export after first run");
    let exported_again = store
        .export_summary_only_envelope("record-b", None, &LocalMemoryQueryOptions::default())
        .expect("repeat export should succeed");
    let second_export =
        fs::read_to_string(&exported_again.output_path).expect("read export after second run");
    assert_eq!(exported.output_path, exported_again.output_path);
    assert_eq!(first_export, second_export);
    assert!(first_export.contains("summary-only-session-context-envelope"));
}

#[test]
fn local_store_reimport_is_idempotent_across_equivalent_imported_at_offsets() {
    let root = unique_temp_dir("elegy-memory-offset-idempotent").join("local-store");
    let store = LocalMemoryStore::new(&root);
    store.init().expect("local store init should succeed");

    let envelope = summary_only_fixture();
    let imported_z = store
        .import_summary_only_envelope(
            &envelope,
            GovernedMemoryRecordImportOptions {
                record_id: "offset-record".to_string(),
                imported_at_utc: "2026-03-23T00:00:00Z".to_string(),
            },
        )
        .expect("initial import should succeed");
    let imported_offset = store
        .import_summary_only_envelope(
            &envelope,
            GovernedMemoryRecordImportOptions {
                record_id: "offset-record".to_string(),
                imported_at_utc: "2026-03-23T01:00:00+01:00".to_string(),
            },
        )
        .expect("equivalent offset import should remain idempotent");

    assert_eq!(imported_z.record, imported_offset.record);
    assert_eq!(
        imported_offset.record.provenance.imported_at_utc,
        "2026-03-23T00:00:00Z"
    );
}

#[test]
fn local_store_rejects_concurrent_writer_lock_file() {
    let root = unique_temp_dir("elegy-memory-lock").join("local-store");
    let store = LocalMemoryStore::new(&root);
    let initialized = store.init().expect("local store init should succeed");

    fs::write(
        &initialized.paths.write_lock_path,
        "single-writer local store; concurrent writers are rejected",
    )
    .expect("seed write lock");

    let error = store
        .import_summary_only_envelope(
            &summary_only_fixture(),
            GovernedMemoryRecordImportOptions {
                record_id: "locked-record".to_string(),
                imported_at_utc: "2026-03-23T00:00:00Z".to_string(),
            },
        )
        .expect_err("concurrent writer lock should be rejected");

    assert!(matches!(
        error,
        LocalMemoryStoreError::ConcurrentWriterRejected { .. }
    ));
}
