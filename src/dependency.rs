//! Dependency topology linker for cross-project structural tracking.

use anyhow::{Context, Result};
use chrono::Utc;
use colored::Colorize;
use sqlx::SqlitePool;
use std::str::FromStr;
use tabled::{builder::Builder, settings::Style};
use uuid::Uuid;

use crate::utils;

pub const DEPENDENCY_SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS workspace_dependencies (
    id          TEXT PRIMARY KEY,
    parent_id   TEXT NOT NULL,
    child_id    TEXT NOT NULL,
    created_at  TEXT NOT NULL,
    UNIQUE (parent_id, child_id)
);

CREATE TABLE IF NOT EXISTS signature_snapshots (
    id              TEXT PRIMARY KEY,
    project_id      TEXT NOT NULL,
    symbol_name     TEXT NOT NULL,
    symbol_type     TEXT NOT NULL,
    signature_hash  TEXT NOT NULL,
    last_seen_at    TEXT NOT NULL,
    changed_at      TEXT,
    UNIQUE (project_id, symbol_name)
);
"#;

async fn open_global_db() -> Result<SqlitePool> {
    let db_path = utils::global_db_path()?;
    let db_dir  = db_path.parent().expect("global_db has parent");
    utils::ensure_dir(db_dir).await?;

    let opts = sqlx::sqlite::SqliteConnectOptions::from_str(&format!(
        "sqlite://{}", db_path.display()
    ))?
    .create_if_missing(true)
    .busy_timeout(std::time::Duration::from_millis(1500))
    .foreign_keys(true);

    let pool = SqlitePool::connect_with(opts).await
        .with_context(|| format!("Cannot open global index at: {}", db_path.display()))?;

    sqlx::query(DEPENDENCY_SCHEMA).execute(&pool).await
        .context("Failed to apply dependency schema")?;

    Ok(pool)
}

async fn alias_to_id(pool: &SqlitePool, alias: &str) -> Result<(String, String)> {
    let row: Option<(String, String)> = sqlx::query_as(
        "SELECT id, name FROM projects WHERE name = ?1 LIMIT 1"
    )
    .bind(alias)
    .fetch_optional(pool)
    .await
    .context("Global index lookup failed")?;

    row.ok_or_else(|| anyhow::anyhow!(
        "No project found with alias '{}'. Run {} to see registered workspaces.",
        alias, "neuron list".cyan()
    ))
}

pub async fn link_deps(parent_alias: &str, child_alias: &str) -> Result<()> {
    let pool = open_global_db().await?;
    let (parent_id, parent_name) = alias_to_id(&pool, parent_alias).await?;
    let (child_id,  child_name)  = alias_to_id(&pool, child_alias).await?;

    if parent_id == child_id {
        anyhow::bail!("A project cannot depend on itself.");
    }

    sqlx::query(r#"
        INSERT INTO workspace_dependencies (id, parent_id, child_id, created_at)
        VALUES (?1, ?2, ?3, ?4)
        ON CONFLICT(parent_id, child_id) DO UPDATE SET created_at = excluded.created_at
    "#)
    .bind(Uuid::new_v4().to_string())
    .bind(&parent_id)
    .bind(&child_id)
    .bind(Utc::now().to_rfc3339())
    .execute(&pool)
    .await
    .context("Failed to write dependency arc")?;

    println!(
        "\n{} Dependency arc registered:\n  {} {} → {}\n",
        "✓".green().bold(), "Parent:".dimmed(),
        parent_name.bright_cyan().bold(), child_name.yellow().bold()
    );
    Ok(())
}

pub async fn unlink_deps(parent_alias: &str, child_alias: &str) -> Result<()> {
    let pool = open_global_db().await?;
    let (parent_id, _) = alias_to_id(&pool, parent_alias).await?;
    let (child_id,  _) = alias_to_id(&pool, child_alias).await?;

    sqlx::query("DELETE FROM workspace_dependencies WHERE parent_id = ?1 AND child_id = ?2")
        .bind(&parent_id).bind(&child_id).execute(&pool).await?;

    println!("\n{} Dependency arc removed: {} → {}\n",
        "✓".green().bold(), parent_alias.bright_cyan(), child_alias.yellow());
    Ok(())
}

pub async fn list_deps(alias: &str) -> Result<()> {
    let pool = open_global_db().await?;
    let (project_id, project_name) = alias_to_id(&pool, alias).await?;

    let parents: Vec<(String,)> = sqlx::query_as(
        "SELECT p.name FROM workspace_dependencies d \
         JOIN projects p ON p.id = d.parent_id WHERE d.child_id = ?1"
    ).bind(&project_id).fetch_all(&pool).await.unwrap_or_default();

    let children: Vec<(String,)> = sqlx::query_as(
        "SELECT p.name FROM workspace_dependencies d \
         JOIN projects p ON p.id = d.child_id WHERE d.parent_id = ?1"
    ).bind(&project_id).fetch_all(&pool).await.unwrap_or_default();

    println!("\n{} Dependency topology for: {}\n",
        "◈".bright_cyan().bold(), project_name.bold());

    if parents.is_empty() && children.is_empty() {
        println!("  {} No arcs registered. Use {} to add one.\n",
            "ℹ".cyan(),
            "neuron link-deps --parent <alias> --child <alias>".yellow());
        return Ok(());
    }

    let mut builder = Builder::default();
    builder.push_record(["Direction", "Workspace"]);
    for (name,) in &parents  { builder.push_record(["↑ Parent", name]); }
    for (name,) in &children { builder.push_record(["↓ Child",  name]); }

    println!("{}\n", builder.build().with(Style::rounded()).to_string());
    Ok(())
}

pub async fn get_parent_ids(child_project_id: &str) -> Result<Vec<String>> {
    let pool = open_global_db().await?;
    let rows: Vec<(String,)> = sqlx::query_as(
        "SELECT parent_id FROM workspace_dependencies WHERE child_id = ?1"
    )
    .bind(child_project_id)
    .fetch_all(&pool)
    .await
    .context("Failed to query parent IDs")?;

    Ok(rows.into_iter().map(|(id,)| id).collect())
}

pub async fn upsert_signature(
    project_id:     &str,
    symbol_name:    &str,
    symbol_type:    &str,
    signature_hash: &str,
) -> Result<bool> {
    let pool = open_global_db().await?;
    let now  = Utc::now().to_rfc3339();

    let existing: Option<(String,)> = sqlx::query_as(
        "SELECT signature_hash FROM signature_snapshots \
         WHERE project_id = ?1 AND symbol_name = ?2"
    )
    .bind(project_id).bind(symbol_name)
    .fetch_optional(&pool).await?;

    let mutated = matches!(&existing, Some((old,)) if old != signature_hash);

    sqlx::query(r#"
        INSERT INTO signature_snapshots
            (id, project_id, symbol_name, symbol_type, signature_hash, last_seen_at, changed_at)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
        ON CONFLICT(project_id, symbol_name) DO UPDATE SET
            symbol_type    = excluded.symbol_type,
            signature_hash = excluded.signature_hash,
            last_seen_at   = excluded.last_seen_at,
            changed_at     = CASE
                WHEN excluded.signature_hash != signature_snapshots.signature_hash
                    THEN excluded.last_seen_at
                ELSE signature_snapshots.changed_at
            END
    "#)
    .bind(Uuid::new_v4().to_string())
    .bind(project_id).bind(symbol_name).bind(symbol_type)
    .bind(signature_hash).bind(&now)
    .bind(if mutated { Some(now.clone()) } else { None })
    .execute(&pool).await
    .context("Failed to upsert signature snapshot")?;

    Ok(mutated)
}

pub async fn get_recent_mutations(
    project_id:   &str,
    within_hours: i64,
) -> Result<Vec<(String, String, String)>> {
    let pool = open_global_db().await?;
    let cutoff = (Utc::now() - chrono::Duration::hours(within_hours)).to_rfc3339();

    let rows: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT symbol_name, symbol_type, changed_at FROM signature_snapshots \
         WHERE project_id = ?1 AND changed_at IS NOT NULL AND changed_at >= ?2 \
         ORDER BY changed_at DESC"
    )
    .bind(project_id).bind(&cutoff)
    .fetch_all(&pool).await
    .context("Failed to query recent mutations")?;

    Ok(rows)
}

pub async fn project_id_for_alias(alias: &str) -> Result<String> {
    let pool = open_global_db().await?;
    alias_to_id(&pool, alias).await.map(|(id, _)| id)
}
