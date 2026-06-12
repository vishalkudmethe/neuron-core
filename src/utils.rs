//! Shared utility functions used across all Neuron modules.

use anyhow::{Context, Result};
use chrono::Utc;
use colored::Colorize;
use sha2::{Digest, Sha256};
use sqlx::sqlite::SqliteConnectOptions;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use tokio::fs;
use tracing::info;

// ─── Path Utilities ───────────────────────────────────────────────────────────

/// Walk upward from `start` to find the nearest directory containing `.neuron/`.
/// Returns the project root (parent of `.neuron/`), NOT the `.neuron/` path itself.
pub fn find_neuron_root(start: &Path) -> Option<PathBuf> {
    let mut current = start.to_path_buf();
    loop {
        let candidate = current.join(".neuron");
        if candidate.is_dir() {
            return Some(current);
        }
        if !current.pop() {
            break;
        }
    }
    None
}

/// Given a project root, return the `.neuron/` directory path.
pub fn neuron_dir(project_root: &Path) -> PathBuf {
    project_root.join(".neuron")
}

/// Given a project root, return the local SQLite index path.
pub fn local_db_path(project_root: &Path) -> PathBuf {
    neuron_dir(project_root).join("index.sqlite")
}

/// Return the global index path: `~/.neuron/global_index.sqlite`
pub fn global_db_path() -> Result<PathBuf> {
    let home = dirs::home_dir()
        .ok_or_else(|| anyhow::anyhow!("Cannot determine home directory"))?;
    Ok(home.join(".neuron").join("global_index.sqlite"))
}

/// Return the global neuron config directory: `~/.neuron/`
#[allow(dead_code)]
pub fn global_neuron_dir() -> Result<PathBuf> {
    let home = dirs::home_dir()
        .ok_or_else(|| anyhow::anyhow!("Cannot determine home directory"))?;
    Ok(home.join(".neuron"))
}

/// Create directory and all parents if they don't exist.
pub async fn ensure_dir(path: &Path) -> Result<()> {
    fs::create_dir_all(path)
        .await
        .with_context(|| format!("Failed to create directory: {}", path.display()))?;
    Ok(())
}

// ─── Hashing ─────────────────────────────────────────────────────────────────

/// Compute SHA-256 of a file's contents. Returns hex string.
pub async fn sha256_file(path: &Path) -> Result<String> {
    let bytes = fs::read(path)
        .await
        .with_context(|| format!("Failed to read file for hashing: {}", path.display()))?;
    let hash = Sha256::digest(&bytes);
    Ok(hex::encode(hash))
}

/// Compute SHA-256 of a string slice.
pub fn sha256_str(s: &str) -> String {
    let hash = Sha256::digest(s.as_bytes());
    hex::encode(hash)
}

// ─── Machine Identity ─────────────────────────────────────────────────────────

/// Deterministic machine ID from hostname + username.
/// Uses COMPUTERNAME (Windows) / HOSTNAME (Unix) env vars with std fallback.
pub fn machine_id() -> String {
    let hostname = std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .unwrap_or_else(|_| "unknown-host".to_string());
    let username = std::env::var("USERNAME")
        .or_else(|_| std::env::var("USER"))
        .unwrap_or_else(|_| "unknown-user".to_string());
    sha256_str(&format!("{hostname}:{username}"))[..16].to_string()
}

// ─── Backup ───────────────────────────────────────────────────────────────────

/// Create a timestamped backup of `.neuron/` inside `.neuron/backups/`.
pub async fn backup_neuron_dir(project_root: &Path) -> Result<()> {
    let neuron_path = neuron_dir(project_root);
    let backups_dir = neuron_path.join("backups");
    ensure_dir(&backups_dir).await?;

    let timestamp = Utc::now().format("%Y%m%d_%H%M%S").to_string();
    let backup_path = backups_dir.join(format!("backup_{timestamp}"));

    copy_dir_all(&neuron_path, &backup_path).await?;

    info!(
        "Backup created at: {}",
        backup_path.display().to_string().green()
    );
    println!(
        "  {} Backup → {}",
        "✓".green(),
        backup_path.display().to_string().dimmed()
    );
    Ok(())
}

/// Recursively copy a directory. Used for backups.
fn copy_dir_all<'a>(
    src: &'a Path,
    dst: &'a Path,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<()>> + Send + 'a>> {
    Box::pin(async move {
        ensure_dir(dst).await?;
        let mut entries = fs::read_dir(src)
            .await
            .with_context(|| format!("Cannot read dir: {}", src.display()))?;

        while let Some(entry) = entries.next_entry().await? {
            let file_type = entry.file_type().await?;
            let src_path = entry.path();
            let dst_path = dst.join(entry.file_name());

            // Skip backups/ to avoid recursive explosion
            if src_path.ends_with("backups") {
                continue;
            }

            if file_type.is_dir() {
                copy_dir_all(&src_path, &dst_path).await?;
            } else {
                fs::copy(&src_path, &dst_path)
                    .await
                    .with_context(|| {
                        format!("Copy failed: {} → {}", src_path.display(), dst_path.display())
                    })?;
            }
        }
        Ok(())
    })
}

// ─── Formatting ───────────────────────────────────────────────────────────────

/// Format a chrono Duration as a human-readable string.
pub fn format_duration(d: chrono::Duration) -> String {
    if d.num_seconds() < 60 {
        format!("{}s ago", d.num_seconds())
    } else if d.num_minutes() < 60 {
        format!("{}m ago", d.num_minutes())
    } else if d.num_hours() < 24 {
        format!("{}h ago", d.num_hours())
    } else {
        format!("{}d ago", d.num_days())
    }
}

/// Truncate a string to `max_len` chars, appending `…` if truncated.
pub fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}…", &s[..max_len.saturating_sub(1)])
    }
}

// ─── PATH Diagnostics ─────────────────────────────────────────────────────────

/// Check if the currently running `neuron` binary's parent directory is on PATH.
/// Prints a highly visible, OS-specific snippet if not found.
pub fn check_path_registration() {
    let bin_path = match std::env::current_exe() {
        Ok(p) => p,
        Err(_) => return,
    };
    let bin_dir = match bin_path.parent() {
        Some(d) => d.to_path_buf(),
        None    => return,
    };
    let bin_dir_str = bin_dir.to_string_lossy().to_string();

    let path_var  = std::env::var("PATH").unwrap_or_default();
    let separator = if cfg!(windows) { ';' } else { ':' };
    let on_path   = path_var.split(separator).any(|p| {
        std::path::Path::new(p.trim()) == bin_dir.as_path()
    });

    if on_path {
        println!(
            "  {:22} {}",
            "PATH:".dimmed(),
            "neuron binary found on PATH ✓".green()
        );
    } else {
        println!();
        println!("{}", "  ⚠  neuron is NOT on your system PATH.".yellow().bold());
        println!("{}", "  ─────────────────────────────────────────────────".dimmed());
        println!("  To fix permanently, run ONE of these:\n");
        println!("  {}", "PowerShell (permanent):".bright_cyan());
        println!(
            "    {}",
            format!(
                r#"[System.Environment]::SetEnvironmentVariable("PATH", $env:PATH + ";{}", "User")"#,
                bin_dir_str
            ).yellow()
        );
        println!("\n  {}", "PowerShell (current session only):".bright_cyan());
        println!("    {}", format!(r#"$env:PATH += ";{}""#, bin_dir_str).yellow());
        println!("\n  {}", "CMD (permanent):".bright_cyan());
        println!("    {}", format!(r#"setx PATH "%PATH%;{}""#, bin_dir_str).yellow());
        println!("\n  {}", "Bash / Zsh (~/.bashrc or ~/.zshrc):".bright_cyan());
        println!("    {}", format!(r#"export PATH="$PATH:{}""#, bin_dir_str).yellow());
        println!();
    }
}

// ─── Full Diagnostics ─────────────────────────────────────────────────────────

/// Run a comprehensive environment and database health audit.
pub async fn run_diagnostics(project_root: Option<&std::path::Path>) -> Result<()> {
    println!("\n  {}", "NEURON DIAGNOSTIC REPORT".bright_cyan().bold());
    println!("  {}", "═".repeat(52).bright_cyan());

    // ── 1. Binary PATH ────────────────────────────────────────────────────────
    let bin_path    = std::env::current_exe().unwrap_or_default();
    let bin_dir     = bin_path.parent().map(|p| p.to_path_buf()).unwrap_or_default();
    let bin_dir_str = bin_dir.to_string_lossy().to_string();
    let path_var    = std::env::var("PATH").unwrap_or_default();
    let sep         = if cfg!(windows) { ';' } else { ':' };
    let on_path     = path_var.split(sep).any(|p| std::path::Path::new(p.trim()) == bin_dir.as_path());

    diag_row("Binary on PATH",
        if on_path { DiagStatus::Ok("Found in PATH".into()) }
        else { DiagStatus::Warn(format!("Not on PATH — add: {}", bin_dir_str)) }
    );

    // ── 2. Global DB ─────────────────────────────────────────────────────────
    let global_db = global_db_path().unwrap_or_default();
    if global_db.exists() {
        match SqliteConnectOptions::from_str(&format!("sqlite://{}", global_db.display()))
            .map(|o| o.create_if_missing(false))
        {
            Ok(_) => diag_row("Global DB", DiagStatus::Ok(format!("{}", global_db.display()))),
            Err(e) => diag_row("Global DB", DiagStatus::Err(format!("Cannot open: {e}"))),
        }
    } else {
        diag_row("Global DB", DiagStatus::Warn("Not found — run `neuron init` in a project".into()));
    }

    // ── 3. Local DB ──────────────────────────────────────────────────────────
    if let Some(root) = project_root {
        let local_db = local_db_path(root);
        if local_db.exists() {
            // Count memory units
            let opts_str = format!("sqlite://{}", local_db.display());
            if let Ok(opts_parsed) = SqliteConnectOptions::from_str(&opts_str) {
                match sqlx::SqlitePool::connect_with(opts_parsed).await {
                    Ok(pool) => {
                        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM memory_units")
                            .fetch_one(&pool).await.unwrap_or((0,));
                        let loop_count: (i64,) = sqlx::query_as(
                            "SELECT COUNT(*) FROM loop_events WHERE terminated = 0"
                        ).fetch_one(&pool).await.unwrap_or((0,));

                        diag_row("Local DB", if count.0 > 0 {
                            DiagStatus::Ok(format!("{} memory units indexed", count.0))
                        } else {
                            DiagStatus::Warn("0 units — run `neuron watch` to index".into())
                        });
                        diag_row("Loop Guardian", if loop_count.0 == 0 {
                            DiagStatus::Ok("No active loop events".into())
                        } else {
                            DiagStatus::Warn(format!("{} unresolved loop event(s)", loop_count.0))
                        });
                    }
                    Err(e) => diag_row("Local DB", DiagStatus::Err(format!("Cannot open: {e}"))),
                }
            }
        } else {
            diag_row("Local DB", DiagStatus::Warn("Not found — run `neuron watch`".into()));
            diag_row("Loop Guardian", DiagStatus::Warn("DB unavailable — cannot audit".into()));
        }
    } else {
        diag_row("Local DB", DiagStatus::Warn("No project in scope — run from project dir".into()));
        diag_row("Loop Guardian", DiagStatus::Warn("No project in scope".into()));
    }

    // ── 4. Watcher process ───────────────────────────────────────────────────
    diag_row("Watcher Process",
        DiagStatus::Info("No persistent daemon — watcher runs in foreground via `neuron watch`".into())
    );

    println!("  {}\n", "═".repeat(52).bright_cyan());
    Ok(())
}

// Internal diagnostic helpers
enum DiagStatus {
    Ok(String),
    Warn(String),
    Err(String),
    Info(String),
}

fn diag_row(label: &str, status: DiagStatus) {
    let (icon, msg) = match status {
        DiagStatus::Ok(m)   => ("✓".green().bold().to_string(),   m.green().to_string()),
        DiagStatus::Warn(m) => ("⚠".yellow().bold().to_string(),  m.yellow().to_string()),
        DiagStatus::Err(m)  => ("✗".red().bold().to_string(),     m.red().to_string()),
        DiagStatus::Info(m) => ("ℹ".cyan().to_string(),           m.dimmed().to_string()),
    };
    println!("  {}  {:22} {}", icon, label.dimmed(), msg);
}

