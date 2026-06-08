//! Neuron — Universal Persistent Memory Layer for AI Coding Agents (v11)
//! CLI entrypoint. All commands are dispatched from here.

mod analyzer;
mod bridge;
mod config;
mod conversation;
mod dependency;
mod git;
mod graph;
mod intent;
mod loop_guard;
mod manifest;
mod parser;
mod project_manager;
mod sanitize;
mod search;
mod session;
mod stream;
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

        /// Spin up localized integration bridge server
        #[arg(short, long)]
        bridge: bool,
    },

    /// Start the real-time file and git watcher daemon (alias for watch)
    Start {
        /// Watch directory (defaults to current dir)
        #[arg(short, long)]
        path: Option<String>,

        /// Spin up localized integration bridge server
        #[arg(short, long)]
        bridge: bool,
    },

    /// Generate rich, ready-to-paste context for AI agents
    Context {
        /// Export context to a file (or use '-' for raw stdout)
        #[arg(short, long)]
        export: Option<String>,

        /// Include an additional project alias to merge into the context block
        #[arg(long, value_name = "ALIAS")]
        include: Vec<String>,
    },

    /// Ingest an external project directory into the global workspace registry
    PowerUp {
        /// Target directory to ingest and index
        path: String,

        /// Alias name for this workspace in the global registry
        #[arg(short, long)]
        alias: Option<String>,
    },

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
        query: Option<String>,

        /// Search across ALL known projects (not just current)
        #[arg(short, long)]
        global: bool,

        /// Maximum results to show
        #[arg(short, long, default_value = "20")]
        limit: usize,

        /// Enter persistent interactive query shell
        #[arg(short, long)]
        interactive: bool,
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

    /// Run a comprehensive environment and database health audit
    Diagnose,

    /// Register a directional dependency arc between two workspaces
    #[command(name = "link-deps")]
    LinkDeps {
        /// Alias of the upstream / library workspace
        #[arg(short, long)]
        parent: String,

        /// Alias of the consumer / downstream workspace
        #[arg(short, long)]
        child: String,

        /// Remove the arc instead of adding it
        #[arg(long)]
        unlink: bool,

        /// List all arcs for the given alias instead
        #[arg(short, long)]
        list: bool,
    },

    /// Scan parent workspace for structural signature mutations and print impact matrix
    Analyze {
        /// Alias of the parent workspace to scan
        #[arg(short, long)]
        parent: String,
    },

    /// Manage the active developer intent session
    Session {
        /// Start background intent tracking and focus score polling loop
        #[arg(short, long)]
        track: bool,
    },

    /// Telemetry inflow: log a compilation or command error to flag the current context stream as failed
    #[command(name = "log-error")]
    LogError {
        /// Command that failed (e.g. "cargo build")
        #[arg(short, long)]
        cmd: String,

        /// Error stderr or compiler failure output
        #[arg(short, long)]
        err: String,
    },

    /// Visualise the cross-workspace dependency graph and trace signature mutations
    Graph {
        /// Recursively trace mutation cascades for a specific symbol
        #[arg(short, long)]
        trace: Option<String>,
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
            utils::check_path_registration();
        }

        Commands::Watch { path, bridge } | Commands::Start { path, bridge } => {
            let neuron_root = match path {
                Some(p) => {
                    let p_buf = std::path::PathBuf::from(p);
                    utils::find_neuron_root(&p_buf).ok_or_else(|| {
                        anyhow::anyhow!("No .neuron/ folder found upward from {}", p_buf.display())
                    })?
                }
                None => project_manager::discover_project_root().await?
            };
            if bridge {
                crate::bridge::start_bridge(&neuron_root).await?;
            }
            println!(
                "{} Watching {} ...",
                "▶".green().bold(),
                neuron_root.display().to_string().cyan()
            );
            watcher::start_watcher(&neuron_root).await?;
        }

        Commands::Context { export, include } => {
            let neuron_root = project_manager::discover_project_root().await?;
            session::print_agent_context(&neuron_root, export.as_deref(), &include).await?;
        }

        Commands::PowerUp { path, alias } => {
            let target = std::path::PathBuf::from(&path);
            let target = if target.is_absolute() {
                target
            } else {
                std::env::current_dir()?.join(target)
            };
            project_manager::power_up(&target, alias.as_deref()).await?;
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

        Commands::Search { query, global, limit, interactive } => {
            let neuron_root = project_manager::discover_project_root().await?;
            if interactive {
                search::search_interactive(&neuron_root).await?;
            } else if let Some(q) = query {
                search::search_memory(&neuron_root, &q, global, limit).await?;
            } else {
                println!(
                    "{} Please specify a search query or run with --interactive.",
                    "⚠".yellow().bold()
                );
            }
        }

        Commands::Snapshot { note } => {
            let neuron_root = project_manager::discover_project_root().await?;
            conversation::save_snapshot(&neuron_root, note.as_deref()).await?;
        }

        Commands::Status => {
            match project_manager::discover_project_root().await {
                Ok(root) => {
                    session::print_status(&root).await?;
                    utils::check_path_registration();
                }
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
                    utils::check_path_registration();
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

        Commands::Diagnose => {
            let neuron_root = project_manager::discover_project_root().await.ok();
            utils::run_diagnostics(neuron_root.as_deref()).await?;
        }

        Commands::LinkDeps { parent, child, unlink, list } => {
            if list {
                dependency::list_deps(&parent).await?;
            } else if unlink {
                dependency::unlink_deps(&parent, &child).await?;
            } else {
                dependency::link_deps(&parent, &child).await?;
            }
        }

        Commands::Analyze { parent } => {
            analyzer::analyze_parent(&parent).await?;
        }

        Commands::Session { track } => {
            if track {
                let neuron_root = project_manager::discover_project_root().await?;
                intent::start_tracker(&neuron_root).await?;
            } else {
                println!("{} Please use --track to start the background session focus score poller.", "⚠".yellow().bold());
            }
        }

        Commands::LogError { cmd, err } => {
            let neuron_root = project_manager::discover_project_root().await?;
            intent::write_error_log(&neuron_root, &cmd, &err).await?;
        }

        Commands::Graph { trace } => {
            if let Some(symbol) = trace {
                graph::trace_symbol_cascade(&symbol).await?;
            } else {
                graph::render_topology_graph().await?;
            }
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
        "v13".bright_yellow().bold(),
        "for AI Coding Agents".dimmed()
    );
}
