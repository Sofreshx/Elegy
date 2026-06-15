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
        } => {
            println!(
                "{{ \"status\": \"not_implemented\", \"command\": \"extract\", \"lang\": \"{}\", \"repo\": \"{}\", \"out\": \"{}\", \"use_scip\": {} }}",
                lang, repo, out, use_scip
            );
        }
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
