//! Live Intent Tracker — real-time developer focus engine.
//!
//! `neuron session --track` launches a background polling loop that:
//!   1. Walks the project index for all known file paths.
//!   2. Reads OS mtime for each file.
//!   3. Assigns a focus score (HIGH/MEDIUM/LOW) based on edit recency.
//!   4. Serialises the scored state to `.neuron/intent_log.json` every 15s.
//!
//! `neuron log-error --cmd <cmd> --err <msg>` writes `.neuron/last_error.json`
//! so the stream compiler can inject failure context automatically.

use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use colored::Colorize;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tokio::time;

// ─── Data Structures ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FocusEntry {
    pub file_path:     String,
    pub score:         u32,
    pub last_modified: String, // RFC3339
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntentLog {
    pub updated_at: String,
    pub entries:    Vec<FocusEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorLog {
    pub command:   String,
    pub stderr:    String,
    pub logged_at: String,
}

// ─── Score constants ───────────────────────────────────────────────────────────

const SCORE_HIGH:   u32 = 100; // modified within 2 minutes
const SCORE_MEDIUM: u32 = 50;  // modified within 10 minutes
const SCORE_LOW:    u32 = 10;  // not recently modified

fn score_file(last_modified: DateTime<Utc>) -> u32 {
    let age = Utc::now().signed_duration_since(last_modified);
    if age < Duration::minutes(2) {
        SCORE_HIGH
    } else if age < Duration::minutes(10) {
        SCORE_MEDIUM
    } else {
        SCORE_LOW
    }
}

// ─── Path helpers ─────────────────────────────────────────────────────────────

pub fn intent_log_path(project_root: &Path) -> PathBuf {
    project_root.join(".neuron").join("intent_log.json")
}

pub fn error_log_path(project_root: &Path) -> PathBuf {
    project_root.join(".neuron").join("last_error.json")
}

// ─── Intent Log I/O ───────────────────────────────────────────────────────────

pub async fn load_intent_log(project_root: &Path) -> Option<IntentLog> {
    let path = intent_log_path(project_root);
    let raw  = tokio::fs::read_to_string(&path).await.ok()?;
    serde_json::from_str(&raw).ok()
}

pub async fn save_intent_log(project_root: &Path, log: &IntentLog) -> Result<()> {
    let path = intent_log_path(project_root);
    let json = serde_json::to_string_pretty(log)?;
    tokio::fs::write(&path, json).await?;
    Ok(())
}

// ─── Error Log I/O ───────────────────────────────────────────────────────────

pub async fn write_error_log(project_root: &Path, command: &str, stderr: &str) -> Result<()> {
    let log = ErrorLog {
        command:   command.to_string(),
        stderr:    stderr.to_string(),
        logged_at: Utc::now().to_rfc3339(),
    };
    let path = error_log_path(project_root);
    let json = serde_json::to_string_pretty(&log)?;
    tokio::fs::write(&path, json).await?;
    println!(
        "\n{} Error logged. Next stream payload will include {} section.\n",
        "✓".green().bold(),
        "🔴 Active Execution Failure".red().bold()
    );
    Ok(())
}

/// Load the error log only if it was written within the last 10 minutes.
pub async fn load_fresh_error_log(project_root: &Path) -> Option<ErrorLog> {
    let path = error_log_path(project_root);
    let raw  = tokio::fs::read_to_string(&path).await.ok()?;
    let log: ErrorLog = serde_json::from_str(&raw).ok()?;

    let logged_at: DateTime<Utc> = log.logged_at.parse().ok()?;
    if Utc::now().signed_duration_since(logged_at) > Duration::minutes(10) {
        return None;
    }
    Some(log)
}

// ─── Score Poll ───────────────────────────────────────────────────────────────

/// Single poll pass: scan known file paths, compute scores, return sorted entries.
async fn poll_scores(project_root: &Path) -> Result<Vec<FocusEntry>> {
    let db_path = crate::utils::local_db_path(project_root);
    if !db_path.exists() {
        return Ok(vec![]);
    }

    let pool = crate::search::open_local_db(&db_path).await?;
    let files: Vec<(String,)> = sqlx::query_as(
        "SELECT DISTINCT file_path FROM memory_units WHERE unit_type = 'file'"
    )
    .fetch_all(&pool)
    .await
    .unwrap_or_default();

    let mut entries: Vec<FocusEntry> = vec![];

    for (file_path,) in files {
        let path = PathBuf::from(&file_path);
        if !path.exists() {
            continue;
        }

        let mtime = match tokio::fs::metadata(&path).await {
            Ok(m) => match m.modified() {
                Ok(t)  => DateTime::<Utc>::from(t),
                Err(_) => Utc::now() - Duration::hours(24),
            },
            Err(_) => continue,
        };

        let score = score_file(mtime);
        entries.push(FocusEntry {
            file_path,
            score,
            last_modified: mtime.to_rfc3339(),
        });
    }

    // Sort by score descending, then by last_modified descending
    entries.sort_by(|a, b| b.score.cmp(&a.score).then(b.last_modified.cmp(&a.last_modified)));

    // Cap at 500 entries
    entries.truncate(500);

    Ok(entries)
}

// ─── Background Tracker ───────────────────────────────────────────────────────

/// Start the background intent tracking loop. Runs indefinitely until process exits.
/// Polls every 15 seconds and updates `.neuron/intent_log.json`.
pub async fn start_tracker(project_root: &Path) -> Result<()> {
    println!(
        "\n{} Intent tracker started — polling every 15s\n  {} Focus state → {}\n",
        "◎".bright_cyan().bold(),
        "→".dimmed(),
        intent_log_path(project_root).display().to_string().dimmed()
    );

    let project_root = project_root.to_path_buf();
    let mut interval = time::interval(std::time::Duration::from_secs(15));

    loop {
        interval.tick().await;

        match poll_scores(&project_root).await {
            Ok(entries) => {
                let high_count = entries.iter().filter(|e| e.score >= SCORE_HIGH).count();
                let log = IntentLog {
                    updated_at: Utc::now().to_rfc3339(),
                    entries,
                };
                if let Err(e) = save_intent_log(&project_root, &log).await {
                    eprintln!("  {} Intent log write error: {}", "⚠".yellow(), e);
                } else if high_count > 0 {
                    println!(
                        "  {} Focus update: {} HIGH-priority file(s) in active edit zone",
                        "◎".bright_cyan(),
                        high_count
                    );
                }
            }
            Err(e) => eprintln!("  {} Poll error: {}", "⚠".yellow(), e),
        }
    }
}
