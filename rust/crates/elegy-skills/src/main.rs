use clap::{Parser, Subcommand, ValueEnum};
use elegy_skills::{
    validate_skill_directory, validate_skill_file, AgentCapabilityProfile,
    RegistryProfileSelection, SkillRegistry, SkillRegistryQuery,
};
use serde::Serialize;
use serde_json::json;
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::OnceLock;

const EXIT_CODE_INVALID_INPUT: u8 = 1;
const EXIT_CODE_RUNTIME_FAILURE: u8 = 2;

static CLI_MACHINE_CONTEXT: OnceLock<MachineContext> = OnceLock::new();

#[derive(Parser, Debug)]
#[command(name = "elegy-skills")]
#[command(about = "Dedicated skill registry CLI for Elegy")]
struct Cli {
    #[arg(long, value_enum, default_value_t = OutputFormat::Text, global = true)]
    format: OutputFormat,
    #[arg(long, global = true)]
    json: bool,
    #[arg(long, global = true)]
    non_interactive: bool,
    #[arg(long, global = true)]
    correlation_id: Option<String>,
    #[arg(long, global = true)]
    profile: Option<PathBuf>,
    #[arg(long, global = true)]
    registry: Vec<PathBuf>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum OutputFormat {
    Text,
    Json,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct MachineEnvelope<T>
where
    T: Serialize,
{
    #[serde(skip_serializing_if = "Option::is_none")]
    correlation_id: Option<String>,
    #[serde(skip_serializing_if = "is_false")]
    non_interactive: bool,
    command: Vec<String>,
    status: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

#[derive(Clone, Debug)]
struct MachineContext {
    format: OutputFormat,
    non_interactive: bool,
    correlation_id: Option<String>,
    command: Vec<String>,
}

#[derive(Clone, Debug)]
enum RegistryLoadError {
    Runtime(String),
    InvalidInput(String),
    InvalidProfile(Box<RegistryProfileSelection>),
}

impl RegistryLoadError {
    fn status(&self) -> &'static str {
        match self {
            RegistryLoadError::Runtime(_) => "error",
            RegistryLoadError::InvalidInput(_) | RegistryLoadError::InvalidProfile(_) => "invalid",
        }
    }

    fn exit_code(&self) -> ExitCode {
        match self {
            RegistryLoadError::Runtime(_) => exit_runtime(),
            RegistryLoadError::InvalidInput(_) | RegistryLoadError::InvalidProfile(_) => {
                exit_invalid()
            }
        }
    }

    fn message(&self) -> String {
        match self {
            RegistryLoadError::Runtime(message) | RegistryLoadError::InvalidInput(message) => {
                message.clone()
            }
            RegistryLoadError::InvalidProfile(selection) => {
                selection_error_message(selection.as_ref())
            }
        }
    }

    fn data(&self) -> Option<serde_json::Value> {
        match self {
            RegistryLoadError::InvalidProfile(selection) => Some(
                serde_json::to_value(selection.as_ref())
                    .unwrap_or_else(|_| json!({ "issues": selection.issues.clone() })),
            ),
            RegistryLoadError::Runtime(_) | RegistryLoadError::InvalidInput(_) => None,
        }
    }
}

#[derive(Subcommand, Debug)]
enum Command {
    List {
        #[arg(long)]
        category: Option<String>,
        #[arg(long)]
        lifecycle: Option<String>,
        #[arg(long)]
        detail: bool,
    },
    Search {
        #[arg(long)]
        query: String,
        #[arg(long)]
        detail: bool,
    },
    Resolve {
        #[arg(long)]
        query: String,
        #[arg(long)]
        detail: bool,
    },
    Get {
        #[arg(long)]
        skill_id: String,
    },
    Capability {
        #[arg(long)]
        capability_id: String,
    },
    Validate {
        #[arg(long)]
        file: Option<PathBuf>,
        #[arg(long)]
        dir: Option<PathBuf>,
    },
}

fn main() -> ExitCode {
    match run() {
        Ok(code) => code,
        Err(error) => {
            if let Some(context) = CLI_MACHINE_CONTEXT.get() {
                if context.format == OutputFormat::Json
                    && print_json(&MachineEnvelope::<serde_json::Value> {
                        correlation_id: context.correlation_id.clone(),
                        non_interactive: context.non_interactive,
                        command: context.command.clone(),
                        status: "error",
                        data: None,
                        error: Some(error.to_string()),
                    })
                    .is_ok()
                {
                    return exit_runtime();
                }
            }
            eprintln!("unexpected CLI failure: {error}");
            exit_runtime()
        }
    }
}

fn run() -> Result<ExitCode, serde_json::Error> {
    let cli = Cli::parse();
    let context = MachineContext {
        format: resolve_output_format(cli.json, cli.format),
        non_interactive: cli.non_interactive,
        correlation_id: cli.correlation_id,
        command: vec![command_name(&cli.command).to_string()],
    };
    let _ = CLI_MACHINE_CONTEXT.set(context.clone());

    match cli.command {
        Command::List {
            category,
            lifecycle,
            detail,
        } => execute_list(
            category,
            lifecycle,
            detail,
            cli.profile,
            cli.registry,
            &context,
        ),
        Command::Search { query, detail } => {
            execute_search(query, detail, cli.profile, cli.registry, &context)
        }
        Command::Resolve { query, detail } => {
            execute_resolve(query, detail, cli.profile, cli.registry, &context)
        }
        Command::Get { skill_id } => execute_get(skill_id, cli.profile, cli.registry, &context),
        Command::Capability { capability_id } => {
            execute_capability(capability_id, cli.profile, cli.registry, &context)
        }
        Command::Validate { file, dir } => execute_validate(file, dir, &context),
    }
}

fn execute_list(
    category: Option<String>,
    lifecycle: Option<String>,
    detail: bool,
    profile: Option<PathBuf>,
    registry: Vec<PathBuf>,
    context: &MachineContext,
) -> Result<ExitCode, serde_json::Error> {
    let registry = match load_registry(&registry, profile.as_deref()) {
        Ok(registry) => registry,
        Err(error) => return emit_registry_load_error(context, error),
    };
    let data = registry.list(&SkillRegistryQuery {
        category,
        lifecycle,
        include_detail: detail,
    });

    match context.format {
        OutputFormat::Text => {
            if data.is_empty() {
                println!("No skills found matching the given filters.");
            } else {
                println!("{:<16} {:<32} {:<6} STATE", "ID", "NAME", "CAPS");
                println!("{}", "-".repeat(70));
                for skill in &data {
                    println!(
                        "{:<16} {:<32} {:<6} {}",
                        skill.summary.id,
                        skill.summary.name,
                        skill.summary.capabilities_count,
                        skill.summary.lifecycle_state
                    );
                }
            }
        }
        OutputFormat::Json => print_json(&build_success_envelope(
            context,
            ["list"],
            json!({
                "skills": data,
                "disclosureLevel": if detail { "detail" } else { "index" }
            }),
        ))?,
    }

    Ok(ExitCode::SUCCESS)
}

fn execute_search(
    query: String,
    detail: bool,
    profile: Option<PathBuf>,
    registry_paths: Vec<PathBuf>,
    context: &MachineContext,
) -> Result<ExitCode, serde_json::Error> {
    let registry = match load_registry(&registry_paths, profile.as_deref()) {
        Ok(registry) => registry,
        Err(error) => return emit_registry_load_error(context, error),
    };
    let data = registry.search(&query, detail);

    match context.format {
        OutputFormat::Text => {
            if data.is_empty() {
                println!("No skills matched query: \"{query}\"");
            } else {
                println!("Skills matching \"{query}\":");
                println!();
                println!("{:<16} {:<32} {:<6} MATCHED", "ID", "NAME", "SCORE");
                println!("{}", "-".repeat(72));
                for skill in &data {
                    println!(
                        "{:<16} {:<32} {:<6.2} {}",
                        skill.summary.id,
                        skill.summary.name,
                        skill
                            .match_result
                            .as_ref()
                            .map(|match_result| match_result.score)
                            .unwrap_or(0.0),
                        skill
                            .match_result
                            .as_ref()
                            .map(|match_result| match_result.match_reasons.join(", "))
                            .unwrap_or_default()
                    );
                }
            }
        }
        OutputFormat::Json => print_json(&build_success_envelope(
            context,
            ["search"],
            json!({
                "query": query,
                "results": data,
                "disclosureLevel": if detail { "detail" } else { "index" }
            }),
        ))?,
    }

    Ok(ExitCode::SUCCESS)
}

fn execute_resolve(
    query: String,
    detail: bool,
    profile: Option<PathBuf>,
    registry_paths: Vec<PathBuf>,
    context: &MachineContext,
) -> Result<ExitCode, serde_json::Error> {
    let registry = match load_registry(&registry_paths, profile.as_deref()) {
        Ok(registry) => registry,
        Err(error) => return emit_registry_load_error(context, error),
    };
    let result = registry.resolve(&query, detail);

    match context.format {
        OutputFormat::Text => {
            if let Some(skill) = &result.top_skill {
                println!("Top skill: {} ({})", skill.summary.name, skill.summary.id);
                if let Some(capability) = &result.top_capability {
                    println!(
                        "Top capability: {} ({})",
                        capability.capability_name, capability.capability_id
                    );
                }
            } else {
                println!("No matching skills found.");
            }
        }
        OutputFormat::Json => print_json(&build_success_envelope(context, ["resolve"], result))?,
    }

    Ok(ExitCode::SUCCESS)
}

fn execute_get(
    skill_id: String,
    profile: Option<PathBuf>,
    registry_paths: Vec<PathBuf>,
    context: &MachineContext,
) -> Result<ExitCode, serde_json::Error> {
    let registry = match load_registry(&registry_paths, profile.as_deref()) {
        Ok(registry) => registry,
        Err(error) => return emit_registry_load_error(context, error),
    };
    let Some(skill) = registry.skill_definition(&skill_id) else {
        return emit_error(
            context,
            format!("skill '{skill_id}' not found"),
            exit_invalid(),
        );
    };

    match context.format {
        OutputFormat::Text => {
            println!("Skill: {}", skill.identity.name);
            println!("Capabilities: {}", skill.capabilities.len());
        }
        OutputFormat::Json => print_json(&build_success_envelope(context, ["get"], skill))?,
    }

    Ok(ExitCode::SUCCESS)
}

fn execute_capability(
    capability_id: String,
    profile: Option<PathBuf>,
    registry_paths: Vec<PathBuf>,
    context: &MachineContext,
) -> Result<ExitCode, serde_json::Error> {
    let registry = match load_registry(&registry_paths, profile.as_deref()) {
        Ok(registry) => registry,
        Err(error) => return emit_registry_load_error(context, error),
    };
    let Some(capability) = registry.capability(&capability_id) else {
        return emit_error(
            context,
            format!("capability '{capability_id}' not found"),
            exit_invalid(),
        );
    };

    match context.format {
        OutputFormat::Text => {
            println!("Capability: {}", capability.capability_name);
            println!("Skill: {}", capability.skill_name);
            println!("Side effects: {}", capability.has_side_effects);
        }
        OutputFormat::Json => {
            print_json(&build_success_envelope(context, ["capability"], capability))?
        }
    }

    Ok(ExitCode::SUCCESS)
}

fn execute_validate(
    file: Option<PathBuf>,
    dir: Option<PathBuf>,
    context: &MachineContext,
) -> Result<ExitCode, serde_json::Error> {
    let report = match (file, dir) {
        (Some(file), None) => validate_skill_file(&file)
            .map_err(|error| <serde_json::Error as serde::de::Error>::custom(error.to_string()))?,
        (None, Some(dir)) => validate_skill_directory(&dir)
            .map_err(|error| <serde_json::Error as serde::de::Error>::custom(error.to_string()))?,
        _ => {
            return emit_error(
                context,
                "pass exactly one of --file or --dir".to_string(),
                exit_invalid(),
            )
        }
    };

    let status = if report.valid { "ok" } else { "error" };
    let code = if report.valid {
        ExitCode::SUCCESS
    } else {
        exit_invalid()
    };

    match context.format {
        OutputFormat::Text => {
            if report.valid {
                println!("skill registry input is valid");
            } else {
                println!("skill registry input is invalid");
                for issue in &report.issues {
                    println!("- {}", issue.message);
                }
            }
        }
        OutputFormat::Json => print_json(&MachineEnvelope {
            correlation_id: context.correlation_id.clone(),
            non_interactive: context.non_interactive,
            command: vec!["validate".to_string()],
            status,
            data: Some(report),
            error: None,
        })?,
    }

    Ok(code)
}

fn load_registry(
    registry_paths: &[PathBuf],
    profile_path: Option<&std::path::Path>,
) -> Result<SkillRegistryWithProfile, RegistryLoadError> {
    let registry = if registry_paths.is_empty() {
        SkillRegistry::builtin().map_err(|error| RegistryLoadError::Runtime(error.to_string()))?
    } else {
        SkillRegistry::from_sources(registry_paths)
            .map_err(|error| RegistryLoadError::InvalidInput(error.to_string()))?
    };
    let profile = match profile_path {
        Some(path) => {
            let contents = fs::read_to_string(path).map_err(|error| {
                RegistryLoadError::InvalidInput(format!(
                    "failed to read agent capability profile {}: {error}",
                    path.display()
                ))
            })?;
            Some(
                serde_json::from_str::<AgentCapabilityProfile>(&contents).map_err(|error| {
                    RegistryLoadError::InvalidInput(format!(
                        "invalid agent capability profile JSON in {}: {error}",
                        path.display()
                    ))
                })?,
            )
        }
        None => None,
    };
    let selection = registry.profile_selection(profile.as_ref());
    if selection.has_errors() {
        return Err(RegistryLoadError::InvalidProfile(Box::new(selection)));
    }
    Ok(SkillRegistryWithProfile {
        registry,
        selection,
    })
}

struct SkillRegistryWithProfile {
    registry: SkillRegistry,
    selection: RegistryProfileSelection,
}

impl SkillRegistryWithProfile {
    fn filtered_skills(&self) -> Vec<elegy_skills::RegistrySkillEntry> {
        if self.selection.profile_provided {
            self.registry.filtered_by_profile(&self.selection)
        } else {
            self.registry.list(&SkillRegistryQuery {
                include_detail: true,
                ..SkillRegistryQuery::default()
            })
        }
    }

    fn list(&self, query: &SkillRegistryQuery) -> Vec<elegy_skills::RegistrySkillEntry> {
        if self.selection.profile_provided {
            self.filtered_skills()
                .into_iter()
                .filter(|skill| {
                    query.category.as_ref().is_none_or(|category| {
                        skill.summary.category.eq_ignore_ascii_case(category)
                    }) && query.lifecycle.as_ref().is_none_or(|lifecycle| {
                        skill
                            .summary
                            .lifecycle_state
                            .eq_ignore_ascii_case(lifecycle)
                    })
                })
                .map(|mut skill| {
                    if !query.include_detail {
                        skill.capabilities = None;
                    }
                    skill
                })
                .collect()
        } else {
            self.registry.list(query)
        }
    }

    fn search(&self, query: &str, include_detail: bool) -> Vec<elegy_skills::RegistrySkillEntry> {
        if self.selection.profile_provided {
            let filtered = self.filtered_skills();
            self.registry
                .search_filtered(&filtered, query, include_detail)
        } else {
            self.registry.search(query, include_detail)
        }
    }

    fn resolve(&self, query: &str, include_detail: bool) -> elegy_skills::RegistryResolveResult {
        if self.selection.profile_provided {
            let filtered = self.filtered_skills();
            self.registry
                .resolve_filtered(&filtered, query, include_detail)
        } else {
            self.registry.resolve(query, include_detail)
        }
    }

    fn skill_definition(&self, skill_id: &str) -> Option<elegy_skills::SkillDefinitionV2> {
        if self.selection.profile_provided {
            let filtered = self.filtered_skills();
            let skill = filtered.into_iter().find(|skill| {
                skill.summary.id == skill_id
                    || skill
                        .summary
                        .aliases
                        .iter()
                        .any(|alias| alias.eq_ignore_ascii_case(skill_id))
            })?;
            let mut definition = self.registry.skill_definition(&skill.summary.id)?;
            definition.capabilities.retain(|capability| {
                self.selection
                    .selected_capability_ids
                    .contains(&capability.id)
            });
            return Some(definition);
        }
        self.registry.skill_definition(skill_id)
    }

    fn capability(&self, capability_id: &str) -> Option<elegy_skills::RegistryCapabilityCard> {
        if self.selection.profile_provided
            && !self
                .selection
                .selected_capability_ids
                .contains(capability_id)
        {
            return None;
        }
        self.registry.capability(capability_id)
    }
}

fn emit_error(
    context: &MachineContext,
    message: String,
    code: ExitCode,
) -> Result<ExitCode, serde_json::Error> {
    match context.format {
        OutputFormat::Text => eprintln!("{message}"),
        OutputFormat::Json => print_json(&MachineEnvelope::<serde_json::Value> {
            correlation_id: context.correlation_id.clone(),
            non_interactive: context.non_interactive,
            command: context.command.clone(),
            status: "error",
            data: None,
            error: Some(message),
        })?,
    }

    Ok(code)
}

fn emit_registry_load_error(
    context: &MachineContext,
    error: RegistryLoadError,
) -> Result<ExitCode, serde_json::Error> {
    match context.format {
        OutputFormat::Text => match &error {
            RegistryLoadError::InvalidProfile(selection) => {
                eprintln!("{}", error.message());
                for issue in selection
                    .as_ref()
                    .issues
                    .iter()
                    .filter(|issue| issue.code.starts_with("REGISTRY-PROFILE-E"))
                {
                    eprintln!("- {}", issue.message);
                }
            }
            RegistryLoadError::Runtime(_) | RegistryLoadError::InvalidInput(_) => {
                eprintln!("{}", error.message())
            }
        },
        OutputFormat::Json => print_json(&MachineEnvelope {
            correlation_id: context.correlation_id.clone(),
            non_interactive: context.non_interactive,
            command: context.command.clone(),
            status: error.status(),
            data: error.data(),
            error: Some(error.message()),
        })?,
    }

    Ok(error.exit_code())
}

fn resolve_output_format(json: bool, format: OutputFormat) -> OutputFormat {
    if json {
        OutputFormat::Json
    } else {
        format
    }
}

fn command_name(command: &Command) -> &'static str {
    match command {
        Command::List { .. } => "list",
        Command::Search { .. } => "search",
        Command::Resolve { .. } => "resolve",
        Command::Get { .. } => "get",
        Command::Capability { .. } => "capability",
        Command::Validate { .. } => "validate",
    }
}

fn selection_error_message(selection: &RegistryProfileSelection) -> String {
    selection
        .issues
        .iter()
        .filter(|issue| issue.code.starts_with("REGISTRY-PROFILE-E"))
        .map(|issue| issue.message.clone())
        .collect::<Vec<_>>()
        .join("; ")
}

fn build_success_envelope<T, S>(
    context: &MachineContext,
    command: impl IntoIterator<Item = S>,
    data: T,
) -> MachineEnvelope<T>
where
    T: Serialize,
    S: Into<String>,
{
    MachineEnvelope {
        correlation_id: context.correlation_id.clone(),
        non_interactive: context.non_interactive,
        command: command.into_iter().map(Into::into).collect(),
        status: "ok",
        data: Some(data),
        error: None,
    }
}

fn is_false(value: &bool) -> bool {
    !*value
}

fn exit_invalid() -> ExitCode {
    ExitCode::from(EXIT_CODE_INVALID_INPUT)
}

fn exit_runtime() -> ExitCode {
    ExitCode::from(EXIT_CODE_RUNTIME_FAILURE)
}

fn print_json<T>(value: &T) -> Result<(), serde_json::Error>
where
    T: Serialize,
{
    let text = serde_json::to_string_pretty(value)?;
    println!("{text}");
    Ok(())
}
