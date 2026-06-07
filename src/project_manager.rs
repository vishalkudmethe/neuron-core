//! Project Manager — v5 Multi-Project Core
//!
//! Maintains `~/.neuron/global_index.sqlite` to track ALL known Neuron
//! projects across directory changes, restarts, and machine migrations.
//!
//! Key operations:
//!   - `init_project`    → create .neuron/, register globally
//!   - `restore_project` → upward-search for .neuron/, load context
//!   - `switch_project`  → load a different project by name or path
//!   - `list_projects`   → print all known projects
//!   - `register`        → upsert a project into global index

use anyhow::{Context, Result};
use chrono::Utc;
use colored::Colorize;
use sqlx::{sqlite::SqliteConnectOptions, SqlitePool};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use tabled::{builder::Builder, settings::Style};
use tracing::debug;

use crate::manifest::NeuronManifest;
use crate::session;
use crate::utils::{self, ensure_dir, format_duration};

// ─── Global Index Schema ──────────────────────────────────────────────────────

const GLOBAL_SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS projects (
    id              TEXT PRIMARY KEY,
    name            TEXT NOT NULL,
    root_path       TEXT NOT NULL UNIQUE,
    neuron_path     TEXT NOT NULL,
    language        TEXT NOT NULL DEFAULT 'unknown',
    last_accessed   TEXT NOT NULL,
    created_at      TEXT NOT NULL,
    tags            TEXT NOT NULL DEFAULT '[]'
);

CREATE TABLE IF NOT EXISTS path_aliases (
    project_id      TEXT NOT NULL,
    machine_id      TEXT NOT NULL,
    local_path      TEXT NOT NULL,
    PRIMARY KEY (project_id, machine_id)
);
"#;

// ─── Open / Bootstrap Global DB ───────────────────────────────────────────────

async fn open_global_db() -> Result<SqlitePool> {
    let db_path = utils::global_db_path()?;
    let db_dir  = db_path.parent().expect("global_db has parent");
    ensure_dir(db_dir).await?;

    let opts = SqliteConnectOptions::from_str(&format!("sqlite://{}", db_path.display()))?
        .create_if_missing(true)
        .foreign_keys(true);

    let pool = SqlitePool::connect_with(opts)
        .await
        .with_context(|| format!("Failed to open global index at: {}", db_path.display()))?;

    // Bootstrap schema
    sqlx::query(GLOBAL_SCHEMA)
        .execute(&pool)
        .await
        .context("Failed to apply global schema")?;

    Ok(pool)
}

// ─── Registration ─────────────────────────────────────────────────────────────

/// Register (or update) a project in the global index.
pub async fn register(manifest: &NeuronManifest) -> Result<()> {
    let pool = open_global_db().await?;
    let now  = Utc::now().to_rfc3339();
    let tags = serde_json::to_string(&manifest.tags)?;
    let id   = manifest.id.to_string();
    let neuron_path = manifest.root_path.join(".neuron").to_string_lossy().to_string();
    let root_str    = manifest.root_path.to_string_lossy().to_string();

    sqlx::query(r#"
        INSERT INTO projects (id, name, root_path, neuron_path, language, last_accessed, created_at, tags)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
        ON CONFLICT(root_path) DO UPDATE SET
            name          = excluded.name,
            neuron_path   = excluded.neuron_path,
            language      = excluded.language,
            last_accessed = excluded.last_accessed,
            tags          = excluded.tags
    "#)
    .bind(&id)
    .bind(&manifest.name)
    .bind(&root_str)
    .bind(&neuron_path)
    .bind(&manifest.language)
    .bind(&now)
    .bind(&now)
    .bind(&tags)
    .execute(&pool)
    .await
    .context("Failed to register project in global index")?;

    // Register path alias for portability
    let mid = utils::machine_id();
    sqlx::query(r#"
        INSERT INTO path_aliases (project_id, machine_id, local_path)
        VALUES (?1, ?2, ?3)
        ON CONFLICT(project_id, machine_id) DO UPDATE SET local_path = excluded.local_path
    "#)
    .bind(&id)
    .bind(&mid)
    .bind(&root_str)
    .execute(&pool)
    .await
    .context("Failed to register path alias")?;

    debug!("Registered project '{}' (id={}) at {}", manifest.name, id, root_str);
    Ok(())
}

// ─── neuron init ─────────────────────────────────────────────────────────────

pub async fn init_project(root: &Path, name: &str, language: &str) -> Result<()> {
    let neuron_dir    = root.join(".neuron");
    let conv_dir      = neuron_dir.join("conversations");
    let backups_dir   = neuron_dir.join("backups");

    println!(
        "\n{} Initializing Neuron project {}\n  {} {}\n",
        "◉".bright_cyan().bold(),
        name.bold().white(),
        "→".dimmed(),
        root.display().to_string().dimmed()
    );

    // Create directory structure
    for dir in &[&neuron_dir, &conv_dir, &backups_dir] {
        ensure_dir(dir).await?;
        println!("  {} {}", "✓".green(), dir.display().to_string().dimmed());
    }

    // Create manifest
    let manifest = NeuronManifest::new(name, root, language);
    manifest.save(root).await?;
    println!("  {} neuron_manifest.json", "✓".green());

    // Bootstrap local SQLite index
    crate::search::bootstrap_local_db(root).await?;
    println!("  {} index.sqlite (local ledger)", "✓".green());

    // Write initial session context
    session::write_initial_context(root, &manifest).await?;
    println!("  {} session_context.md", "✓".green());

    // Register in global index
    register(&manifest).await?;
    println!("  {} Registered in global index (~/.neuron/)", "✓".green());

    println!(
        "\n{} Project '{}' is ready. Run {} to start watching.\n",
        "✅".green(),
        name.bold().bright_cyan(),
        "neuron watch".yellow().bold()
    );
    Ok(())
}

// ─── neuron restore ───────────────────────────────────────────────────────────

/// Walk upward from `start` to find nearest .neuron/, then load its context.
pub async fn restore_project(start: &Path) -> Result<()> {
    match utils::find_neuron_root(start) {
        None => {
            println!(
                "\n{} No .neuron/ directory found searching upward from:\n  {}\n",
                "⚠".yellow().bold(),
                start.display().to_string().dimmed()
            );
            println!(
                "  Run {} to initialize a new project, or {} to find one by name.\n",
                "neuron init".cyan(),
                "neuron switch <name>".cyan()
            );
        }
        Some(project_root) => {
            println!(
                "\n{} Restoring project context from:\n  {}\n",
                "◈".bright_cyan().bold(),
                project_root.display().to_string().cyan()
            );

            let manifest = NeuronManifest::load(&project_root)
                .await
                .context("Failed to load manifest during restore")?;

            // Re-register in case paths changed (machine migration)
            register(&manifest).await?;

            // Print session context
            session::print_restored_context(&project_root, &manifest).await?;
        }
    }
    Ok(())
}

// ─── neuron switch ────────────────────────────────────────────────────────────

/// Switch to another known project by name or absolute path.
pub async fn switch_project(target: &str) -> Result<()> {
    let pool = open_global_db().await?;

    // Try by exact name first, then by path substring
    let row: Option<(String, String, String)> = sqlx::query_as(
        "SELECT id, name, root_path FROM projects WHERE name = ?1 OR root_path LIKE ?2 LIMIT 1"
    )
    .bind(target)
    .bind(format!("%{target}%"))
    .fetch_optional(&pool)
    .await
    .context("Global index query failed")?;

    match row {
        None => {
            println!(
                "\n{} No project found matching: {}\n",
                "✗".red().bold(),
                target.yellow()
            );
            println!("  Run {} to see all known projects.\n", "neuron list".cyan());
        }
        Some((_, name, root_path)) => {
            let root = PathBuf::from(&root_path);
            if !root.exists() {
                println!(
                    "\n{} Project '{}' was at path:\n  {}\n  but that path no longer exists on this machine.\n",
                    "⚠".yellow().bold(),
                    name.bold(),
                    root_path.dimmed()
                );
                println!(
                    "  If you moved the project, run {} inside the new location.\n",
                    "neuron init".cyan()
                );
                return Ok(());
            }

            println!(
                "\n{} Switching to project: {}\n  {}\n",
                "⇄".bright_cyan().bold(),
                name.bold().white(),
                root_path.dimmed()
            );

            // Update last_accessed
            let now = Utc::now().to_rfc3339();
            sqlx::query("UPDATE projects SET last_accessed = ?1 WHERE name = ?2")
                .bind(&now)
                .bind(&name)
                .execute(&pool)
                .await?;

            let manifest = NeuronManifest::load(&root).await?;
            session::print_restored_context(&root, &manifest).await?;
        }
    }
    Ok(())
}

// ─── neuron list ─────────────────────────────────────────────────────────────

/// Print a table of all known projects from the global index.
pub async fn list_projects(long: bool) -> Result<()> {
    let pool = open_global_db().await?;

    let rows: Vec<(String, String, String, String, String)> = sqlx::query_as(
        "SELECT id, name, root_path, language, last_accessed FROM projects ORDER BY last_accessed DESC"
    )
    .fetch_all(&pool)
    .await
    .context("Failed to query global index")?;

    if rows.is_empty() {
        println!(
            "\n{} No projects registered yet. Run {} to initialize one.\n",
            "ℹ".cyan(),
            "neuron init".yellow()
        );
        return Ok(());
    }

    println!(
        "\n{} {} known project(s):\n",
        "◉".bright_cyan().bold(),
        rows.len()
    );

    let mut builder = Builder::default();
    if long {
        builder.push_record(["#", "Name", "Language", "Last Accessed", "Root Path"]);
    } else {
        builder.push_record(["#", "Name", "Language", "Last Accessed"]);
    }

    for (i, (_, name, root_path, language, last_accessed)) in rows.iter().enumerate() {
        // Parse last_accessed for human-readable display
        let accessed = chrono::DateTime::parse_from_rfc3339(last_accessed)
            .map(|dt| {
                let dur = Utc::now().signed_duration_since(dt.with_timezone(&Utc));
                format_duration(dur)
            })
            .unwrap_or_else(|_| last_accessed.clone());

        if long {
            builder.push_record([
                &format!("{}", i + 1),
                name,
                language,
                &accessed,
                root_path,
            ]);
        } else {
            builder.push_record([&format!("{}", i + 1), name, language, &accessed]);
        }
    }

    let table = builder.build().with(Style::rounded()).to_string();
    println!("{table}\n");
    Ok(())
}
