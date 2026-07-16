use std::fs;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .expect("shared/tooling must be two directories below the repo root")
}

#[test]
fn reusable_external_plugin_workflow_is_immutable_and_fail_closed() {
    let path = repo_root()
        .join(".github")
        .join("workflows")
        .join("publish-external-plugin.yml");
    let workflow = fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));

    assert!(workflow.contains("workflow_call:"));
    assert!(!workflow.contains("workflow_dispatch:"));
    assert!(!workflow.contains("\n  push:"));
    assert!(workflow.contains("marketplace_eligible:"));
    assert!(workflow.contains("type: boolean"));
    assert!(workflow.contains("required: true"));
    assert!(workflow.contains("refs/tags/"));
    assert!(workflow.contains("main-snapshot"));
    assert!(workflow.contains("--clobber"));
    assert!(workflow.contains("gh release upload $env:RELEASE_TAG --repo Sofreshx/Elegy @assets"));
    assert!(workflow.contains("actions/checkout@34e114876b0b11c390a56381ad16ebd13914f8d5"));
    assert!(workflow.contains("dtolnay/rust-toolchain@4cda84d5c5c54efe2404f9d843567869ab1699d4"));
    assert!(workflow.contains("toolchain: 1.88.0"));
    assert!(workflow.contains("ELEGY_RELEASE_TOKEN:"));
}
