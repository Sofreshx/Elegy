use clap::{Parser, Subcommand};
use serde_json::Value;

#[derive(Parser)]
#[command(name = "elegy-desktop", about = "Safe desktop automation commands")]
struct Cli {
    /// Output raw JSON instead of human-readable text
    #[arg(long, default_value_t = false)]
    json: bool,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Simulate a mouse click at pixel coordinates
    Click {
        /// X coordinate
        x: i32,
        /// Y coordinate
        y: i32,
        /// Mouse button: left, right, or middle
        #[arg(long, default_value = "left")]
        button: String,
        /// Preview the action without executing it
        #[arg(long, default_value_t = false)]
        dry_run: bool,
    },
    /// Type text by injecting Unicode key events
    Type {
        /// Text to type
        text: String,
        /// Preview the action without executing it
        #[arg(long, default_value_t = false)]
        dry_run: bool,
    },
    /// Send a key combination (e.g. ctrl+s, alt+tab)
    Key {
        /// Key combo string (e.g. "ctrl+s", "alt+tab", "enter")
        combo: String,
        /// Preview the action without executing it
        #[arg(long, default_value_t = false)]
        dry_run: bool,
    },
    /// Focus a window by title pattern or raw HWND
    Focus {
        /// Window title pattern (case-insensitive substring match)
        #[arg(long)]
        title: Option<String>,
        /// Raw window handle (HWND) as decimal integer
        #[arg(long)]
        hwnd: Option<u64>,
        /// Preview the action without executing it
        #[arg(long, default_value_t = false)]
        dry_run: bool,
    },
    /// Move and optionally resize a window
    Move {
        /// Window title pattern (case-insensitive substring match)
        #[arg(long)]
        title: Option<String>,
        /// Raw window handle (HWND) as decimal integer
        #[arg(long)]
        hwnd: Option<u64>,
        /// Target X coordinate
        x: i32,
        /// Target Y coordinate
        y: i32,
        /// New width in pixels
        #[arg(long)]
        width: Option<u32>,
        /// New height in pixels
        #[arg(long)]
        height: Option<u32>,
        /// Preview the action without executing it
        #[arg(long, default_value_t = false)]
        dry_run: bool,
    },
    /// Minimize a window
    Minimize {
        /// Window title pattern (case-insensitive substring match)
        #[arg(long)]
        title: Option<String>,
        /// Raw window handle (HWND) as decimal integer
        #[arg(long)]
        hwnd: Option<u64>,
        /// Preview the action without executing it
        #[arg(long, default_value_t = false)]
        dry_run: bool,
    },
    /// Maximize a window
    Maximize {
        /// Window title pattern (case-insensitive substring match)
        #[arg(long)]
        title: Option<String>,
        /// Raw window handle (HWND) as decimal integer
        #[arg(long)]
        hwnd: Option<u64>,
        /// Preview the action without executing it
        #[arg(long, default_value_t = false)]
        dry_run: bool,
    },
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    let result: Result<Value, String> = match &cli.command {
        Command::Click {
            x,
            y,
            button,
            dry_run,
        } => {
            let r = elegy_desktop::click(*x, *y, button, *dry_run).map_err(|e| e.to_string())?;
            serde_json::to_value(&r).map_err(|e| e.to_string())
        }
        Command::Type { text, dry_run } => {
            let r = elegy_desktop::type_text(text, *dry_run).map_err(|e| e.to_string())?;
            serde_json::to_value(&r).map_err(|e| e.to_string())
        }
        Command::Key { combo, dry_run } => {
            let r = elegy_desktop::send_key(combo, *dry_run).map_err(|e| e.to_string())?;
            serde_json::to_value(&r).map_err(|e| e.to_string())
        }
        Command::Focus {
            title,
            hwnd,
            dry_run,
        } => {
            let r = elegy_desktop::focus_window(title.as_deref(), *hwnd, *dry_run)
                .map_err(|e| e.to_string())?;
            serde_json::to_value(&r).map_err(|e| e.to_string())
        }
        Command::Move {
            title,
            hwnd,
            x,
            y,
            width,
            height,
            dry_run,
        } => {
            let r = elegy_desktop::move_window(
                title.as_deref(),
                *hwnd,
                *x,
                *y,
                *width,
                *height,
                *dry_run,
            )
            .map_err(|e| e.to_string())?;
            serde_json::to_value(&r).map_err(|e| e.to_string())
        }
        Command::Minimize {
            title,
            hwnd,
            dry_run,
        } => {
            let r = elegy_desktop::minimize_window(title.as_deref(), *hwnd, *dry_run)
                .map_err(|e| e.to_string())?;
            serde_json::to_value(&r).map_err(|e| e.to_string())
        }
        Command::Maximize {
            title,
            hwnd,
            dry_run,
        } => {
            let r = elegy_desktop::maximize_window(title.as_deref(), *hwnd, *dry_run)
                .map_err(|e| e.to_string())?;
            serde_json::to_value(&r).map_err(|e| e.to_string())
        }
    };

    match result {
        Ok(value) => {
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&value).unwrap());
            } else {
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
