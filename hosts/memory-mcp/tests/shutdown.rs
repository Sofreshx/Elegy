#![cfg(unix)]

use std::time::Duration;

use tempfile::TempDir;
use tokio::{io::AsyncReadExt, process::Command};

#[tokio::test]
async fn sigterm_produces_clean_exit() {
    let temp_dir = TempDir::new().expect("tempdir should create");
    let db_path = temp_dir.path().join("memory.db");
    std::fs::write(&db_path, b"").expect("db placeholder should write");

    let mut command = Command::new(env!("CARGO_BIN_EXE_elegy-memory-mcp-stdio"));
    command
        .env("ELEGY_DB_PATH", &db_path)
        .env("ELEGY_MCP_AGENT_ID", "shutdown-test-agent")
        .env("ELEGY_EMBEDDING_BOOT_POLICY", "off")
        .env("RUST_LOG", "info");

    let mut child = command
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("stdio child should spawn");

    let pid = child.id().expect("child should have a pid");

    let mut stderr = child.stderr.take().expect("stderr should be piped");
    let stderr_task = tokio::spawn(async move {
        let mut output = String::new();
        stderr.read_to_string(&mut output).await?;
        Ok(output)
    });

    tokio::time::sleep(Duration::from_millis(500)).await;

    Command::new("kill")
        .args(["-TERM", &pid.to_string()])
        .status()
        .await
        .expect("kill command should run");

    let exit_status = tokio::time::timeout(Duration::from_secs(10), child.wait())
        .await
        .expect("process should exit within timeout")
        .expect("wait should succeed");

    assert!(
        exit_status.success(),
        "process should exit with status 0, got {:?}",
        exit_status.code()
    );

    let stderr_output = stderr_task
        .await
        .expect("stderr task should join")
        .expect("stderr should read");

    assert!(
        stderr_output.contains("received SIGTERM, shutting down"),
        "stderr should contain SIGTERM shutdown message: {stderr_output}"
    );
}

#[tokio::test]
async fn sigint_produces_clean_exit() {
    let temp_dir = TempDir::new().expect("tempdir should create");
    let db_path = temp_dir.path().join("memory.db");
    std::fs::write(&db_path, b"").expect("db placeholder should write");

    let mut command = Command::new(env!("CARGO_BIN_EXE_elegy-memory-mcp-stdio"));
    command
        .env("ELEGY_DB_PATH", &db_path)
        .env("ELEGY_MCP_AGENT_ID", "shutdown-test-agent")
        .env("ELEGY_EMBEDDING_BOOT_POLICY", "off")
        .env("RUST_LOG", "info");

    let mut child = command
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("stdio child should spawn");

    let pid = child.id().expect("child should have a pid");

    let mut stderr = child.stderr.take().expect("stderr should be piped");
    let stderr_task = tokio::spawn(async move {
        let mut output = String::new();
        stderr.read_to_string(&mut output).await?;
        Ok(output)
    });

    tokio::time::sleep(Duration::from_millis(500)).await;

    Command::new("kill")
        .args(["-INT", &pid.to_string()])
        .status()
        .await
        .expect("kill command should run");

    let exit_status = tokio::time::timeout(Duration::from_secs(10), child.wait())
        .await
        .expect("process should exit within timeout")
        .expect("wait should succeed");

    assert!(
        exit_status.success(),
        "process should exit with status 0, got {:?}",
        exit_status.code()
    );

    let stderr_output = stderr_task
        .await
        .expect("stderr task should join")
        .expect("stderr should read");

    assert!(
        stderr_output.contains("received SIGINT, shutting down"),
        "stderr should contain SIGINT shutdown message: {stderr_output}"
    );
}
