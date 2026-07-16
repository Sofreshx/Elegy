use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use elegy_plugin_sdk::{
    validate_elegy_marketplace_v1, validate_elegy_plugin_v1, ElegyMarketplaceV1, ElegyPluginV1,
    ELEGY_MARKETPLACE_V1_SCHEMA_VERSION,
};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DistributionCatalog {
    #[serde(rename = "schemaVersion")]
    schema_version: String,
    surfaces: Vec<DistributionSurface>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DistributionSurface {
    name: String,
    kind: String,
    #[serde(default)]
    packaging: Option<String>,
    #[serde(default)]
    plugin_root: Option<String>,
    #[serde(default = "default_marketplace_published")]
    marketplace_published: bool,
}

fn default_marketplace_published() -> bool {
    true
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .expect("shared/tooling must be two directories below the repo root")
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> T {
    let content =
        fs::read_to_string(path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
    serde_json::from_str(&content)
        .unwrap_or_else(|error| panic!("parse {} as JSON: {error}", path.display()))
}

fn load_marketplace() -> ElegyMarketplaceV1 {
    read_json(&repo_root().join(".elegy").join("marketplace.json"))
}

fn load_surfaces() -> DistributionCatalog {
    read_json(&repo_root().join("distribution").join("surfaces.json"))
}

#[test]
fn generated_marketplace_is_valid() {
    let marketplace = load_marketplace();

    assert_eq!(
        marketplace.schema_version,
        ELEGY_MARKETPLACE_V1_SCHEMA_VERSION
    );
    let validation = validate_elegy_marketplace_v1(&marketplace);
    assert!(
        validation.is_valid(),
        "invalid marketplace: {}",
        validation.issues.join("; ")
    );
}

#[test]
fn distribution_catalog_uses_explicit_surface_roles() {
    let surfaces = load_surfaces();

    assert_eq!(surfaces.schema_version, "elegy-surfaces/v2");
    for surface in surfaces.surfaces {
        assert!(
            matches!(
                surface.kind.as_str(),
                "bundled-plugin"
                    | "cli"
                    | "host-adapter"
                    | "skill-package"
                    | "external-plugin-wrapper"
            ),
            "{} uses unsupported surface kind {}",
            surface.name,
            surface.kind
        );
    }
}

#[test]
fn generated_marketplace_matches_packaged_surfaces() {
    let marketplace = load_marketplace();
    let surfaces = load_surfaces();

    let expected: BTreeMap<String, String> = surfaces
        .surfaces
        .into_iter()
        .filter(|surface| {
            surface.packaging.as_deref() == Some("plugin") && surface.marketplace_published
        })
        .map(|surface| {
            let plugin_root = surface
                .plugin_root
                .unwrap_or_else(|| panic!("{} must declare pluginRoot", surface.name));
            (surface.name, format!("./{plugin_root}"))
        })
        .collect();
    let actual: BTreeMap<String, String> = marketplace
        .plugins
        .into_iter()
        .map(|plugin| (plugin.name, plugin.source.path))
        .collect();

    assert_eq!(actual, expected);
}

#[test]
fn client_radar_remains_quarantined_until_publication_is_reapproved() {
    let marketplace = load_marketplace();
    let surfaces = load_surfaces();
    let client_radar = surfaces
        .surfaces
        .iter()
        .find(|surface| surface.name == "elegy-client-radar")
        .expect("Client Radar wrapper must remain registered while quarantined");

    assert!(
        !client_radar.marketplace_published,
        "Client Radar must not be marketplace-published before its owning pilot and publication gates are explicitly reapproved"
    );
    assert!(
        marketplace
            .plugins
            .iter()
            .all(|plugin| plugin.name != "elegy-client-radar"),
        "the generated marketplace must omit quarantined Client Radar"
    );
}

#[test]
fn generated_marketplace_points_to_matching_plugin_manifests() {
    let root = repo_root();
    let marketplace = load_marketplace();
    let mut names = BTreeSet::new();

    for plugin in marketplace.plugins {
        assert!(
            names.insert(plugin.name.clone()),
            "duplicate marketplace plugin: {}",
            plugin.name
        );
        let source_path = plugin.source.path.trim_start_matches("./");
        let manifest_path = root
            .join(source_path)
            .join(".elegy-plugin")
            .join("plugin.json");
        let manifest: ElegyPluginV1 = read_json(&manifest_path);
        let validation = validate_elegy_plugin_v1(&manifest);

        assert!(
            validation.is_valid(),
            "invalid manifest {}: {}",
            manifest_path.display(),
            validation.issues.join("; ")
        );
        assert_eq!(
            manifest.name,
            plugin.name,
            "{} points to manifest {}",
            plugin.name,
            manifest_path.display()
        );
    }
}
