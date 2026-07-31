use std::{fs, path::PathBuf};

use elegy_plugin_sdk::{
    load_capability_catalog, validate_elegy_capability_catalog_v2, ElegyCapabilityCatalog,
    ElegyCapabilityV2, ElegySideEffectClass,
};
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
    assert_eq!(catalog["schemaVersion"], "elegy-capability-catalog/v2");

    let loaded = load_capability_catalog(&plugin.join("capability-catalog.json"))
        .expect("capability catalog should load");
    let ElegyCapabilityCatalog::V2(catalog) = loaded else {
        panic!("Accounts catalog must use v2");
    };
    assert!(validate_elegy_capability_catalog_v2(&catalog).is_valid());
    let ids = catalog
        .capabilities
        .iter()
        .map(|capability| capability.common().id.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(ids.len(), 18);
    assert!(ids.contains("accounts.center"));
    for id in [
        "account_list",
        "account_discover",
        "account_require",
        "account_request_access",
        "account_request_creation",
        "account_request_status",
        "account_attention_list",
        "account_present",
        "account_cancel_request",
        "account_resume_request",
        "account_open_center",
        "account_revoke_grant",
        "account_audit_list",
        "github_profile_read",
        "github_repositories_read",
        "cloudflare_zones_read",
        "cloudflare_dns_records_read",
    ] {
        assert!(ids.contains(id), "missing tool {id}");
    }
    for capability in &catalog.capabilities {
        let encoded = serde_json::to_value(capability).expect("v2 capability should serialize");
        assert!(encoded.get("fallback").is_none());
        assert!(encoded.get("appBinding").is_none());
        match capability {
            ElegyCapabilityV2::Cli { common, .. } => {
                assert_eq!(common.id, "accounts.center");
                assert_eq!(common.side_effect_class, ElegySideEffectClass::Mutation);
            }
            ElegyCapabilityV2::McpTool { common, .. } => {
                assert_eq!(encoded["toolName"], common.id);
                assert_eq!(encoded["inputSchema"]["type"], "object");
                assert_eq!(encoded["outputSchema"]["type"], "object");
                assert_eq!(common.readiness.as_str(), "implemented");
                let expected = matches!(
                    common.id.as_str(),
                    "account_request_access"
                        | "account_request_creation"
                        | "account_present"
                        | "account_cancel_request"
                        | "account_resume_request"
                        | "account_revoke_grant"
                        | "account_open_center"
                );
                assert_eq!(
                    common.side_effect_class == ElegySideEffectClass::Mutation,
                    expected,
                    "wrong side effect for {}",
                    common.id
                );
            }
            ElegyCapabilityV2::McpResource { .. } => {
                panic!("Accounts catalog has no MCP resources")
            }
        }
    }

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
