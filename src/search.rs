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
    id              TEXT PRIMARY KEY,
    project_id      TEXT NOT NULL DEFAULT '',
    unit_type       TEXT NOT NULL,
    file_path       TEXT,
    symbol_name     TEXT,
    symbol_type     TEXT,
    language        TEXT,
    content         TEXT,
    semantic_intent TEXT,
    sha256          TEXT,
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_memory_file_path ON memory_units(file_path);
CREATE INDEX IF NOT EXISTS idx_memory_type      ON memory_units(unit_type);
CREATE INDEX IF NOT EXISTS idx_memory_updated   ON memory_units(updated_at);

CREATE VIRTUAL TABLE IF NOT EXISTS memory_fts USING fts5(
    id              UNINDEXED,
    content,
    symbol_name,
    symbol_type,
    file_path,
    semantic_intent,
    content='memory_units',
    content_rowid='rowid'
);

-- Triggers to keep FTS in sync
CREATE TRIGGER IF NOT EXISTS memory_units_ai
    AFTER INSERT ON memory_units BEGIN
    INSERT INTO memory_fts(rowid, id, content, symbol_name, symbol_type, file_path, semantic_intent)
    VALUES (new.rowid, new.id, new.content, new.symbol_name, new.symbol_type, new.file_path, new.semantic_intent);
END;

CREATE TRIGGER IF NOT EXISTS memory_units_ad
    AFTER DELETE ON memory_units BEGIN
    INSERT INTO memory_fts(memory_fts, rowid, id, content, symbol_name, symbol_type, file_path, semantic_intent)
    VALUES ('delete', old.rowid, old.id, old.content, old.symbol_name, old.symbol_type, old.file_path, old.semantic_intent);
END;

CREATE TRIGGER IF NOT EXISTS memory_units_au
    AFTER UPDATE ON memory_units BEGIN
    INSERT INTO memory_fts(memory_fts, rowid, id, content, symbol_name, symbol_type, file_path, semantic_intent)
    VALUES ('delete', old.rowid, old.id, old.content, old.symbol_name, old.symbol_type, old.file_path, old.semantic_intent);
    INSERT INTO memory_fts(rowid, id, content, symbol_name, symbol_type, file_path, semantic_intent)
    VALUES (new.rowid, new.id, new.content, new.symbol_name, new.symbol_type, new.file_path, new.semantic_intent);
END;

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
    
    // Check if the old schema is present (i.e. has 'path' instead of 'file_path')
    let has_old_schema: bool = sqlx::query("PRAGMA table_info(memory_units)")
        .fetch_all(&pool)
        .await
        .map(|rows| rows.iter().any(|row| {
            let name: String = sqlx::Row::get(row, "name");
            name == "path"
        }))
        .unwrap_or(false);

    if has_old_schema {
        debug!("Detected old schema. Re-bootstrapping database...");
        let _ = sqlx::query("DROP TABLE IF EXISTS memory_units").execute(&pool).await;
        let _ = sqlx::query("DROP TABLE IF EXISTS memory_fts").execute(&pool).await;
    }

    sqlx::query(LOCAL_SCHEMA)
        .execute(&pool)
        .await
        .context("Failed to apply local schema")?;
    Ok(pool)
}

// ─── Upsert ───────────────────────────────────────────────────────────────────

pub async fn upsert_file(
    pool:          &SqlitePool,
    _project_root: &Path,
    path:          &Path,
    sha256:        &str,
    symbols:       &[Symbol],
) -> Result<()> {
    let now      = Utc::now().to_rfc3339();
    let path_str = path.to_string_lossy().to_string();
    let language = crate::parser::detect_language(path)
        .unwrap_or("unknown")
        .to_string();

    // Read file content (capped at 8 KB for indexing) and sanitize
    let content = tokio::fs::read_to_string(path)
        .await
        .map(|s| s.chars().take(8192).collect::<String>())
        .unwrap_or_default();
    let sanitized_content = crate::sanitize::sanitize_content(&content);

    // 1. Delete old memory units for this file path (clears out old symbols)
    sqlx::query("DELETE FROM memory_units WHERE file_path = ?1")
        .bind(&path_str)
        .execute(pool)
        .await?;

    // 2. Insert the file-level memory unit
    let file_id = Uuid::new_v4().to_string();
    sqlx::query(r#"
        INSERT INTO memory_units (id, unit_type, file_path, language, content, sha256, created_at, updated_at)
        VALUES (?1, 'file', ?2, ?3, ?4, ?5, ?6, ?7)
    "#)
    .bind(&file_id)
    .bind(&path_str)
    .bind(&language)
    .bind(&sanitized_content)
    .bind(sha256)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await?;

    // 3. Insert each extracted symbol (sanitized)
    for sym in symbols {
        let sym_id = Uuid::new_v4().to_string();
        let sanitized_snippet = crate::sanitize::sanitize_content(&sym.snippet);
        let sanitized_intent  = crate::sanitize::sanitize_content(&sym.semantic_intent);
        sqlx::query(r#"
            INSERT INTO memory_units (id, unit_type, file_path, symbol_name, symbol_type, language, content, semantic_intent, sha256, created_at, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
        "#)
        .bind(&sym_id)
        .bind(sym.kind.to_string())
        .bind(&path_str)
        .bind(&sym.name)
        .bind(sym.kind.to_string())
        .bind(&sym.language)
        .bind(&sanitized_snippet)
        .bind(&sanitized_intent)
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
        SELECT m.unit_type, m.file_path, m.symbol_name, snippet(memory_fts, 1, '[', ']', '...', 10)
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

    for (i, (unit_type, file_path, symbol_name, snippet)) in rows.iter().enumerate() {
        let label = match symbol_name.as_deref() {
            Some(s) => format!("{} ({})", s.bold().white(), unit_type.dimmed()),
            None    => unit_type.dimmed().to_string(),
        };
        println!("  {}. {}", i + 1, label);
        println!("     {} {}", "📄".dimmed(), file_path.cyan());
        if let Some(snip) = snippet {
            println!("     {}", snip.dimmed());
        }
        println!();
    }

    Ok(())
}

// ─── Interactive Search Shell ─────────────────────────────────────────────────

/// Launch a persistent interactive FTS5 query shell.
/// Type a keyword and press Enter to see ranked results.
/// Special commands: `:q` / `:quit` to exit, `:help` for usage.
pub async fn search_interactive(project_root: &Path) -> Result<()> {
    use std::io::{self, BufRead, Write};

    let db_path = utils::local_db_path(project_root);
    if !db_path.exists() {
        println!(
            "\n{} No memory index found. Run {} first.\n",
            "⚠".yellow(), "neuron watch".cyan()
        );
        return Ok(());
    }

    let pool = open_local_db(&db_path).await?;

    println!("\n{}", "╔══════════════════════════════════════════════════════╗".bright_cyan());
    println!("{}", "║   NEURON INTERACTIVE SEARCH — FTS5 Query Shell      ║".bright_cyan().bold());
    println!("{}", "╚══════════════════════════════════════════════════════╝".bright_cyan());
    println!("  {} Type a search term and press Enter.", "›".cyan());
    println!("  {} Special: {} to exit, {} for help.\n", "›".cyan(), ":q".yellow(), ":help".yellow());

    let stdin  = io::stdin();
    let stdout = io::stdout();

    loop {
        // Print prompt
        {
            let mut out = stdout.lock();
            write!(out, "{} ", "neuron›".bright_cyan().bold())?;
            out.flush()?;
        }

        // Read line
        let mut line = String::new();
        if stdin.lock().read_line(&mut line).is_err() || line.is_empty() {
            break;
        }
        let query = line.trim();

        // Special commands
        match query {
            "" => continue,
            ":q" | ":quit" | ":exit" => {
                println!("{} Exiting interactive search.\n", "◎".dimmed());
                break;
            }
            ":help" => {
                println!("\n  {}", "NEURON INTERACTIVE SEARCH HELP".bright_cyan());
                println!("  ─────────────────────────────────────────────────");
                println!("  Type any keyword to search symbols, file paths, and semantic intents.");
                println!("  Results are ranked by FTS5 relevance score.\n");
                println!("  {}", "Special commands:".bold());
                println!("    {} — exit the search shell", ":q / :quit".yellow());
                println!("    {} — show this help", ":help".yellow());
                println!("    {} — clear results (re-prompt)", "<Enter>".yellow());
                println!();
                continue;
            }
            _ => {}
        }

        // Run FTS5 query
        let rows: Vec<(String, String, Option<String>, Option<String>, Option<String>)> =
            sqlx::query_as(r#"
                SELECT m.unit_type, m.file_path, m.symbol_name, m.semantic_intent,
                       snippet(memory_fts, 1, '[', ']', '...', 12)
                FROM memory_fts
                JOIN memory_units m ON memory_fts.id = m.id
                WHERE memory_fts MATCH ?1
                ORDER BY rank
                LIMIT 15
            "#)
            .bind(query)
            .fetch_all(&pool)
            .await
            .unwrap_or_default();

        if rows.is_empty() {
            println!("  {} No results for: {}\n", "○".dimmed(), query.yellow());
        } else {
            println!(
                "\n  {} {} result(s) for {}\n  {}\n",
                "◉".bright_cyan().bold(),
                rows.len(),
                query.yellow().bold(),
                "─".repeat(50).dimmed()
            );
            for (i, (unit_type, file_path, symbol_name, intent, snippet)) in rows.iter().enumerate() {
                let label = match symbol_name.as_deref() {
                    Some(s) => format!("{} ({})", s.bold().white(), unit_type.dimmed()),
                    None    => unit_type.dimmed().to_string(),
                };
                println!("  {}. {}", i + 1, label);
                println!("     {} {}", "📄".dimmed(), file_path.cyan());
                if let Some(i) = intent.as_deref().filter(|s| !s.is_empty()) {
                    println!("     {} {}", "💡".dimmed(), i.dimmed());
                }
                if let Some(snip) = snippet {
                    println!("     {}", snip.dimmed());
                }
                println!();
            }
        }
    }

    Ok(())
}
