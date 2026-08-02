use clap::{Parser, Subcommand};
use elegy_plugin_sdk::capability_package::{
    canonical_json_sha256, pack_elegy_package_v1, validate_elegy_lock_v1,
    validate_elegy_package_v1, validate_elegy_sbom_v1, verify_elegy_package_v1, ElegyLockV1,
    ElegyPackageEntrypointKind, ElegyPackageEntrypointV1, ElegyPackageFileV1,
    ElegyPackageProvenanceV1, ElegyPackagePublisherV1, ElegyPackageV1, ElegySbomFileV1,
    ElegySbomV1, ELEGY_LOCK_V1_SCHEMA_VERSION, ELEGY_PACKAGE_V1_SCHEMA_VERSION,
};
use elegy_plugin_sdk::{
    load_capability_catalog, validate_elegy_capability_catalog_v2, validate_elegy_plugin_v3,
    ElegyCapabilityCatalog, ElegyCapabilityCatalogV2, ElegyCapabilityV2, ElegyPluginV3,
    ElegyReadinessV1, ELEGY_CAPABILITY_CATALOG_V2_SCHEMA_VERSION,
    ELEGY_READINESS_V1_SCHEMA_VERSION,
};
use elegy_tooling::capability_installer::{
    install_elegy_package_from_archive, uninstall_elegy_package, update_elegy_package_from_archive,
    verify_elegy_installation,
};
use serde::Serialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

#[derive(Parser)]
#[command(name = "elegy")]
#[command(about = "Create, validate, package, and lock AI capability packages")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Create a host-neutral package scaffold.
    Init {
        #[arg(long)]
        name: String,
        #[arg(long)]
        output: PathBuf,
        #[arg(long, default_value = "0.1.0")]
        version: String,
        #[arg(long, default_value = "Apache-2.0")]
        license: String,
    },
    /// Validate a host-neutral package and its capability catalog.
    Check {
        #[arg(long, default_value = ".")]
        package: PathBuf,
    },
    /// Run package contract checks without creating an archive.
    Test {
        #[arg(long, default_value = ".")]
        package: PathBuf,
    },
    /// Create a deterministic host-neutral package archive.
    Pack {
        #[arg(long, default_value = ".")]
        package: PathBuf,
        #[arg(long)]
        output: PathBuf,
        /// Optional sidecar SBOM path. Defaults to <archive>.sbom.json.
        #[arg(long)]
        sbom_output: Option<PathBuf>,
        #[arg(long)]
        source_commit: Option<String>,
        #[arg(long)]
        build_workflow: Option<String>,
        #[arg(long)]
        builder: Option<String>,
    },
    /// Validate an exact agent lock file.
    Lock {
        #[command(subcommand)]
        command: LockCommand,
    },
    /// Install an exact package selected by an agent lock.
    Install {
        #[arg(long)]
        archive: PathBuf,
        #[arg(long)]
        lock: PathBuf,
        #[arg(long, default_value = "any")]
        target: String,
        #[arg(long)]
        install_root: PathBuf,
        /// Replace an existing verified package atomically.
        #[arg(long)]
        update: bool,
    },
    /// Remove an installed package only after exact lock verification.
    Uninstall {
        #[arg(long)]
        package: PathBuf,
        #[arg(long)]
        lock: PathBuf,
        #[arg(long, default_value = "any")]
        target: String,
    },
    /// Verify a package directory, optionally against an exact installed lock.
    Verify {
        #[arg(long)]
        package: PathBuf,
        #[arg(long)]
        lock: Option<PathBuf>,
        #[arg(long, default_value = "any")]
        target: String,
    },
    /// Generate a host projection from the canonical package.
    Project {
        #[arg(long)]
        package: PathBuf,
        #[arg(long)]
        host: String,
        #[arg(long)]
        output: PathBuf,
        #[arg(long)]
        lock: Option<PathBuf>,
        #[arg(long, default_value = "any")]
        target: String,
        /// Permit projecting mutation and fenced-mutation capabilities.
        #[arg(long)]
        allow_side_effects: bool,
    },
}

#[derive(Subcommand)]
enum LockCommand {
    Verify {
        #[arg(long)]
        lock: PathBuf,
    },
    Create {
        #[arg(long)]
        package: PathBuf,
        #[arg(long)]
        archive: PathBuf,
        #[arg(long)]
        agent_id: String,
        #[arg(long, default_value = "any")]
        target: String,
        #[arg(long)]
        source: Option<String>,
        #[arg(long)]
        publisher: Option<String>,
        #[arg(long = "capability")]
        capabilities: Vec<String>,
        #[arg(long)]
        output: PathBuf,
    },
}

struct LockCreateRequest<'a> {
    package_root: &'a Path,
    archive: &'a Path,
    agent_id: &'a str,
    target: &'a str,
    source: Option<&'a str>,
    publisher: Option<&'a str>,
    capabilities: &'a [String],
    output: &'a Path,
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(message) => {
            println!("{message}");
            ExitCode::SUCCESS
        }
        Err(message) => {
            eprintln!("Error: {message}");
            ExitCode::from(2)
        }
    }
}

fn run(cli: Cli) -> Result<String, String> {
    match cli.command {
        Command::Init {
            name,
            output,
            version,
            license,
        } => init_package(&name, &version, &license, &output),
        Command::Check { package } => {
            check_package(&package)?;
            Ok(format!("Package at {} is valid.", package.display()))
        }
        Command::Test { package } => {
            test_package(&package)?;
            Ok(format!("Package tests passed at {}.", package.display()))
        }
        Command::Pack {
            package,
            output,
            sbom_output,
            source_commit,
            build_workflow,
            builder,
        } => {
            apply_provenance(
                &package,
                source_commit.as_deref(),
                build_workflow.as_deref(),
                builder.as_deref(),
            )?;
            materialize_file_digests(&package)?;
            check_package(&package)?;
            let digest =
                pack_elegy_package_v1(&package, &output).map_err(|error| error.to_string())?;
            let sbom_path = sbom_output
                .unwrap_or_else(|| PathBuf::from(format!("{}.sbom.json", output.display())));
            write_sbom(&package, &output, &digest, &sbom_path)?;
            Ok(format!(
                "Packed {} (sha256: {digest}); SBOM: {}.",
                output.display(),
                sbom_path.display()
            ))
        }
        Command::Lock { command } => match command {
            LockCommand::Verify { lock } => verify_lock(&lock),
            LockCommand::Create {
                package,
                archive,
                agent_id,
                target,
                source,
                publisher,
                capabilities,
                output,
            } => create_lock(LockCreateRequest {
                package_root: &package,
                archive: &archive,
                agent_id: &agent_id,
                target: &target,
                source: source.as_deref(),
                publisher: publisher.as_deref(),
                capabilities: &capabilities,
                output: &output,
            }),
        },
        Command::Install {
            archive,
            lock,
            target,
            install_root,
            update,
        } => {
            let lock = read_lock(&lock)?;
            let receipt = if update {
                update_elegy_package_from_archive(&archive, &install_root, &lock, &target)
                    .map_err(|error| error.to_string())?
            } else {
                install_elegy_package_from_archive(&archive, &install_root, &lock, &target)
                    .map_err(|error| error.to_string())?
            };
            Ok(format!(
                "Installed {} v{} to {}.",
                receipt.name, receipt.version, receipt.install_dir
            ))
        }
        Command::Uninstall {
            package,
            lock,
            target,
        } => {
            let lock = read_lock(&lock)?;
            let receipt = uninstall_elegy_package(&package, &lock, &target)
                .map_err(|error| error.to_string())?;
            Ok(format!(
                "Uninstalled {} v{} from {}.",
                receipt.name,
                receipt.version,
                package.display()
            ))
        }
        Command::Verify {
            package,
            lock,
            target,
        } => {
            if let Some(lock) = lock {
                let lock = read_lock(&lock)?;
                verify_elegy_installation(&package, &lock, &target)
                    .map_err(|error| error.to_string())?;
                Ok(format!(
                    "Installed package at {} is verified.",
                    package.display()
                ))
            } else {
                check_package(&package)?;
                Ok(format!("Package at {} is valid.", package.display()))
            }
        }
        Command::Project {
            package,
            host,
            output,
            lock,
            target,
            allow_side_effects,
        } => project_package(
            &package,
            &host,
            &output,
            lock.as_deref(),
            &target,
            allow_side_effects,
        ),
    }
}

fn init_package(name: &str, version: &str, license: &str, output: &Path) -> Result<String, String> {
    validate_name(name)?;
    semver::Version::parse(version).map_err(|_| "version must be valid SemVer".to_string())?;
    if license.trim().is_empty() {
        return Err("license must not be empty".to_string());
    }
    fs::create_dir_all(output).map_err(|error| format!("create {}: {error}", output.display()))?;
    fs::create_dir_all(output.join("bin"))
        .map_err(|error| format!("create bin directory: {error}"))?;
    fs::create_dir_all(output.join("skills").join(name))
        .map_err(|error| format!("create skills directory: {error}"))?;

    let manifest = ElegyPackageV1 {
        schema_version: ELEGY_PACKAGE_V1_SCHEMA_VERSION.to_string(),
        name: name.to_string(),
        version: version.to_string(),
        description: format!("Host-neutral capability package for {name}."),
        publisher: ElegyPackagePublisherV1 {
            name: "Replace with publisher name".to_string(),
            repository: "https://github.com/replace/me".to_string(),
            workflow_identity: None,
        },
        license: license.to_string(),
        targets: vec!["any".to_string()],
        capability_catalog: "./capability-catalog.json".to_string(),
        readiness: Some("./readiness.json".to_string()),
        entrypoints: vec![ElegyPackageEntrypointV1 {
            id: format!("{name}-cli"),
            kind: ElegyPackageEntrypointKind::Cli,
            executable: format!("./bin/{name}"),
            command: vec!["--json".to_string()],
        }],
        files: vec![
            ElegyPackageFileV1 {
                path: format!("./bin/{name}"),
                role: "executable".to_string(),
                sha256: None,
            },
            ElegyPackageFileV1 {
                path: "./capability-catalog.json".to_string(),
                role: "capability-catalog".to_string(),
                sha256: None,
            },
            ElegyPackageFileV1 {
                path: "./readiness.json".to_string(),
                role: "readiness".to_string(),
                sha256: None,
            },
            ElegyPackageFileV1 {
                path: format!("./skills/{name}/SKILL.md"),
                role: "skill".to_string(),
                sha256: None,
            },
        ],
        skills: vec![format!("./skills/{name}/SKILL.md")],
        provenance: None,
    };

    write_json(&output.join("elegy-package.json"), &manifest)?;
    write_json(
        &output.join("capability-catalog.json"),
        &json!({
            "schemaVersion": ELEGY_CAPABILITY_CATALOG_V2_SCHEMA_VERSION,
            "plugin": name,
            "pluginVersion": version,
            "capabilities": [{
                "kind": "cli",
                "id": format!("{name}.example"),
                "description": "Replace this example capability before publishing.",
                "contractVersion": "v1",
                "sideEffectClass": "query",
                "readiness": "concept",
                "invocation": {
                    "executable": format!("./bin/{name}"),
                    "command": ["--json"],
                    "inputSchema": {"type": "object"},
                    "outputSchema": {"type": "object"}
                }
            }]
        }),
    )?;
    write_json(
        &output.join("readiness.json"),
        &json!({
            "schemaVersion": "elegy-readiness/v1",
            "surface": name,
            "surfaceVersion": version,
            "stage": "concept",
            "summary": "Authoring scaffold; replace every generated example before publishing.",
            "worksToday": ["The package scaffold can be inspected."],
            "limitations": ["No executable or real task evidence exists yet."],
            "supportedEnvironments": ["any"],
            "installation": "Build and install the completed package.",
            "invocation": format!("Run the {name} CLI with JSON input."),
            "evidence": []
        }),
    )?;
    fs::write(
        output.join("skills").join(name).join("SKILL.md"),
        format!("# {name}\n\nReplace this scaffold with concise workflow guidance.\n"),
    )
    .map_err(|error| format!("write skill: {error}"))?;

    Ok(format!(
        "Created package scaffold at {}. Build the executable and replace generated examples before running elegy check.",
        output.display()
    ))
}

fn check_package(root: &Path) -> Result<(), String> {
    let package = verify_elegy_package_v1(root).map_err(|error| error.to_string())?;
    if let Some(readiness_path) = &package.readiness {
        let readiness_path = package_path(root, readiness_path)?;
        let readiness: ElegyReadinessV1 = serde_json::from_slice(
            &fs::read(&readiness_path)
                .map_err(|error| format!("read {}: {error}", readiness_path.display()))?,
        )
        .map_err(|error| format!("parse package readiness: {error}"))?;
        let mut issues = readiness.validation_issues();
        if readiness.surface != package.name {
            issues.push("readiness surface does not match package name".to_string());
        }
        if readiness.surface_version != package.version {
            issues.push("readiness surfaceVersion does not match package version".to_string());
        }
        if !issues.is_empty() {
            return Err(issues.join("; "));
        }
    }
    let catalog_path = package_path(root, &package.capability_catalog)?;
    let declared_files = package
        .files
        .iter()
        .map(|file| file.path.as_str())
        .collect::<BTreeSet<_>>();
    let catalog = load_capability_catalog(&catalog_path).map_err(|error| error.to_string())?;
    match catalog {
        ElegyCapabilityCatalog::V2(catalog) => {
            let validation = validate_elegy_capability_catalog_v2(&catalog);
            if !validation.is_valid() {
                return Err(validation.issues.join("; "));
            }
            if catalog.plugin != package.name || catalog.plugin_version != package.version {
                return Err(
                    "capability catalog identity does not match package identity".to_string(),
                );
            }
            for capability in &catalog.capabilities {
                if let ElegyCapabilityV2::Cli { common, invocation } = capability {
                    if !declared_files.contains(invocation.executable.as_str()) {
                        return Err(format!(
                            "capabilities.{}.invocation.executable '{}' must be declared in package files",
                            common.id, invocation.executable
                        ));
                    }
                    if !invocation
                        .input_schema
                        .as_ref()
                        .is_some_and(serde_json::Value::is_object)
                    {
                        return Err(format!(
                            "capabilities.{}.invocation.inputSchema must be a JSON object",
                            common.id
                        ));
                    }
                    if !invocation
                        .output_schema
                        .as_ref()
                        .is_some_and(serde_json::Value::is_object)
                    {
                        return Err(format!(
                            "capabilities.{}.invocation.outputSchema must be a JSON object",
                            common.id
                        ));
                    }
                }
            }
        }
        ElegyCapabilityCatalog::V1(_) => {
            return Err("publishable capability packages require capability-catalog/v2".to_string())
        }
    }
    Ok(())
}

fn materialize_file_digests(root: &Path) -> Result<(), String> {
    let manifest_path = root.join("elegy-package.json");
    let raw = fs::read(&manifest_path)
        .map_err(|error| format!("read {}: {error}", manifest_path.display()))?;
    let mut package: ElegyPackageV1 = serde_json::from_slice(&raw)
        .map_err(|error| format!("parse {}: {error}", manifest_path.display()))?;
    let issues = validate_elegy_package_v1(&package);
    if !issues.is_empty() {
        return Err(issues.join("; "));
    }
    let mut changed = false;
    for file in &mut package.files {
        let path = package_path(root, &file.path)?;
        let bytes = fs::read(&path).map_err(|error| format!("read {}: {error}", path.display()))?;
        let digest = format!("{:x}", Sha256::digest(bytes));
        if file.sha256.as_deref() != Some(digest.as_str()) {
            file.sha256 = Some(digest);
            changed = true;
        }
    }
    if changed {
        write_json(&manifest_path, &package)?;
    }
    Ok(())
}

fn package_executable_digests(
    package: &ElegyPackageV1,
) -> Result<BTreeMap<String, String>, String> {
    let mut digests = BTreeMap::new();
    for entrypoint in &package.entrypoints {
        let digest = package
            .files
            .iter()
            .find(|file| file.path == entrypoint.executable)
            .and_then(|file| file.sha256.as_deref())
            .ok_or_else(|| {
                format!(
                    "entrypoint executable '{}' must have a materialized sha256 before lock creation",
                    entrypoint.executable
                )
            })?;
        digests.insert(entrypoint.executable.clone(), digest.to_string());
    }
    Ok(digests)
}

fn test_package(root: &Path) -> Result<(), String> {
    materialize_file_digests(root)?;
    check_package(root)?;
    let package = verify_elegy_package_v1(root).map_err(|error| error.to_string())?;
    let catalog_path = package_path(root, &package.capability_catalog)?;
    let catalog = load_capability_catalog(&catalog_path).map_err(|error| error.to_string())?;
    let ElegyCapabilityCatalog::V2(catalog) = catalog else {
        return Err("package tests require capability-catalog/v2".to_string());
    };
    let routable = catalog
        .capabilities
        .iter()
        .filter(|capability| capability.common().readiness.is_agent_routable())
        .count();
    if routable == 0 {
        return Err("package must declare at least one routable capability".to_string());
    }
    let Some(readiness_path) = &package.readiness else {
        return Err(
            "agent-routable packages must declare a readiness artifact with proof".to_string(),
        );
    };
    let readiness_bytes = fs::read(package_path(root, readiness_path)?)
        .map_err(|error| format!("read package readiness: {error}"))?;
    let readiness: ElegyReadinessV1 = serde_json::from_slice(&readiness_bytes)
        .map_err(|error| format!("parse package readiness: {error}"))?;
    if !readiness.is_agent_routable() {
        return Err(
            "agent-routable capabilities require usable or production readiness evidence"
                .to_string(),
        );
    }
    Ok(())
}

fn verify_lock(path: &Path) -> Result<String, String> {
    let _ = read_lock(path)?;
    Ok(format!("Lock {} is valid.", path.display()))
}

fn read_lock(path: &Path) -> Result<ElegyLockV1, String> {
    let raw = fs::read(path).map_err(|error| format!("read {}: {error}", path.display()))?;
    let lock: ElegyLockV1 = serde_json::from_slice(&raw)
        .map_err(|error| format!("parse {}: {error}", path.display()))?;
    if lock.schema_version != ELEGY_LOCK_V1_SCHEMA_VERSION {
        return Err(format!(
            "schemaVersion must be '{}'.",
            ELEGY_LOCK_V1_SCHEMA_VERSION
        ));
    }
    let issues = validate_elegy_lock_v1(&lock);
    if !issues.is_empty() {
        return Err(issues.join("; "));
    }
    Ok(lock)
}

fn create_lock(request: LockCreateRequest<'_>) -> Result<String, String> {
    materialize_file_digests(request.package_root)?;
    test_package(request.package_root)?;
    if request.capabilities.is_empty() {
        return Err("lock creation requires at least one --capability allowlist entry".to_string());
    }
    let package =
        verify_elegy_package_v1(request.package_root).map_err(|error| error.to_string())?;
    let catalog_path = package_path(request.package_root, &package.capability_catalog)?;
    let catalog_raw = fs::read(&catalog_path)
        .map_err(|error| format!("read {}: {error}", catalog_path.display()))?;
    let catalog_value: Value = serde_json::from_slice(&catalog_raw)
        .map_err(|error| format!("parse {}: {error}", catalog_path.display()))?;
    let catalog: ElegyCapabilityCatalogV2 = serde_json::from_value(catalog_value.clone())
        .map_err(|error| format!("parse capability catalog: {error}"))?;
    let declared = catalog
        .capabilities
        .iter()
        .map(|capability| capability.common().id.as_str())
        .collect::<BTreeSet<_>>();
    if let Some(missing) = request
        .capabilities
        .iter()
        .find(|capability| !declared.contains(capability.as_str()))
    {
        return Err(format!(
            "lock capability '{missing}' is not declared in the catalog"
        ));
    }
    let archive_raw = fs::read(request.archive)
        .map_err(|error| format!("read {}: {error}", request.archive.display()))?;
    let archive_sha256 = format!("{:x}", Sha256::digest(&archive_raw));
    let manifest_sha256 = canonical_json_sha256(&package).map_err(|error| error.to_string())?;
    let capability_catalog_sha256 =
        canonical_json_sha256(&catalog_value).map_err(|error| error.to_string())?;
    let executable_digests = package_executable_digests(&package)?;
    let repository = package.publisher.repository.clone();
    if let Some(publisher) = request.publisher {
        if publisher != repository {
            return Err(format!(
                "--publisher must match package publisher repository ({repository})"
            ));
        }
    }
    let lock = ElegyLockV1 {
        schema_version: ELEGY_LOCK_V1_SCHEMA_VERSION.to_string(),
        agent_id: request.agent_id.to_string(),
        packages: vec![
            elegy_plugin_sdk::capability_package::ElegyCapabilityReferenceV1 {
                name: package.name,
                version: package.version,
                target: request.target.to_string(),
                source: request.source.unwrap_or(&repository).to_string(),
                archive_sha256,
                manifest_sha256,
                capability_catalog_sha256,
                executable_digests,
                publisher: request.publisher.unwrap_or(&repository).to_string(),
                allowed_capabilities: request.capabilities.to_vec(),
            },
        ],
    };
    let issues = validate_elegy_lock_v1(&lock);
    if !issues.is_empty() {
        return Err(issues.join("; "));
    }
    let archive_check_root = tempfile::tempdir()
        .map_err(|error| format!("create temporary archive verification root: {error}"))?;
    install_elegy_package_from_archive(
        request.archive,
        archive_check_root.path(),
        &lock,
        request.target,
    )
    .map_err(|error| format!("archive does not match the package and lock: {error}"))?;
    write_json(request.output, &lock)?;
    Ok(format!(
        "Created exact lock at {}.",
        request.output.display()
    ))
}

fn project_package(
    package_root: &Path,
    host: &str,
    output: &Path,
    lock_path: Option<&Path>,
    target: &str,
    allow_side_effects: bool,
) -> Result<String, String> {
    materialize_file_digests(package_root)?;
    check_package(package_root)?;
    let package = verify_elegy_package_v1(package_root).map_err(|error| error.to_string())?;
    let lock = lock_path.map(read_lock).transpose()?;
    if let Some(lock) = &lock {
        verify_elegy_installation(package_root, lock, target).map_err(|error| {
            format!("locked projections require a verified installation: {error}")
        })?;
    }
    let package_reference = lock.as_ref().and_then(|lock| {
        lock.packages
            .iter()
            .find(|reference| reference.name == package.name)
    });
    if let Some(reference) = package_reference {
        if reference.version != package.version {
            return Err("lock version does not match package projection".to_string());
        }
    } else if lock.is_some() {
        return Err("lock does not contain the projected package".to_string());
    }
    let catalog_path = package_path(package_root, &package.capability_catalog)?;
    let catalog = match load_capability_catalog(&catalog_path).map_err(|error| error.to_string())? {
        ElegyCapabilityCatalog::V2(catalog) => catalog,
        ElegyCapabilityCatalog::V1(_) => {
            return Err("host projections require capability-catalog/v2".to_string());
        }
    };
    let allowed = package_reference.map(|reference| {
        reference
            .allowed_capabilities
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>()
    });
    for capability in &catalog.capabilities {
        let selected = capability.common().readiness.is_agent_routable()
            && allowed
                .as_ref()
                .is_none_or(|ids| ids.contains(&capability.common().id));
        if selected && !matches!(capability, ElegyCapabilityV2::Cli { .. }) {
            return Err(format!(
                "capability '{}' is a native MCP surface; generic CLI projection would be lossy",
                capability.common().id
            ));
        }
    }
    let capabilities = catalog
        .capabilities
        .iter()
        .filter(|capability| capability.common().readiness.is_agent_routable())
        .filter(|capability| {
            allowed
                .as_ref()
                .is_none_or(|ids| ids.contains(&capability.common().id))
        })
        .map(capability_projection)
        .collect::<Result<Vec<_>, _>>()?;
    let has_side_effects = capabilities.iter().any(|capability| {
        matches!(
            capability["sideEffectClass"].as_str(),
            Some("mutation") | Some("fenced-mutation")
        )
    });
    if has_side_effects && !allow_side_effects {
        return Err(
            "projection includes side-effecting capabilities; rerun with --allow-side-effects"
                .to_string(),
        );
    }
    let package_digest = canonical_json_sha256(&package).map_err(|error| error.to_string())?;
    fs::create_dir_all(output).map_err(|error| format!("create {}: {error}", output.display()))?;
    let manifest_bytes = serde_json::to_vec_pretty(&package)
        .map_err(|error| format!("serialize package manifest: {error}"))?;
    fs::write(output.join("elegy-package.json"), manifest_bytes)
        .map_err(|error| format!("write projected package manifest: {error}"))?;
    for file in &package.files {
        let source = package_path(package_root, &file.path)?;
        let relative = file.path.trim_start_matches("./");
        let destination = output.join(relative);
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("create {}: {error}", parent.display()))?;
        }
        fs::copy(&source, &destination).map_err(|error| {
            format!(
                "copy projected package file {} to {}: {error}",
                source.display(),
                destination.display()
            )
        })?;
    }
    if let Some(lock_path) = lock_path {
        fs::copy(lock_path, output.join("elegy.lock.json"))
            .map_err(|error| format!("copy exact lock: {error}"))?;
        fs::copy(
            package_root.join("capability-install-receipt.json"),
            output.join("capability-install-receipt.json"),
        )
        .map_err(|error| format!("copy install receipt: {error}"))?;
    }
    let mut mcp_args = vec!["--package".to_string(), ".".to_string()];
    if lock_path.is_some() {
        mcp_args.extend([
            "--lock".to_string(),
            "./elegy.lock.json".to_string(),
            "--target".to_string(),
            target.to_string(),
        ]);
    }
    if allow_side_effects {
        mcp_args.push("--allow-side-effects".to_string());
    }
    let mcp_config = json!({
        "mcpServers": {
            package.name.clone(): {
                "command": "elegy-capability-mcp",
                "args": mcp_args,
                "x-elegy": {
                    "package": package.name.clone(),
                    "version": package.version.clone(),
                    "capabilityIds": capabilities.iter().filter_map(|value| value.get("id")).cloned().collect::<Vec<_>>()
                }
            }
        }
    });
    let projection = json!({
        "schemaVersion": "elegy-projection/v1",
        "host": host,
        "package": {
            "name": package.name,
            "version": package.version,
            "manifestSha256": package_digest,
            "publisher": package.publisher.repository,
            "capabilityCatalog": package.capability_catalog,
            "capabilities": capabilities
        }
    });
    match host {
        "mcp" => write_json(&output.join("mcp.json"), &mcp_config)?,
        "codex" => {
            write_json(&output.join(".mcp.json"), &mcp_config)?;
            let readiness_path = package.readiness.as_ref().ok_or_else(|| {
                "Codex projection requires a declared package readiness artifact".to_string()
            })?;
            let readiness_value: Value = serde_json::from_slice(
                &fs::read(package_path(package_root, readiness_path)?).map_err(|error| {
                    format!("read package readiness {}: {error}", readiness_path)
                })?,
            )
            .map_err(|error| format!("parse package readiness: {error}"))?;
            let stage = readiness_value["stage"].as_str().ok_or_else(|| {
                "package readiness must contain a string stage for Codex projection".to_string()
            })?;
            if !matches!(stage, "concept" | "implemented" | "usable" | "production") {
                return Err(format!("unsupported package readiness stage '{stage}'"));
            }
            let codex_value = json!({
                "schemaVersion": "elegy-plugin/v3",
                "name": package.name,
                "version": package.version,
                "description": "Derived Codex projection for an Elegy capability package.",
                "repository": package.publisher.repository,
                "license": package.license,
                "skills": if package.skills.is_empty() { Value::Null } else { Value::Array(package.skills.iter().cloned().map(Value::String).collect()) },
                "mcpServers": "./.mcp.json",
                "elegy": {
                    "surfaceClass": "adapter-plugin",
                    "capabilityCatalog": {
                        "path": package.capability_catalog,
                        "schemaVersion": ELEGY_CAPABILITY_CATALOG_V2_SCHEMA_VERSION
                    },
                    "connections": {"requirements": {"mode": "none"}},
                    "readiness": {
                        "stage": stage,
                        "path": readiness_path,
                        "schemaVersion": ELEGY_READINESS_V1_SCHEMA_VERSION
                    },
                    "mcpAuthentication": {},
                    "packageAssets": package.files.iter().map(|file| file.path.clone()).collect::<Vec<_>>()
                },
                "x-elegy-package": projection
            });
            let codex: ElegyPluginV3 = serde_json::from_value(codex_value.clone())
                .map_err(|error| format!("validate Codex projection shape: {error}"))?;
            let validation = validate_elegy_plugin_v3(&codex);
            if !validation.is_valid() {
                return Err(format!(
                    "Codex projection would be invalid: {}",
                    validation.issues.join("; ")
                ));
            }
            write_json(
                &output.join(".codex-plugin").join("plugin.json"),
                &codex_value,
            )?;
        }
        "holon" => write_json(&output.join("holon-registration.json"), &json!({
            "schemaVersion": "elegy-holon-registration/v1",
            "package": projection["package"],
            "mcp": mcp_config
        }))?,
        "shell" => fs::write(
            output.join("USAGE.md"),
            format!(
                "# {}\n\nRun `elegy-capability-mcp --package .` for the MCP projection.\n\nCapabilities:\n{}\n",
                projection["package"]["name"],
                capabilities
                    .iter()
                    .filter_map(|value| {
                        let id = value.get("id").and_then(Value::as_str)?;
                        let invocation = value.get("invocation")?.as_object()?;
                        let executable = invocation.get("executable").and_then(Value::as_str)?;
                        let command = invocation
                            .get("command")
                            .and_then(Value::as_array)
                            .map(|parts| {
                                parts
                                    .iter()
                                    .filter_map(Value::as_str)
                                    .collect::<Vec<_>>()
                                    .join(" ")
                            })
                            .unwrap_or_default();
                        Some(format!(
                            "- `{id}`\n  - CLI: `{executable} {command}` (JSON on stdin, JSON on stdout)"
                        ))
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            ),
        )
        .map_err(|error| format!("write shell projection: {error}"))?,
        other => return Err(format!("unsupported projection host '{other}'")),
    }
    Ok(format!(
        "Projected {} for {host} into {}.",
        projection["package"]["name"],
        output.display()
    ))
}

fn capability_projection(capability: &ElegyCapabilityV2) -> Result<Value, String> {
    serde_json::to_value(capability).map_err(|error| format!("serialize capability: {error}"))
}

fn apply_provenance(
    root: &Path,
    source_commit: Option<&str>,
    build_workflow: Option<&str>,
    builder: Option<&str>,
) -> Result<(), String> {
    if source_commit.is_none() && build_workflow.is_none() && builder.is_none() {
        return Ok(());
    }
    let mut package = verify_elegy_package_v1(root).map_err(|error| error.to_string())?;
    let mut provenance = package.provenance.unwrap_or(ElegyPackageProvenanceV1 {
        source_commit: None,
        build_workflow: None,
        builder: None,
    });
    if source_commit.is_some() {
        provenance.source_commit = source_commit.map(str::to_string);
    }
    if build_workflow.is_some() {
        provenance.build_workflow = build_workflow.map(str::to_string);
    }
    if builder.is_some() {
        provenance.builder = builder.map(str::to_string);
    }
    package.provenance = Some(provenance);
    write_json(&root.join("elegy-package.json"), &package)
}

fn write_sbom(
    package_root: &Path,
    archive: &Path,
    archive_sha256: &str,
    output: &Path,
) -> Result<(), String> {
    let package = verify_elegy_package_v1(package_root).map_err(|error| error.to_string())?;
    let manifest_bytes = serde_json::to_vec_pretty(&package)
        .map_err(|error| format!("serialize package manifest for SBOM: {error}"))?;
    let mut files = vec![ElegySbomFileV1 {
        path: "elegy-package.json".to_string(),
        role: "package-manifest".to_string(),
        sha256: format!("{:x}", Sha256::digest(&manifest_bytes)),
    }];
    files.extend(
        package
            .files
            .iter()
            .map(|file| {
                Ok(ElegySbomFileV1 {
                    path: file.path.clone(),
                    role: file.role.clone(),
                    sha256: file.sha256.clone().ok_or_else(|| {
                        format!("package file '{}' has no materialized digest", file.path)
                    })?,
                })
            })
            .collect::<Result<Vec<_>, String>>()?,
    );
    files.sort_by(|left, right| left.path.cmp(&right.path));
    let sbom = ElegySbomV1 {
        schema_version: elegy_plugin_sdk::capability_package::ELEGY_SBOM_V1_SCHEMA_VERSION
            .to_string(),
        package: package.name,
        version: package.version,
        publisher: package.publisher.repository,
        archive_sha256: archive_sha256.to_string(),
        files,
        provenance: package.provenance,
    };
    let issues = validate_elegy_sbom_v1(&sbom);
    if !issues.is_empty() {
        return Err(issues.join("; "));
    }
    write_json(output, &sbom)
        .map_err(|error| format!("write SBOM beside {}: {error}", archive.display()))
}

fn package_path(root: &Path, relative: &str) -> Result<PathBuf, String> {
    let normalized = relative
        .strip_prefix("./")
        .ok_or_else(|| format!("path '{relative}' must start with './'"))?;
    if relative.contains('\\')
        || normalized.is_empty()
        || normalized
            .split('/')
            .any(|part| part.is_empty() || part == "..")
    {
        return Err(format!("path '{relative}' is unsafe"));
    }
    Ok(root.join(normalized))
}

fn validate_name(name: &str) -> Result<(), String> {
    if name.is_empty()
        || name.starts_with('-')
        || name.ends_with('-')
        || name.chars().any(|character| {
            !(character.is_ascii_lowercase() || character.is_ascii_digit() || character == '-')
        })
    {
        return Err("name must be lowercase kebab-case".to_string());
    }
    Ok(())
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("create {}: {error}", parent.display()))?;
    }
    let content =
        serde_json::to_vec_pretty(value).map_err(|error| format!("serialize JSON: {error}"))?;
    fs::write(path, content).map_err(|error| format!("write {}: {error}", path.display()))
}
