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

    assert_eq!(manifest["schemaVersion"], "elegy-plugin/v1");
    assert_eq!(manifest["name"], "elegy-accounts");
    assert_eq!(manifest["skills"], "./skills/");
    assert_eq!(
        manifest["extensions"]["codex.plugin/v1"]["mcpServers"],
        "./.mcp.json"
    );
    assert!(manifest.get("apps").is_none());
    assert!(!plugin.join(".app.json").exists());

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
        "ui/account-center/index.html",
        "browser/brave/manifest.json",
        "DISTRIBUTION.md",
    ] {
        assert!(plugin.join(required).is_file(), "missing {required}");
    }
}
