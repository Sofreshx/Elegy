//! Regression coverage for `load_project_from_root_config`, in particular
//! the zero-match descriptor branch (`DESC-CONFIG-002`) that was previously
//! untested anywhere in the workspace and let a repo-root `elegy.toml` with
//! no matching descriptors ship silently, breaking the `elegy-run` MCP host.

use elegy_descriptor::load_project_from_root_config;
use std::fs;
use tempfile::TempDir;

fn write(dir: &TempDir, relative: &str, content: &str) {
    let path = dir.path().join(relative);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("parent dirs should create");
    }
    fs::write(path, content).expect("fixture file should write");
}

#[test]
fn zero_matching_descriptor_files_is_a_hard_error() {
    let dir = TempDir::new().expect("tempdir should create");
    write(
        &dir,
        "elegy.toml",
        "version = 1\n\n[project]\nname = \"broken\"\n",
    );

    let error = load_project_from_root_config(&dir.path().join("elegy.toml"))
        .expect_err("empty elegy.resources.d should fail to load");

    let diagnostics = error.diagnostics();
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, "DESC-CONFIG-002");
    assert_eq!(
        diagnostics[0].location.field.as_deref(),
        Some("descriptors.include")
    );
}

#[test]
fn absent_descriptors_table_falls_back_to_default_pattern() {
    let dir = TempDir::new().expect("tempdir should create");
    write(
        &dir,
        "elegy.toml",
        "version = 1\n\n[project]\nname = \"default-pattern\"\n",
    );
    write(
        &dir,
        "elegy.resources.d/main.toml",
        r#"version = 1
name = "main"

[[resources]]
kind = "static"
id = "hello"
uri = "elegy://default-pattern/resource/hello"
content = "hello"
"#,
    );

    let loaded = load_project_from_root_config(&dir.path().join("elegy.toml"))
        .expect("descriptor matching the implicit default pattern should load");

    assert_eq!(loaded.config.project_name, "default-pattern");
    assert_eq!(loaded.descriptors.len(), 1);
    assert_eq!(loaded.resources.len(), 1);
}

#[test]
fn explicit_descriptors_include_is_honored() {
    let dir = TempDir::new().expect("tempdir should create");
    write(
        &dir,
        "elegy.toml",
        r#"version = 1

[project]
name = "explicit-pattern"

[descriptors]
include = ["custom.d/*.toml"]
"#,
    );
    write(
        &dir,
        "custom.d/one.toml",
        r#"version = 1
name = "one"

[[resources]]
kind = "static"
id = "hello"
uri = "elegy://explicit-pattern/resource/hello"
content = "hello"
"#,
    );

    let loaded = load_project_from_root_config(&dir.path().join("elegy.toml"))
        .expect("descriptor matching an explicit include pattern should load");

    assert_eq!(loaded.descriptors.len(), 1);
    assert_eq!(loaded.descriptors[0].name, "one");
}

#[test]
fn invalid_glob_pattern_is_reported_as_a_config_error() {
    let dir = TempDir::new().expect("tempdir should create");
    write(
        &dir,
        "elegy.toml",
        r#"version = 1

[project]
name = "invalid-glob"

[descriptors]
include = ["["]
"#,
    );

    let error = load_project_from_root_config(&dir.path().join("elegy.toml"))
        .expect_err("an invalid glob pattern should fail to load");

    let diagnostics = error.diagnostics();
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, "DESC-CONFIG-003");
}

#[test]
fn descriptor_with_resources_present_loads_successfully() {
    let dir = TempDir::new().expect("tempdir should create");
    write(
        &dir,
        "elegy.toml",
        r#"version = 1

[project]
name = "populated"

[descriptors]
include = ["elegy.resources.d/*.toml"]
"#,
    );
    write(
        &dir,
        "elegy.resources.d/populated.toml",
        r#"version = 1
name = "populated"

[[resources]]
kind = "static"
id = "welcome"
uri = "elegy://populated/resource/welcome"
title = "Welcome"
content = "Hello from a populated descriptor.\n"
"#,
    );

    let loaded = load_project_from_root_config(&dir.path().join("elegy.toml"))
        .expect("populated descriptor should load");

    assert_eq!(loaded.config.project_name, "populated");
    assert_eq!(loaded.resources.len(), 1);
    assert_eq!(loaded.resources[0].id(), "welcome");
}
