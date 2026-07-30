use std::{fs, path::PathBuf};

use serde_json::Value;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repository root should resolve")
}

#[test]
fn accounts_plugin_is_a_portable_bundled_capability() {
    let plugin = repo_root().join("plugins/accounts");
    let manifest_path = plugin.join(".elegy-plugin/plugin.json");
    let manifest: Value = serde_json::from_str(
        &fs::read_to_string(&manifest_path).expect("accounts plugin manifest should exist"),
    )
    .expect("accounts plugin manifest should be JSON");

    assert_eq!(manifest["schemaVersion"], "elegy-plugin/v3");
    assert_eq!(manifest["name"], "elegy-accounts");
    assert_eq!(manifest["skills"], "./skills/");
    assert_eq!(manifest["elegy"]["surfaceClass"], "adapter-plugin");
    assert_eq!(
        manifest["elegy"]["connections"]["requirements"]["mode"],
        "none"
    );
    assert_eq!(
        manifest["elegy"]["connections"]["provider"]["schemaVersion"],
        "elegy-connection-provider/v1"
    );
    assert_eq!(manifest["mcpServers"], "./.mcp.json");
    assert_eq!(
        manifest["elegy"]["mcpAuthentication"]["elegy-accounts"]["mode"],
        "none"
    );
    assert_eq!(
        manifest["elegy"]["mcpAuthentication"]["elegy-account-actions"]["mode"],
        "none"
    );
    assert_eq!(
        manifest["assets"],
        serde_json::json!(["./ui/", "./browser/", "./providers/"])
    );
    assert!(manifest["elegy"].get("packageAssets").is_none());
    assert!(manifest.get("apps").is_none());
    assert!(!plugin.join(".app.json").exists());

    let provider: Value = serde_json::from_str(
        &fs::read_to_string(plugin.join("connection-provider.json"))
            .expect("connection provider descriptor should exist"),
    )
    .expect("connection provider descriptor should be JSON");
    assert_eq!(provider["controlProtocol"], "elegy-connection-control/v1");
    assert_eq!(
        provider["invocation"]["command"],
        serde_json::json!(["broker"])
    );

    let mcp: Value = serde_json::from_str(
        &fs::read_to_string(plugin.join(".mcp.json")).expect("MCP descriptor should exist"),
    )
    .expect("MCP descriptor should be JSON");
    assert_eq!(
        mcp["mcpServers"]["elegy-accounts"]["args"],
        serde_json::json!(["mcp"])
    );
    assert_eq!(
        mcp["mcpServers"]["elegy-account-actions"]["args"],
        serde_json::json!(["actions-mcp"])
    );

    let catalog: Value = serde_json::from_str(
        &fs::read_to_string(plugin.join("capability-catalog.json"))
            .expect("capability catalog should exist"),
    )
    .expect("capability catalog should be JSON");
    assert!(catalog["capabilities"]
        .as_array()
        .expect("capabilities")
        .iter()
        .any(|capability| capability["id"] == "accounts.actions.mcp"));

    for required in [
        "skills/elegy-manage-accounts/SKILL.md",
        "capability-catalog.json",
        "connection-provider.json",
        "ui/account-center/index.html",
        "browser/brave/manifest.json",
        "DISTRIBUTION.md",
    ] {
        assert!(plugin.join(required).is_file(), "missing {required}");
    }
}
