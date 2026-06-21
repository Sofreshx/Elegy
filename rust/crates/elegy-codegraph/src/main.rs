//! Elegy Codegraph CLI — portable codebase graph extraction and query.
//!
//! ## Usage
//!
//! ```text
//! elegy-codegraph extract --lang ts|rust --repo <path> --out <graph.bin> [--use-scip]
//! elegy-codegraph query   --graph <graph.bin> symbol --name <q> [--lang ts|rust]
//! elegy-codegraph query   --graph <graph.bin> neighbors --id <id> --direction in|out
//! elegy-codegraph query   --graph <graph.bin> impact --path <file>
//! elegy-codegraph query   --graph <graph.bin> summary
//! ```
//!
//! ## Deferred commands (see docs/specs/codegraph-diff-slice.md)
//! - `diff`  — structural diff between two graph snapshots
//! - `review` — rule-pack-based code review
//! - `validate` — graph freshness and schema compliance

use clap::{Parser, Subcommand};
use elegy_codegraph::query::QueryEngine;
use elegy_codegraph::store::Store;

#[derive(Parser)]
#[command(name = "elegy-codegraph")]
#[command(version = env!("CARGO_PKG_VERSION"))]
#[command(about = "Portable codebase graph extraction and query")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Build a graph index from source code
    Extract {
        /// Source language: ts or rust
        #[arg(long)]
        lang: String,
        /// Path to the repository root
        #[arg(long)]
        repo: String,
        /// Output graph database path
        #[arg(long)]
        out: String,
        /// Use SCIP from rust-analyzer for Rust semantic edges (Rust only)
        #[arg(long)]
        use_scip: bool,
    },
    /// Query an existing graph index
    Query {
        /// Path to the graph database
        #[arg(long)]
        graph: String,
        #[command(subcommand)]
        sub: QueryCommand,
    },
}

#[derive(Subcommand)]
enum QueryCommand {
    /// Look up a symbol by name
    Symbol {
        #[arg(long)]
        name: String,
        #[arg(long)]
        lang: Option<String>,
    },
    /// Get neighbors of an entity
    Neighbors {
        #[arg(long)]
        id: String,
        #[arg(long)]
        direction: String,
    },
    /// Analyze impact of changes to a file
    Impact {
        #[arg(long)]
        path: String,
    },
    /// Get a structural summary of the repository
    Summary,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Extract {
            lang,
            repo,
            out,
            use_scip,
        } => match run_extract(&lang, &repo, &out, use_scip) {
            Ok(json_output) => println!("{}", json_output),
            Err(e) => {
                let error = serde_json::json!({
                    "status": "error",
                    "message": e.to_string(),
                });
                println!(
                    "{}",
                    serde_json::to_string(&error)
                        .unwrap_or_else(|_| r#"{"status":"error","message":"serialization failed"}"#.to_string())
                );
                std::process::exit(1);
            }
        },
        Command::Query { graph, sub } => {
            let store = Store::open(&graph)?;
            let engine = QueryEngine::new(store);

            let output = match sub {
                QueryCommand::Symbol { name, lang } => {
                    engine.symbol(&name, lang.as_deref())?
                }
                QueryCommand::Neighbors { id, direction } => {
                    engine.neighbors(&id, &direction)?
                }
                QueryCommand::Impact { path } => engine.impact(&path)?,
                QueryCommand::Summary => engine.summary()?,
            };

            println!("{}", output);
        }
    }

    Ok(())
}

fn run_extract(lang: &str, repo: &str, out: &str, use_scip: bool) -> anyhow::Result<String> {
    // Validate --lang
    match lang {
        "rust" | "ts" => {}
        _ => anyhow::bail!("Unsupported language: '{}'. Supported: ts, rust", lang),
    }

    // Check parent directory exists
    if let Some(parent) = std::path::Path::new(out).parent() {
        if !parent.as_os_str().is_empty() && !parent.exists() {
            anyhow::bail!("Output directory does not exist: {}", parent.display());
        }
    }

    // Dispatch extractor
    let mut graph = match lang {
        "rust" => elegy_codegraph::extractor::rust_lang::extract(repo)?,
        "ts" => elegy_codegraph::extractor::ts::extract(repo)?,
        _ => unreachable!(),
    };

    // Handle --use-scip
    let mut warning: Option<String> = None;
    if use_scip {
        match lang {
            "rust" => {
                elegy_codegraph::extractor::rust_scip::augment(&mut graph, repo)?;
                if let Some(ref w) = graph.extractor.warning {
                    warning = Some(w.clone());
                }
            }
            "ts" => {
                warning = Some("--use-scip is not supported for TypeScript; ignoring".to_string());
            }
            _ => unreachable!(),
        }
    }

    // Write to redb store in a temp file, then atomically rename.
    // Preserves the existing --out file until the write succeeds.
    let temp_path = format!("{}.tmp", out);
    {
        let mut store = Store::open(&temp_path)?;
        for entity in &graph.entities {
            store.insert_entity(entity)?;
        }
        for edge in &graph.edges {
            store.insert_edge(edge)?;
        }
        store.compact()?;
    } // Store dropped here, closing the redb file.

    // Atomic swap: delete old file (if any), rename temp → target.
    if std::path::Path::new(out).exists() {
        std::fs::remove_file(out)?;
    }
    std::fs::rename(&temp_path, out)?;

    // Build JSON result
    let mut result = serde_json::json!({
        "status": "ok",
        "lang": lang,
        "repo": repo,
        "out": out,
        "entityCount": graph.entities.len(),
        "edgeCount": graph.edges.len(),
        "extractor": graph.extractor.name,
    });

    if let Some(w) = warning {
        result["warning"] = serde_json::json!(w);
    }

    Ok(serde_json::to_string(&result)
        .unwrap_or_else(|_| r#"{"status":"error","message":"serialization failed"}"#.to_string()))
}
