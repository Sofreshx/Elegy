#![cfg(unix)]

use std::{process::Stdio, time::Duration};

use rmcp::{ClientHandler, ServiceExt};
use tempfile::TempDir;
use tokio::{io::AsyncReadExt, process::Command};

#[derive(Default)]
struct TestClient;

impl ClientHandler for TestClient {}

async fn run_shutdown_signal(signal: &str) -> String {
    let temp_dir = TempDir::new().expect("tempdir should create");
    let db_path = temp_dir.path().join("memory.db");
    std::fs::write(&db_path, b"").expect("db placeholder should write");

    let mut command = Command::new(env!("CARGO_BIN_EXE_elegy-memory-mcp-stdio"));
    command
        .env("ELEGY_DB_PATH", &db_path)
        .env("ELEGY_MCP_AGENT_ID", "shutdown-test-agent")
        .env("ELEGY_EMBEDDING_BOOT_POLICY", "off")
        .env("RUST_LOG", "info")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);

    let mut child = command.spawn().expect("stdio child should spawn");

    let pid = child.id().expect("child should have a pid");
    let child_stdin = child.stdin.take().expect("stdin should be piped");
    let child_stdout = child.stdout.take().expect("stdout should be piped");
    let mut stderr = child.stderr.take().expect("stderr should be piped");
    let stderr_task = tokio::spawn(async move {
        let mut output = String::new();
        stderr.read_to_string(&mut output).await?;
        Ok::<String, std::io::Error>(output)
    });

    let client = tokio::time::timeout(
        Duration::from_secs(10),
        TestClient.serve((child_stdout, child_stdin)),
    )
    .await
    .expect("stdio client should initialize within timeout")
    .expect("stdio client should initialize");

    tokio::time::timeout(Duration::from_secs(10), client.peer().list_tools(None))
        .await
        .expect("list-tools should complete within timeout")
        .expect("list-tools should succeed");

    let kill_status = Command::new("kill")
        .args([signal, &pid.to_string()])
        .status()
        .await
        .expect("kill command should run");
    assert!(kill_status.success(), "kill command should succeed");

    let exit_status = tokio::time::timeout(Duration::from_secs(10), child.wait())
        .await
        .expect("process should exit within timeout")
        .expect("wait should succeed");

    assert!(
        exit_status.success(),
        "process should exit with status 0, got {:?}",
        exit_status.code()
    );

    drop(client);
    tokio::time::timeout(Duration::from_secs(10), stderr_task)
        .await
        .expect("stderr task should join within timeout")
        .expect("stderr task should join")
        .expect("stderr should read")
}

#[tokio::test]
async fn sigterm_produces_clean_exit() {
    let stderr_output = run_shutdown_signal("-TERM").await;

    assert!(
        stderr_output.contains("received SIGTERM, shutting down"),
        "stderr should contain SIGTERM shutdown message: {stderr_output}"
    );
}

#[tokio::test]
async fn sigint_produces_clean_exit() {
    let stderr_output = run_shutdown_signal("-INT").await;

    assert!(
        stderr_output.contains("received SIGINT, shutting down"),
        "stderr should contain SIGINT shutdown message: {stderr_output}"
    );
}
