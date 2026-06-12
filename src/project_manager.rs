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

/// Discover the active project root path.
/// First searches upward from CWD. If not found, queries the global database for the
/// most recently accessed project whose root path still exists on disk.
pub async fn discover_project_root() -> Result<PathBuf> {
    let cwd = std::env::current_dir()?;
    if let Some(root) = utils::find_neuron_root(&cwd) {
        return Ok(root);
    }

    // Fall back to global index
    let pool = open_global_db().await?;
    let rows: Vec<(String, String)> = sqlx::query_as(
        "SELECT name, root_path FROM projects ORDER BY last_accessed DESC"
    )
    .fetch_all(&pool)
    .await
    .unwrap_or_default();

    for (name, root_path) in rows {
        let path = PathBuf::from(&root_path);
        if path.join(".neuron").exists() {
            debug!("Discovered project root from global index: {} ({})", name, root_path);
            return Ok(path);
        }
    }

    anyhow::bail!(
        "No Neuron project detected in current directory or parents, and no valid project found in global index.\n\n\
        To start a new project:\n  {}\n\n\
        Or switch to an existing project:\n  {}",
        "neuron init".cyan(),
        "neuron switch <name>".cyan()
    )
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
    // Helpful message suggesting adding to PATH
    println!(
        "  {} Hint: Add this directory or the binary location to your PATH to run 'neuron' globally.\n",
        "💡".yellow()
    );
    Ok(())
}

// ─── neuron restore ───────────────────────────────────────────────────────────

/// Walk upward from `start` to find nearest .neuron/ (or fall back to global discovery), then load its context.
pub async fn restore_project(start: &Path) -> Result<()> {
    let project_root = match utils::find_neuron_root(start) {
        Some(root) => root,
        None => {
            // Fall back to global discovery
            discover_project_root().await?
        }
    };

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

// ─── neuron power-up ──────────────────────────────────────────────────────────

/// Ingest an external workspace into the Neuron global registry.
///
/// Steps:
///   1. Resolve `target` to an absolute canonical path.
///   2. Scaffold `.neuron/` if not already present.
///   3. Write a starter `Neuron.toml` matching the detected primary language.
///   4. Deep-crawl every parseable file → extract symbols → sanitize → upsert into
///      that project's local `index.sqlite` (created if missing).
///   5. Register the workspace in `~/.neuron/global_index.sqlite` under `alias`.
pub async fn power_up(target: &Path, alias: Option<&str>) -> Result<()> {
    // ── 1. Resolve absolute path ──────────────────────────────────────────────
    let target = target.canonicalize().with_context(|| {
        format!("Cannot resolve path: {} — does it exist?", target.display())
    })?;

    let project_name = alias
        .map(|s| s.to_string())
        .unwrap_or_else(|| {
            target
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "unnamed".to_string())
        });

    println!(
        "\n{} Power-up: ingesting workspace {}\n  {} {}\n",
        "⚡".bright_yellow().bold(),
        project_name.bold().white(),
        "→".dimmed(),
        target.display().to_string().dimmed()
    );

    // ── 2. Scaffold .neuron/ ─────────────────────────────────────────────────
    let neuron_dir  = target.join(".neuron");
    let conv_dir    = neuron_dir.join("conversations");
    let backups_dir = neuron_dir.join("backups");
    for dir in &[&neuron_dir, &conv_dir, &backups_dir] {
        ensure_dir(dir).await?;
        println!("  {} {}", "✓".green(), dir.display().to_string().dimmed());
    }

    // ── 3. Detect primary language & write Neuron.toml if missing ────────────
    let detected_lang = detect_primary_language(&target).await;
    let toml_path = target.join("Neuron.toml");
    if !toml_path.exists() {
        let default_toml = format!(
            "# Neuron Configuration — auto-generated by `neuron power-up`\n\
             profile = \"antigravity\"\n\
             token_cap = 250000\n\
             max_granularity = true\n\
             include_evolution_ledger = true\n\
             # primary_language = \"{}\"\n",
            detected_lang
        );
        tokio::fs::write(&toml_path, &default_toml).await?;
        println!("  {} Neuron.toml written (profile=antigravity, lang={})", "✓".green(), detected_lang);
    } else {
        println!("  {} Neuron.toml already exists — preserving.", "·".dimmed());
    }

    // ── 4. Bootstrap local SQLite & deep-crawl ───────────────────────────────
    crate::search::bootstrap_local_db(&target).await?;
    println!("  {} index.sqlite bootstrapped", "✓".green());

    let db_path = crate::utils::local_db_path(&target);
    let pool    = crate::search::open_local_db(&db_path).await?;

    // Load config for token budgeting
    let config = crate::config::NeuronConfig::load(&target).await;

    println!("  {} Crawling files (profile: {}) …", "⟳".cyan(), config.profile);

    let mut file_count   = 0usize;
    let mut symbol_count = 0usize;

    // Walk every file in the target directory
    let walk = ignore::WalkBuilder::new(&target)
        .hidden(false)
        .git_ignore(true)
        .build();

    for entry in walk.flatten() {
        let path = entry.path().to_path_buf();
        if !path.is_file() {
            continue;
        }
        // Skip known build/cache artifacts
        let path_str = path.to_string_lossy();
        if path_str.contains("\\target\\")
            || path_str.contains("/target/")
            || path_str.contains(".neuron")
            || path_str.contains(".git")
            || path_str.ends_with(".tmp")
        {
            continue;
        }
        // Only index parseable languages
        if crate::parser::detect_language(&path).is_none() {
            continue;
        }

        let hash = match crate::utils::sha256_file(&path).await {
            Ok(h)  => h,
            Err(_) => continue,
        };

        let symbols = crate::parser::extract_symbols(&path).await.unwrap_or_default();
        symbol_count += symbols.len();

        crate::search::upsert_file(&pool, &target, &path, &hash, &symbols).await?;
        file_count += 1;
    }

    println!(
        "  {} Indexed {} file(s), {} symbol(s)",
        "✓".green(),
        file_count,
        symbol_count
    );

    // ── 5. Create manifest & register in global index ────────────────────────
    let manifest_path = neuron_dir.join("neuron_manifest.json");
    let manifest = if manifest_path.exists() {
        NeuronManifest::load(&target).await.unwrap_or_else(|_| {
            NeuronManifest::new(&project_name, &target, &detected_lang)
        })
    } else {
        let m = NeuronManifest::new(&project_name, &target, &detected_lang);
        m.save(&target).await?;
        m
    };

    session::write_initial_context(&target, &manifest).await?;
    register(&manifest).await?;

    println!(
        "\n{} '{}' is powered up and registered.\n  {} Run {} to switch context.\n",
        "✅".green(),
        project_name.bold().bright_cyan(),
        "→".dimmed(),
        format!("neuron switch {}", project_name).yellow().bold()
    );

    Ok(())
}

/// Detect the most common source language in a directory by file-extension count.
async fn detect_primary_language(root: &Path) -> String {
    let mut counts: std::collections::HashMap<&'static str, usize> = std::collections::HashMap::new();

    let walk = ignore::WalkBuilder::new(root)
        .hidden(false)
        .git_ignore(true)
        .build();

    for entry in walk.flatten() {
        let path = entry.path().to_path_buf();
        if let Some(lang) = crate::parser::detect_language(&path) {
            *counts.entry(lang).or_default() += 1;
        }
    }

    counts
        .into_iter()
        .max_by_key(|(_, c)| *c)
        .map(|(lang, _)| lang.to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

/// Look up a project's root path by alias from the global registry.
pub async fn resolve_alias(alias: &str) -> Result<PathBuf> {
    let pool = open_global_db().await?;
    let row: Option<(String,)> = sqlx::query_as(
        "SELECT root_path FROM projects WHERE name = ?1 LIMIT 1"
    )
    .bind(alias)
    .fetch_optional(&pool)
    .await
    .context("Global index query failed during alias resolution")?;

    match row {
        Some((root_path,)) => Ok(PathBuf::from(root_path)),
        None => anyhow::bail!(
            "No project found with alias '{}'. Run {} to see all workspaces.",
            alias,
            "neuron list".cyan()
        ),
    }
}
