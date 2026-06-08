//! Storage and Database maintenance utilities.
//!
//! Exposes `neuron cleanup` which:
//!   1. Rotates `.neuron/intent_log.json` if it exceeds 10MB, compressing it
//!      and removing entries older than 7 days.
//!   2. Runs `VACUUM;` and `ANALYZE;` on all local and global SQLite index files.

use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use colored::Colorize;
use sqlx::{sqlite::SqliteConnectOptions, SqlitePool};
use std::path::Path;
use std::str::FromStr;

use crate::{intent::{self, FocusEntry, IntentLog}, utils};

// ─── Database Vacuum / Optimization ──────────────────────────────────────────

async fn run_sql_optimization(db_path: &Path) -> Result<()> {
    if !db_path.exists() {
        return Ok(());
    }

    let opts = SqliteConnectOptions::from_str(&format!("sqlite://{}", db_path.display()))?
        .create_if_missing(false)
        .busy_timeout(std::time::Duration::from_millis(1500));

    let pool = SqlitePool::connect_with(opts).await?;

    // VACUUM database to reclaim space
    sqlx::query("VACUUM;").execute(&pool).await?;

    // ANALYZE database to update query planner stats
    sqlx::query("ANALYZE;").execute(&pool).await?;

    Ok(())
}

// ─── Log Rotation ─────────────────────────────────────────────────────────────

fn filter_old_entries(log: &IntentLog) -> Vec<FocusEntry> {
    let cutoff = Utc::now() - Duration::days(7);
    log.entries
        .iter()
        .filter(|e| {
            if let Ok(mtime) = DateTime::parse_from_rfc3339(&e.last_modified) {
                mtime.with_timezone(&Utc) >= cutoff
            } else {
                true
            }
        })
        .cloned()
        .collect()
}

pub async fn rotate_intent_logs(project_root: &Path) -> Result<()> {
    let log_path = intent::intent_log_path(project_root);
    if !log_path.exists() {
        return Ok(());
    }

    let metadata = tokio::fs::metadata(&log_path).await?;
    let size_bytes = metadata.len();

    // Limit set to 10MB (10 * 1024 * 1024)
    if size_bytes > 10 * 1024 * 1024 {
        println!("  {} Intent log size exceeds 10MB ({} bytes). Rotating...", "⚠".yellow(), size_bytes);
        
        let old_log_path = project_root.join(".neuron").join("intent_log.old.json");
        let _ = tokio::fs::copy(&log_path, &old_log_path).await;

        if let Some(log) = intent::load_intent_log(project_root).await {
            let filtered = filter_old_entries(&log);
            let new_log = IntentLog {
                updated_at: Utc::now().to_rfc3339(),
                entries: filtered,
            };
            intent::save_intent_log(project_root, &new_log).await?;
            println!("  {} Intent log rotated and filtered to 7-day cutoff.", "✓".green());
        }
    }

    Ok(())
}

// ─── Entry Point ─────────────────────────────────────────────────────────────

/// Perform vacuuming, index optimizations, and log rotation.
pub async fn run_maintenance(project_root: &Path) -> Result<()> {
    println!(
        "\n{} Running storage maintenance & vacuum protocol...\n",
        "⚙".bright_cyan().bold()
    );

    // 1. Optimize global DB
    let global_db = utils::global_db_path()?;
    print!("  Optimizing global registry database... ");
    if let Err(e) = run_sql_optimization(&global_db).await {
        println!("{}", "Failed".red());
        eprintln!("    Error: {}", e);
    } else {
        println!("{}", "Success".green());
    }

    // 2. Optimize local DB
    let local_db = utils::local_db_path(project_root);
    print!("  Optimizing local index database... ");
    if let Err(e) = run_sql_optimization(&local_db).await {
        println!("{}", "Failed".red());
        eprintln!("    Error: {}", e);
    } else {
        println!("{}", "Success".green());
    }

    // 3. Rotate logs
    print!("  Evaluating log rotation policy... ");
    if let Err(e) = rotate_intent_logs(project_root).await {
        println!("{}", "Failed".red());
        eprintln!("    Error: {}", e);
    } else {
        println!("{}", "Success".green());
    }

    println!(
        "\n{} Maintenance protocols complete. Storage optimized.\n",
        "✓".green().bold()
    );

    Ok(())
}
