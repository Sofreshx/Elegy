use clap::{Parser, Subcommand};
use serde_json::Value;
use std::path::PathBuf;
use std::time::Duration;

#[derive(Parser)]
#[command(name = "elegy-observe", about = "Desktop and OS observation commands")]
struct Cli {
    /// Output raw JSON instead of human-readable text
    #[arg(long, default_value_t = false)]
    json: bool,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Snapshot running processes, optionally filtered by name
    Processes {
        /// Optional process name filter (case-insensitive substring match)
        filter: Option<String>,
    },
    /// Get the foreground (active) window info
    Window,
    /// List visible top-level windows, optionally filtered by title
    Windows {
        /// Optional window title filter (case-insensitive substring match)
        filter: Option<String>,
    },
    /// Capture the screen to a PNG file or base64
    Screen {
        /// Monitor index to capture (default: 0)
        #[arg(long, default_value_t = 0)]
        monitor: u32,
        /// Output file path for PNG (omit for base64 output)
        #[arg(long)]
        output: Option<PathBuf>,
    },
    /// Read the system clipboard contents
    Clipboard,
    /// Observe a filesystem path for changes over a bounded time window
    Filesystem {
        /// Path to watch for changes
        #[arg(long)]
        path: PathBuf,
        /// Observation timeout in seconds
        #[arg(long, default_value_t = 5)]
        timeout: u64,
    },
    /// Get system information snapshot
    System,
    /// Record a bounded foreground-window observation session
    Record {
        /// Duration of the recording session in seconds
        #[arg(long, default_value_t = 30)]
        duration: u64,
        /// Poll interval in milliseconds
        #[arg(long, default_value_t = 500)]
        poll_interval: u64,
    },
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    let result: Result<Value, String> = match &cli.command {
        Command::Processes { filter } => {
            let snap = elegy_observe::snapshot_processes(filter.as_deref());
            serde_json::to_value(&snap).map_err(|e| e.to_string())
        }
        Command::Window => {
            let info = elegy_observe::foreground_window().map_err(|e| e.to_string())?;
            serde_json::to_value(&info).map_err(|e| e.to_string())
        }
        Command::Windows { filter } => {
            let windows =
                elegy_observe::list_windows(filter.as_deref()).map_err(|e| e.to_string())?;
            serde_json::to_value(&windows).map_err(|e| e.to_string())
        }
        Command::Screen { monitor, output } => {
            let result = elegy_observe::capture_screen(Some(*monitor), output.as_deref())
                .map_err(|e| e.to_string())?;
            serde_json::to_value(&result).map_err(|e| e.to_string())
        }
        Command::Clipboard => {
            let contents = elegy_observe::read_clipboard().map_err(|e| e.to_string())?;
            serde_json::to_value(&contents).map_err(|e| e.to_string())
        }
        Command::Filesystem { path, timeout } => {
            let result = elegy_observe::observe_filesystem(path, Duration::from_secs(*timeout))
                .map_err(|e| e.to_string())?;
            serde_json::to_value(&result).map_err(|e| e.to_string())
        }
        Command::System => {
            let info = elegy_observe::system_info();
            serde_json::to_value(&info).map_err(|e| e.to_string())
        }
        Command::Record {
            duration,
            poll_interval,
        } => {
            let request = elegy_observe::ObservationRecordRequest::new(
                Duration::from_secs(*duration),
                Duration::from_millis(*poll_interval),
            )
            .map_err(|e| e.to_string())?;
            let session =
                elegy_observe::record_observation_session(&request).map_err(|e| e.to_string())?;
            serde_json::to_value(&session).map_err(|e| e.to_string())
        }
    };

    match result {
        Ok(value) => {
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&value).unwrap());
            } else {
                // Human-readable: pretty-print the JSON
                println!("{}", serde_json::to_string_pretty(&value).unwrap());
            }
        }
        Err(msg) => {
            eprintln!("Error: {}", msg);
            std::process::exit(1);
        }
    }
    Ok(())
}
