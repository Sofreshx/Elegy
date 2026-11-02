//! Real-process MCP handshake tests for the `elegy-run` binary.
//!
//! Every prior test for this host (see `src/host.rs`'s `#[cfg(test)] mod
//! tests`) exercised `ElegyMcpHost` in-process over a `tokio::io::duplex`
//! transport, and none of them pointed `ProjectLocator` at the repository
//! root. That gap let the repo root's `elegy.toml` ship with no matching
//! descriptors (`DESC-CONFIG-002`) for days without a single test catching
//! it: the process died before answering the MCP `initialize` request, and
//! Claude Desktop only ever saw "Connection closed".
//!
//! These tests spawn the actual compiled `elegy-run` binary as a real OS
//! child process and speak real MCP stdio to it, mirroring the harness
//! already used by `hosts/memory-mcp/tests/stdio_bootstrap.rs`.

use rmcp::{
    model::ReadResourceRequestParams, transport::TokioChildProcess, ClientHandler, ServiceExt,
};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tempfile::TempDir;
use tokio::process::Command;

#[derive(Default, Clone)]
struct TestClient;

impl ClientHandler for TestClient {}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root")
        .to_path_buf()
}

fn elegy_run_command(project: &Path, extra_args: &[&str]) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_elegy-run"));
    command
        .arg("--project")
        .arg(project)
        .args(extra_args)
        .env("RUST_LOG", "info");
    command
}

/// A project with a root `elegy.toml` but no file matching its descriptor
/// include pattern — the exact shape the repo root had when this regressed.
fn write_descriptor_less_project(dir: &TempDir) {
    std::fs::write(
        dir.path().join("elegy.toml"),
        "version = 1\n\n[project]\nname = \"descriptor-less\"\n",
    )
    .expect("elegy.toml should write");
}

#[tokio::test]
async fn elegy_run_serves_real_resources_for_the_workspace_root() {
    // This is the regression test: run elegy-run against the actual
    // repository root, exactly like the Claude Desktop `elegy-planning`
    // registration does. It must complete `initialize` and list at least
    // one real (non-diagnostic) resource.
    let transport = TokioChildProcess::new(elegy_run_command(&repo_root(), &[]))
        .expect("elegy-run should spawn");

    let client = TestClient
        .serve(transport)
        .await
        .expect("elegy-run should complete the MCP initialize handshake");

    let resources = client
        .list_all_resources()
        .await
        .expect("client should list resources");

    assert!(
        !resources.is_empty(),
        "expected the repo root's elegy.resources.d descriptors to be served"
    );
    assert!(
        resources
            .iter()
            .any(|resource| resource.uri == "elegy://elegy/resource/readiness"),
        "expected the readiness resource declared in elegy.resources.d/elegy-authority.toml"
    );

    client.cancel().await.expect("client should cancel cleanly");
}

#[tokio::test]
async fn elegy_run_completes_initialize_and_reports_a_diagnostic_when_descriptors_are_missing() {
    let dir = TempDir::new().expect("tempdir should create");
    write_descriptor_less_project(&dir);

    let transport =
        TokioChildProcess::new(elegy_run_command(dir.path(), &[])).expect("elegy-run should spawn");

    // The critical assertion: initialize succeeds even though descriptor
    // composition failed. Before this fix, the process exited before this
    // handshake completed and this `.serve()` call would fail.
    let client = TestClient
        .serve(transport)
        .await
        .expect("elegy-run should complete initialize even in degraded mode");

    let resources = client
        .list_all_resources()
        .await
        .expect("client should list resources");
    assert_eq!(resources.len(), 1);
    assert_eq!(resources[0].uri, "elegy://host/diagnostics");

    let diagnostics = client
        .read_resource(ReadResourceRequestParams::new("elegy://host/diagnostics"))
        .await
        .expect("diagnostics resource should be readable");
    let text = format!("{:?}", diagnostics.contents);
    assert!(
        text.contains("DESC-CONFIG-002"),
        "expected the diagnostic resource to name the failing check, got: {text}"
    );

    client.cancel().await.expect("client should cancel cleanly");
}

#[tokio::test]
async fn preflight_exits_zero_for_the_workspace_root() {
    let output = tokio::time::timeout(
        Duration::from_secs(30),
        elegy_run_command(&repo_root(), &["--preflight"]).output(),
    )
    .await
    .expect("preflight should not hang")
    .expect("preflight command should run");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("preflight OK"));
}

#[tokio::test]
async fn preflight_exits_nonzero_when_descriptors_are_missing() {
    let dir = TempDir::new().expect("tempdir should create");
    write_descriptor_less_project(&dir);

    let output = tokio::time::timeout(
        Duration::from_secs(30),
        elegy_run_command(dir.path(), &["--preflight"]).output(),
    )
    .await
    .expect("preflight should not hang")
    .expect("preflight command should run");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("DESC-CONFIG-002"));
}

#[tokio::test]
async fn version_flag_reports_the_crate_version() {
    let output = tokio::time::timeout(
        Duration::from_secs(30),
        elegy_run_command(&repo_root(), &["--version"]).output(),
    )
    .await
    .expect("version should not hang")
    .expect("version command should run");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("elegy-run"));
}
