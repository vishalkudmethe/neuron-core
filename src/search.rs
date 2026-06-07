//! SQLite FTS5 search and local ledger management.

use anyhow::{Context, Result};
use chrono::Utc;
use colored::Colorize;
use sqlx::{sqlite::SqliteConnectOptions, SqlitePool};
use std::path::Path;
use std::str::FromStr;
use tracing::debug;
use uuid::Uuid;

use crate::parser::Symbol;
use crate::utils;

// ─── Schema ───────────────────────────────────────────────────────────────────

const LOCAL_SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS memory_units (
    id          TEXT PRIMARY KEY,
    project_id  TEXT NOT NULL DEFAULT '',
    unit_type   TEXT NOT NULL,
    path        TEXT,
    symbol_name TEXT,
    language    TEXT,
    content     TEXT,
    sha256      TEXT,
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_memory_path     ON memory_units(path);
CREATE INDEX IF NOT EXISTS idx_memory_type     ON memory_units(unit_type);
CREATE INDEX IF NOT EXISTS idx_memory_updated  ON memory_units(updated_at);

CREATE VIRTUAL TABLE IF NOT EXISTS memory_fts USING fts5(
    id        UNINDEXED,
    content,
    symbol_name,
    path,
    content='memory_units',
    content_rowid='rowid'
);

CREATE TABLE IF NOT EXISTS loop_events (
    id         TEXT PRIMARY KEY,
    project_id TEXT NOT NULL DEFAULT '',
    pattern    TEXT NOT NULL,
    count      INTEGER NOT NULL,
    first_seen TEXT NOT NULL,
    last_seen  TEXT NOT NULL,
    terminated INTEGER NOT NULL DEFAULT 0
);
"#;

// ─── Open / Bootstrap ─────────────────────────────────────────────────────────

pub async fn open_local_db(db_path: &Path) -> Result<SqlitePool> {
    let opts = SqliteConnectOptions::from_str(&format!("sqlite://{}", db_path.display()))?
        .create_if_missing(true)
        .foreign_keys(true);
    let pool = SqlitePool::connect_with(opts)
        .await
        .with_context(|| format!("Cannot open local DB: {}", db_path.display()))?;
    Ok(pool)
}

pub async fn bootstrap_local_db(project_root: &Path) -> Result<SqlitePool> {
    let db_path = utils::local_db_path(project_root);
    let pool = open_local_db(&db_path).await?;
    sqlx::query(LOCAL_SCHEMA)
        .execute(&pool)
        .await
        .context("Failed to apply local schema")?;
    Ok(pool)
}

// ─── Upsert ───────────────────────────────────────────────────────────────────

pub async fn upsert_file(
    pool:         &SqlitePool,
    project_root: &Path,
    path:         &Path,
    sha256:       &str,
    symbols:      &[Symbol],
) -> Result<()> {
    let now      = Utc::now().to_rfc3339();
    let path_str = path.to_string_lossy().to_string();
    let language = crate::parser::detect_language(path)
        .unwrap_or("unknown")
        .to_string();

    // Read file content (capped at 8 KB for indexing)
    let content = tokio::fs::read_to_string(path)
        .await
        .map(|s| s.chars().take(8192).collect::<String>())
        .unwrap_or_default();

    // Upsert the file-level memory unit
    let file_id = Uuid::new_v4().to_string();
    sqlx::query(r#"
        INSERT INTO memory_units (id, unit_type, path, language, content, sha256, created_at, updated_at)
        VALUES (?1, 'file', ?2, ?3, ?4, ?5, ?6, ?7)
        ON CONFLICT(id) DO UPDATE SET
            content    = excluded.content,
            sha256     = excluded.sha256,
            updated_at = excluded.updated_at
    "#)
    .bind(&file_id)
    .bind(&path_str)
    .bind(&language)
    .bind(&content)
    .bind(sha256)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await?;

    // Upsert each extracted symbol
    for sym in symbols {
        let sym_id = Uuid::new_v4().to_string();
        sqlx::query(r#"
            INSERT INTO memory_units (id, unit_type, path, symbol_name, language, content, sha256, created_at, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
        "#)
        .bind(&sym_id)
        .bind(sym.kind.to_string())
        .bind(&path_str)
        .bind(&sym.name)
        .bind(&sym.language)
        .bind(&sym.snippet)
        .bind(sha256)
        .bind(&now)
        .bind(&now)
        .execute(pool)
        .await?;
    }

    // Rebuild FTS index for changed rows
    sqlx::query("INSERT INTO memory_fts(memory_fts) VALUES('rebuild')")
        .execute(pool)
        .await
        .ok(); // Non-fatal if FTS rebuild fails

    debug!("Upserted {} symbols for {}", symbols.len(), path_str);
    Ok(())
}

// ─── Search ───────────────────────────────────────────────────────────────────

pub async fn search_memory(
    project_root: &Path,
    query:        &str,
    _global:      bool,
    limit:        usize,
) -> Result<()> {
    let db_path = utils::local_db_path(project_root);
    if !db_path.exists() {
        println!(
            "\n{} No memory index found. Run {} first.\n",
            "⚠".yellow(), "neuron watch".cyan()
        );
        return Ok(());
    }

    let pool = open_local_db(&db_path).await?;

    // FTS5 search with ranking
    let rows: Vec<(String, String, Option<String>, Option<String>)> = sqlx::query_as(r#"
        SELECT m.unit_type, m.path, m.symbol_name, snippet(memory_fts, 1, '[', ']', '...', 10)
        FROM memory_fts
        JOIN memory_units m ON memory_fts.id = m.id
        WHERE memory_fts MATCH ?1
        ORDER BY rank
        LIMIT ?2
    "#)
    .bind(query)
    .bind(limit as i64)
    .fetch_all(&pool)
    .await
    .unwrap_or_default();

    if rows.is_empty() {
        println!("\n{} No results for: {}\n", "○".dimmed(), query.yellow());
        return Ok(());
    }

    println!(
        "\n{} {} result(s) for {}\n",
        "◉".bright_cyan().bold(),
        rows.len(),
        query.yellow().bold()
    );
    println!("  {}", "─".repeat(55).dimmed());

    for (i, (unit_type, path, symbol_name, snippet)) in rows.iter().enumerate() {
        let label = match symbol_name.as_deref() {
            Some(s) => format!("{} ({})", s.bold().white(), unit_type.dimmed()),
            None    => unit_type.dimmed().to_string(),
        };
        println!("  {}. {}", i + 1, label);
        println!("     {} {}", "📄".dimmed(), path.cyan());
        if let Some(snip) = snippet {
            println!("     {}", snip.dimmed());
        }
        println!();
    }

    Ok(())
}
