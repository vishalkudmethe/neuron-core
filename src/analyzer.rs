//! Cross-project signature mutation scanner and impact matrix generator.
//!
//! `neuron analyze --parent <alias>` computes a canonical signature hash for
//! every indexed symbol in the parent workspace, diffs against the
//! `signature_snapshots` table, then queries each registered child workspace's
//! FTS5 index to identify at-risk call sites.

use anyhow::Result;
use colored::Colorize;
use sha2::{Digest, Sha256};
use std::path::Path;
use tabled::{builder::Builder, settings::Style};

use crate::{dependency, search, utils};

// ─── Signature helpers ────────────────────────────────────────────────────────

/// Compute a stable signature hash for a symbol.
///
/// Hashing strategy per symbol kind:
///   - function / method : normalise the first-line declaration
///   - struct            : sort field lines, join, hash
///   - enum              : sort variant lines, join, hash
///   - other             : first 512 chars of snippet
fn compute_signature_hash(symbol_type: &str, snippet: &str) -> String {
    let canonical: String = match symbol_type {
        "function" | "method" => {
            // Extract the first non-comment, non-blank line (the signature line)
            snippet
                .lines()
                .map(|l| l.trim())
                .find(|l| !l.is_empty() && !l.starts_with("//") && !l.starts_with('#'))
                .unwrap_or("")
                .to_string()
        }
        "struct" => {
            // Collect field lines (lines containing ':'), sort them for stability
            let mut fields: Vec<&str> = snippet
                .lines()
                .map(|l| l.trim())
                .filter(|l| l.contains(':') && !l.starts_with("//"))
                .collect();
            fields.sort_unstable();
            fields.join(";")
        }
        "enum" => {
            // Collect non-blank, non-comment lines inside the braces
            let mut variants: Vec<&str> = snippet
                .lines()
                .map(|l| l.trim())
                .filter(|l| !l.is_empty() && !l.starts_with("//") && *l != "{" && *l != "}")
                .collect();
            variants.sort_unstable();
            variants.join(";")
        }
        _ => snippet.chars().take(512).collect(),
    };

    let mut hasher = Sha256::new();
    hasher.update(canonical.as_bytes());
    hex::encode(hasher.finalize())
}

// ─── Impact row ───────────────────────────────────────────────────────────────

struct ImpactRow {
    symbol_name:  String,
    symbol_type:  String,
    child_alias:  String,
    at_risk_file: String,
}

// ─── Public entry point ───────────────────────────────────────────────────────

/// Run the full mutation scan for `parent_alias` and print an impact matrix.
pub async fn analyze_parent(parent_alias: &str) -> Result<()> {
    println!(
        "\n{} Analyzing parent workspace: {}\n",
        "🔍".cyan(),
        parent_alias.bold().bright_cyan()
    );

    // ── 1. Resolve parent ─────────────────────────────────────────────────────
    let parent_root = crate::project_manager::resolve_alias(parent_alias).await?;
    let parent_id   = dependency::project_id_for_alias(parent_alias).await?;

    let parent_db = utils::local_db_path(&parent_root);
    if !parent_db.exists() {
        anyhow::bail!(
            "No index found for '{}'. Run `neuron power-up {}` first.",
            parent_alias, parent_alias
        );
    }

    // ── 2. Load all non-file symbols from parent ──────────────────────────────
    let parent_pool = search::open_local_db(&parent_db).await?;
    let symbols: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT symbol_name, symbol_type, content FROM memory_units \
         WHERE unit_type != 'file' AND symbol_name IS NOT NULL"
    )
    .fetch_all(&parent_pool)
    .await?;

    println!("  {} {} symbols loaded from parent index", "·".dimmed(), symbols.len());

    // ── 3. Hash each symbol, detect mutations ─────────────────────────────────
    let mut mutated: Vec<(String, String)> = vec![]; // (symbol_name, symbol_type)

    for (name, kind, snippet) in &symbols {
        let hash = compute_signature_hash(kind, snippet);
        let changed = dependency::upsert_signature(&parent_id, name, kind, &hash).await?;
        if changed {
            mutated.push((name.clone(), kind.clone()));
        }
    }

    println!(
        "  {} {} mutation(s) detected in parent",
        if mutated.is_empty() { "✓".green() } else { "⚠".yellow() },
        mutated.len()
    );

    if mutated.is_empty() {
        println!("\n{} No interface changes detected. All signatures stable.\n", "✅".green());
        return Ok(());
    }

    // ── 4. Resolve children ───────────────────────────────────────────────────
    let children = get_child_aliases(&parent_id).await?;

    println!(
        "  {} {} child workspace(s) to scan: {}\n",
        "·".dimmed(),
        children.len(),
        children.iter().map(|(a, _)| a.as_str()).collect::<Vec<_>>().join(", ")
    );

    // ── 5. For each mutated symbol, scan child FTS5 indexes ───────────────────
    let mut impact_rows: Vec<ImpactRow> = vec![];

    for (symbol_name, symbol_type) in &mutated {
        for (child_alias, child_root) in &children {
            let child_db = utils::local_db_path(Path::new(child_root));
            if !child_db.exists() {
                continue;
            }
            let child_pool = search::open_local_db(&child_db).await?;

            // FTS5 full-text match across all indexed content
            let hits: Vec<(String,)> = sqlx::query_as(
                "SELECT DISTINCT file_path FROM memory_units \
                 WHERE content LIKE ?1 AND unit_type = 'file'"
            )
            .bind(format!("%{}%", symbol_name))
            .fetch_all(&child_pool)
            .await
            .unwrap_or_default();

            for (file_path,) in hits {
                impact_rows.push(ImpactRow {
                    symbol_name: symbol_name.clone(),
                    symbol_type: symbol_type.clone(),
                    child_alias:  child_alias.clone(),
                    at_risk_file: file_path,
                });
            }
        }
    }

    // ── 6. Print impact matrix ────────────────────────────────────────────────
    println!(
        "{} IMPACT MATRIX — Parent: {} → {} child(ren) scanned\n",
        "⚡".bright_yellow().bold(),
        parent_alias.bright_cyan().bold(),
        children.len()
    );

    if impact_rows.is_empty() {
        println!("  {} No at-risk call sites found in registered child workspaces.\n", "✓".green());
        return Ok(());
    }

    let mut builder = Builder::default();
    builder.push_record(["Symbol", "Change", "Child Workspace", "At-Risk File"]);

    for row in &impact_rows {
        let change_label = format!("{} ↻", row.symbol_type);
        // Shorten the file path to last 2 segments for readability
        let short_path = shorten_path(&row.at_risk_file);
        builder.push_record([
            &row.symbol_name,
            &change_label,
            &row.child_alias,
            &short_path,
        ]);
    }

    println!("{}\n", builder.build().with(Style::rounded()).to_string());

    // Summary
    println!(
        "{} {} at-risk file(s) across {} child workspace(s). Review before deploying.\n",
        "⚠".bright_yellow().bold(),
        impact_rows.len(),
        children.len()
    );

    Ok(())
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

/// Get all child workspaces of a given parent project ID.
/// Returns `(alias, root_path)` pairs.
async fn get_child_aliases(parent_project_id: &str) -> Result<Vec<(String, String)>> {
    let db_path = utils::global_db_path()?;
    if !db_path.exists() {
        return Ok(vec![]);
    }

    let opts = sqlx::sqlite::SqliteConnectOptions::from_str(
        &format!("sqlite://{}", db_path.display())
    )?
    .create_if_missing(false);

    use std::str::FromStr;
    let pool = sqlx::SqlitePool::connect_with(opts).await?;

    let rows: Vec<(String, String)> = sqlx::query_as(
        "SELECT p.name, p.root_path FROM workspace_dependencies d \
         JOIN projects p ON p.id = d.child_id \
         WHERE d.parent_id = ?1"
    )
    .bind(parent_project_id)
    .fetch_all(&pool)
    .await
    .unwrap_or_default();

    Ok(rows)
}

fn shorten_path(path: &str) -> String {
    let parts: Vec<&str> = path.split(['/', '\\']).filter(|s| !s.is_empty()).collect();
    match parts.len() {
        0 => path.to_string(),
        1 => parts[0].to_string(),
        n => format!("{}/{}", parts[n - 2], parts[n - 1]),
    }
}
