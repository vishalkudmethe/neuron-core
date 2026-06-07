//! Git integration using git2.
//! Indexes commits as memory units and reads branch/commit metadata.

use anyhow::Result;
use chrono::Utc;
use colored::Colorize;
use git2::Repository;
use sqlx::SqlitePool;
use std::path::Path;
use tracing::{debug, warn};
use uuid::Uuid;

// ─── Branch & Commit Info ─────────────────────────────────────────────────────

pub fn current_branch(project_root: &Path) -> Result<String> {
    let repo = Repository::discover(project_root)?;
    let head = repo.head()?;
    Ok(head.shorthand().unwrap_or("HEAD").to_string())
}

pub fn last_commit_message(project_root: &Path) -> Result<String> {
    let repo   = Repository::discover(project_root)?;
    let head   = repo.head()?;
    let oid    = head.target().ok_or_else(|| anyhow::anyhow!("No HEAD target"))?;
    let commit = repo.find_commit(oid)?;
    Ok(commit.summary().unwrap_or("(no message)").to_string())
}

// ─── Index Latest Commit ──────────────────────────────────────────────────────

pub async fn index_latest_commit(pool: &SqlitePool, project_root: &Path) -> Result<()> {
    let repo = match Repository::discover(project_root) {
        Ok(r)  => r,
        Err(e) => { warn!("Git repo not found: {e}"); return Ok(()); }
    };

    let head   = repo.head()?;
    let oid    = match head.target() {
        Some(o) => o,
        None    => return Ok(()),
    };
    let commit = repo.find_commit(oid)?;

    let sha     = oid.to_string();
    let msg     = commit.summary().unwrap_or("").to_string();
    let author  = commit.author().name().unwrap_or("unknown").to_string();
    let ts      = Utc::now().to_rfc3339();
    let id      = Uuid::new_v4().to_string();
    let content = format!("commit {sha}\nAuthor: {author}\n\n{msg}");

    sqlx::query(r#"
        INSERT OR IGNORE INTO memory_units
            (id, unit_type, symbol_name, language, content, sha256, created_at, updated_at)
        VALUES (?1, 'git_commit', ?2, 'git', ?3, ?4, ?5, ?6)
    "#)
    .bind(&id)
    .bind(&msg)
    .bind(&content)
    .bind(&sha)
    .bind(&ts)
    .bind(&ts)
    .execute(pool)
    .await?;

    println!(
        "  {} Indexed commit: {} — {}",
        "⚡".bright_yellow(),
        &sha[..8].yellow(),
        msg.bold()
    );

    debug!("Indexed git commit {sha}: {msg}");
    Ok(())
}
