//! Neuron — Universal Persistent Memory Layer for AI Coding Agents (v5)
//! CLI entrypoint. All commands are dispatched from here.

mod conversation;
mod git;
mod loop_guard;
mod manifest;
mod parser;
mod project_manager;
mod search;
mod session;
mod sync;
mod utils;
mod watcher;

use anyhow::Result;
use clap::{Parser, Subcommand};
use colored::Colorize;
use tracing_subscriber::{fmt, EnvFilter};

// ─── CLI Definition ───────────────────────────────────────────────────────────

#[derive(Parser)]
#[command(
    name    = "neuron",
    version = "0.5.0",
    author  = "AI Neuron Project",
    about   = "Universal Persistent Memory Layer for AI Coding Agents",
    long_about = r#"
Neuron maintains complete, portable project memory (code, conversations,
decisions, architecture) that survives folder changes, restarts, logouts,
account switches, and directory switches.

Multi-project support: Neuron tracks ALL your projects globally and lets you
switch context instantly without losing memory.
"#
)]
struct Cli {
    /// Verbosity level. Use -v for debug, -vv for trace.
    #[arg(short, long, action = clap::ArgAction::Count, global = true)]
    verbose: u8,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize a new Neuron project in the current directory
    Init {
        /// Project name (defaults to directory name)
        #[arg(short, long)]
        name: Option<String>,

        /// Primary language (rust, python, typescript, javascript, go)
        #[arg(short, long, default_value = "rust")]
        language: String,
    },

    /// Start the real-time file and git watcher daemon
    Watch {
        /// Watch directory (defaults to current dir)
        #[arg(short, long)]
        path: Option<String>,
    },

    /// Start the real-time file and git watcher daemon (alias for watch)
    Start {
        /// Watch directory (defaults to current dir)
        #[arg(short, long)]
        path: Option<String>,
    },

    /// Generate rich, ready-to-paste context for AI agents
    Context,

    /// Auto-detect nearest .neuron/ folder (upward search) and restore full context
    Restore {
        /// Starting path for upward search (defaults to current dir)
        #[arg(short, long)]
        from: Option<String>,
    },

    /// Switch to another known project by name or path
    Switch {
        /// Project name or absolute path
        target: String,
    },

    /// List all known projects from the global index
    List {
        /// Show full paths
        #[arg(short, long)]
        long: bool,
    },

    /// Full-text search across project memory
    Search {
        /// Search query
        query: String,

        /// Search across ALL known projects (not just current)
        #[arg(short, long)]
        global: bool,

        /// Maximum results to show
        #[arg(short, long, default_value = "20")]
        limit: usize,
    },

    /// Force-save current session to conversations/ snapshot
    Snapshot {
        /// Optional note to attach to the snapshot
        #[arg(short, long)]
        note: Option<String>,
    },

    /// Show current project status, loop guard state, and last session
    Status,

    /// Manually trigger a backup of .neuron/
    Backup,

    /// Export .neuron/ as a portable archive (.tar.gz)
    Export {
        /// Output file path
        #[arg(short, long)]
        output: Option<String>,
    },
}

// ─── Main ─────────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Set up tracing based on verbosity
    let filter = match cli.verbose {
        0 => "neuron=info,warn",
        1 => "neuron=debug",
        _ => "neuron=trace,debug",
    };
    fmt()
        .with_env_filter(EnvFilter::new(filter))
        .with_target(false)
        .compact()
        .init();

    print_banner();

    match cli.command {
        Commands::Init { name, language } => {
            let cwd = std::env::current_dir()?;
            let project_name = name.unwrap_or_else(|| {
                cwd.file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| "unnamed".to_string())
            });
            project_manager::init_project(&cwd, &project_name, &language).await?;
        }

        Commands::Watch { path } | Commands::Start { path } => {
            let neuron_root = match path {
                Some(p) => {
                    let p_buf = std::path::PathBuf::from(p);
                    utils::find_neuron_root(&p_buf).ok_or_else(|| {
                        anyhow::anyhow!("No .neuron/ folder found upward from {}", p_buf.display())
                    })?
                }
                None => project_manager::discover_project_root().await?
            };
            println!(
                "{} Watching {} ...",
                "▶".green().bold(),
                neuron_root.display().to_string().cyan()
            );
            watcher::start_watcher(&neuron_root).await?;
        }

        Commands::Context => {
            let neuron_root = project_manager::discover_project_root().await?;
            session::print_agent_context(&neuron_root).await?;
        }

        Commands::Restore { from } => {
            let start = match from {
                Some(p) => std::path::PathBuf::from(p),
                None    => std::env::current_dir()?,
            };
            project_manager::restore_project(&start).await?;
        }

        Commands::Switch { target } => {
            project_manager::switch_project(&target).await?;
        }

        Commands::List { long } => {
            project_manager::list_projects(long).await?;
        }

        Commands::Search { query, global, limit } => {
            let neuron_root = project_manager::discover_project_root().await?;
            search::search_memory(&neuron_root, &query, global, limit).await?;
        }

        Commands::Snapshot { note } => {
            let neuron_root = project_manager::discover_project_root().await?;
            conversation::save_snapshot(&neuron_root, note.as_deref()).await?;
        }

        Commands::Status => {
            match project_manager::discover_project_root().await {
                Ok(root) => session::print_status(&root).await?,
                Err(_) => {
                    println!(
                        "{} No Neuron project detected in current directory or parents.",
                        "⚠".yellow().bold()
                    );
                    println!(
                        "  Run {} to initialize, or {} to restore.",
                        "neuron init".cyan(),
                        "neuron restore".cyan()
                    );
                }
            }
        }

        Commands::Backup => {
            let neuron_root = project_manager::discover_project_root().await?;
            utils::backup_neuron_dir(&neuron_root).await?;
            println!("{} Backup complete.", "✓".green().bold());
        }

        Commands::Export { output } => {
            let neuron_root = project_manager::discover_project_root().await?;
            sync::export_archive(&neuron_root, output.as_deref()).await?;
        }
    }

    Ok(())
}

// ─── Banner ───────────────────────────────────────────────────────────────────

fn print_banner() {
    println!(
        "{}",
        r#"
  ███╗   ██╗███████╗██╗   ██╗██████╗  ██████╗ ███╗   ██╗
  ████╗  ██║██╔════╝██║   ██║██╔══██╗██╔═══██╗████╗  ██║
  ██╔██╗ ██║█████╗  ██║   ██║██████╔╝██║   ██║██╔██╗ ██║
  ██║╚██╗██║██╔══╝  ██║   ██║██╔══██╗██║   ██║██║╚██╗██║
  ██║ ╚████║███████╗╚██████╔╝██║  ██║╚██████╔╝██║ ╚████║
  ╚═╝  ╚═══╝╚══════╝ ╚═════╝ ╚═╝  ╚═╝ ╚═════╝ ╚═╝  ╚═══╝
"#
        .bright_cyan()
        .bold()
    );
    println!(
        "  {} {}  {}\n",
        "Universal Persistent Memory Layer".white().bold(),
        "v5".bright_yellow().bold(),
        "for AI Coding Agents".dimmed()
    );
}
