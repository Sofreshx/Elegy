#![allow(clippy::unwrap_used)]

use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

use serde::Deserialize;

#[derive(Deserialize)]
struct MarketplacePlugin {
    name: String,
    source: String,
    policy: String,
    manifest: String,
}

#[derive(Deserialize)]
struct Marketplace {
    #[serde(rename = "schemaVersion")]
    schema_version: String,
    plugins: Vec<MarketplacePlugin>,
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct Surface {
    name: String,
    kind: String,
    #[serde(default)]
    packaging: Option<String>,
    #[serde(rename = "pluginRoot", default)]
    plugin_root: Option<String>,
}

#[derive(Deserialize)]
struct Surfaces {
    surfaces: Vec<Surface>,
}

fn load_marketplace() -> Marketplace {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let repo_root = manifest_dir
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();
    let marketplace_path = repo_root.join("distribution").join("marketplace.json");
    let content =
        fs::read_to_string(&marketplace_path).expect("Cannot read distribution/marketplace.json");
    serde_json::from_str(&content).expect("Invalid marketplace.json")
}

fn load_surfaces() -> Surfaces {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let repo_root = manifest_dir
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();
    let surfaces_path = repo_root.join("distribution").join("surfaces.json");
    let content =
        fs::read_to_string(&surfaces_path).expect("Cannot read distribution/surfaces.json");
    serde_json::from_str(&content).expect("Invalid surfaces.json")
}

#[test]
fn marketplace_schema_version_is_correct() {
    let marketplace = load_marketplace();
    assert_eq!(marketplace.schema_version, "elegy-plugin-marketplace/v1");
}

#[test]
fn marketplace_has_seven_plugins() {
    let marketplace = load_marketplace();
    assert_eq!(marketplace.plugins.len(), 7);
}

#[test]
fn marketplace_plugins_have_valid_source_paths() {
    let marketplace = load_marketplace();
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let repo_root = manifest_dir
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();

    for plugin in &marketplace.plugins {
        let source_dir = repo_root.join(&plugin.source);
        assert!(source_dir.exists(), "Source dir missing: {}", plugin.source);

        let manifest_path = source_dir.join(&plugin.manifest);
        assert!(
            manifest_path.exists(),
            "Manifest missing: {}/{}",
            plugin.source,
            plugin.manifest
        );
    }
}

#[test]
fn marketplace_no_duplicate_plugin_names() {
    let marketplace = load_marketplace();
    let mut seen = HashSet::new();
    for plugin in &marketplace.plugins {
        assert!(
            seen.insert(&plugin.name),
            "Duplicate plugin name: {}",
            plugin.name
        );
    }
}

#[test]
fn marketplace_default_policy_correct() {
    let marketplace = load_marketplace();
    for plugin in &marketplace.plugins {
        match plugin.name.as_str() {
            "elegy-planning" | "elegy-memory" => {
                assert_eq!(
                    plugin.policy, "default",
                    "{} should be default, got {}",
                    plugin.name, plugin.policy
                );
            }
            _ => {
                assert_eq!(
                    plugin.policy, "available",
                    "{} should be available, got {}",
                    plugin.name, plugin.policy
                );
            }
        }
    }
}

#[test]
fn marketplace_matches_surfaces_json() {
    let marketplace = load_marketplace();
    let surfaces = load_surfaces();

    // All marketplace plugins should have a matching surface with packaging=plugin
    for plugin in &marketplace.plugins {
        let matching_surface = surfaces.surfaces.iter().find(|s| {
            s.name == plugin.name
                && s.packaging.as_deref() == Some("plugin")
                && s.plugin_root.as_deref() == Some(&plugin.source)
        });
        assert!(
            matching_surface.is_some(),
            "Marketplace plugin '{}' (source: {}) has no matching surface with packaging=plugin",
            plugin.name,
            plugin.source
        );
    }

    // All surfaces with packaging=plugin should have a marketplace entry
    for surface in &surfaces.surfaces {
        if surface.packaging.as_deref() == Some("plugin") {
            let matching = marketplace.plugins.iter().find(|p| {
                p.name == surface.name && p.source == surface.plugin_root.as_deref().unwrap_or("")
            });
            assert!(
                matching.is_some(),
                "Surface '{}' has packaging=plugin but no marketplace entry",
                surface.name
            );
        }
    }
}

#[test]
fn marketplace_plugin_manifests_are_valid() {
    let marketplace = load_marketplace();
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let repo_root = manifest_dir
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();

    for plugin in &marketplace.plugins {
        let manifest_path = repo_root.join(&plugin.source).join(&plugin.manifest);
        let content = fs::read_to_string(&manifest_path)
            .unwrap_or_else(|_| panic!("Cannot read manifest: {:?}", manifest_path));
        let _json: serde_json::Value = serde_json::from_str(&content)
            .unwrap_or_else(|_| panic!("Invalid JSON in manifest: {:?}", manifest_path));
    }
}

#[test]
fn marketplace_sources_are_local_relative_only() {
    let marketplace = load_marketplace();
    for plugin in &marketplace.plugins {
        assert!(
            !plugin.source.starts_with("http"),
            "Source should not be URL: {}",
            plugin.source
        );
        assert!(
            !plugin.source.starts_with("/"),
            "Source should be relative: {}",
            plugin.source
        );
        assert!(
            !plugin.source.contains(".."),
            "Source should not contain parent refs: {}",
            plugin.source
        );
    }
}
