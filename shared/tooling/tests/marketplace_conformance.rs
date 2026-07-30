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
    surface_class: String,
    lifecycle: String,
    #[serde(default)]
    packaging: Option<String>,
    #[serde(default)]
    plugin_root: Option<String>,
    #[serde(default = "default_marketplace_published")]
    marketplace_published: bool,
    #[serde(default)]
    readiness: Option<SurfaceReadinessReference>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SurfaceReadinessReference {
    path: String,
    schema_version: String,
}

#[test]
fn standalone_surfaces_declare_readiness_authority() {
    let root = repo_root();
    let surfaces = load_surfaces();

    for surface in surfaces
        .surfaces
        .iter()
        .filter(|surface| surface.packaging.as_deref() != Some("plugin"))
    {
        let readiness = surface.readiness.as_ref().unwrap_or_else(|| {
            panic!(
                "{} must reference its canonical readiness artifact",
                surface.name
            )
        });
        assert_eq!(readiness.schema_version, "elegy-readiness/v1");
        assert!(
            root.join(readiness.path.trim_start_matches("./")).is_file(),
            "{} readiness artifact does not exist: {}",
            surface.name,
            readiness.path
        );
    }
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

    assert_eq!(surfaces.schema_version, "elegy-surfaces/v3");
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
        assert!(
            matches!(
                surface.surface_class.as_str(),
                "adapter-plugin" | "tool" | "skill" | "host-adapter" | "host-extension"
            ),
            "{} uses unsupported surface class {}",
            surface.name,
            surface.surface_class
        );
        assert!(
            matches!(
                surface.lifecycle.as_str(),
                "active" | "rework" | "deprecated" | "blocked"
            ),
            "{} uses unsupported lifecycle {}",
            surface.name,
            surface.lifecycle
        );
        if surface.packaging.as_deref() == Some("plugin") {
            assert_eq!(surface.surface_class, "adapter-plugin");
            assert_eq!(surface.lifecycle, "active");
        }
    }
}

#[test]
fn generated_marketplace_matches_packaged_surfaces() {
    let root = repo_root();
    let marketplace = load_marketplace();
    let surfaces = load_surfaces();

    let expected: BTreeMap<String, String> = surfaces
        .surfaces
        .into_iter()
        .filter_map(|surface| {
            if surface.packaging.as_deref() != Some("plugin") || !surface.marketplace_published {
                return None;
            }
            let plugin_root = surface
                .plugin_root
                .unwrap_or_else(|| panic!("{} must declare pluginRoot", surface.name));
            let manifest: ElegyPluginV1 = read_json(
                &root
                    .join(&plugin_root)
                    .join(".elegy-plugin")
                    .join("plugin.json"),
            );
            manifest
                .is_agent_routable()
                .then(|| (surface.name, format!("./{plugin_root}")))
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
fn domain_products_are_not_registered_as_plugins() {
    let marketplace = load_marketplace();
    let surfaces = load_surfaces();
    let domain_products = [
        "elegy-ai-radar",
        "elegy-client-radar",
        "elegy-question-studio",
    ];

    for product in domain_products {
        assert!(
            surfaces
                .surfaces
                .iter()
                .all(|surface| surface.name != product),
            "{product} is domain/business logic and must be distributed as a library, tool, or application rather than an Elegy plugin"
        );
        assert!(
            marketplace
                .plugins
                .iter()
                .all(|plugin| plugin.name != product),
            "{product} must not be discoverable through the Elegy plugin marketplace"
        );
    }
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

        assert_eq!(
            manifest.schema_version,
            "elegy-plugin/v2",
            "published manifest {} must declare its authentication posture",
            manifest_path.display()
        );
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
