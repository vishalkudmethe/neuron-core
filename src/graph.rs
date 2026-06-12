//! Terminal Topological Memory Graph Engine.
//!
//! Renders inter-project dependency networks in high-fidelity ASCII,
//! integrates focus hotspot states, and traces mutation propagation paths.

use anyhow::{Context, Result};
use chrono::{DateTime, Duration, Utc};
use colored::Colorize;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::str::FromStr;

use crate::{intent, search, utils};

// ─── Node Metadata ────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
struct GraphNode {
    id:            String,
    name:          String,
    _root_path:    String,
    language:      String,
    last_accessed: DateTime<Utc>,
    focus_level:   String, // "HIGH", "MEDIUM", "LOW"
}

// ─── Query Global DB ──────────────────────────────────────────────────────────

async fn open_global_db_pool() -> Result<sqlx::SqlitePool> {
    let db_path = utils::global_db_path()?;
    let opts = sqlx::sqlite::SqliteConnectOptions::from_str(&format!(
        "sqlite://{}", db_path.display()
    ))?
    .create_if_missing(false)
    .busy_timeout(std::time::Duration::from_millis(1500));
    sqlx::SqlitePool::connect_with(opts).await.context("Failed to open global database")
}

// ─── Graph Drawing ────────────────────────────────────────────────────────────

/// Renders the complete workspace dependency memory graph in ASCII.
pub async fn render_topology_graph() -> Result<()> {
    let pool = open_global_db_pool().await?;

    // 1. Fetch all projects
    let projects: Vec<(String, String, String, String, String)> = sqlx::query_as(
        "SELECT id, name, root_path, language, last_accessed FROM projects"
    )
    .fetch_all(&pool)
    .await?;

    if projects.is_empty() {
        println!(
            "\n{} No workspaces registered. Use {} to add projects.\n",
            "ℹ".cyan(),
            "neuron power-up <path> --alias <name>".yellow()
        );
        return Ok(());
    }

    // 2. Fetch all dependency links
    let links: Vec<(String, String)> = sqlx::query_as(
        "SELECT parent_id, child_id FROM workspace_dependencies"
    )
    .fetch_all(&pool)
    .await?;

    // 3. Process focus states for each project
    let mut nodes = HashMap::new();
    for (id, name, root_path, language, last_acc_str) in projects {
        let last_acc = DateTime::parse_from_rfc3339(&last_acc_str)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now());

        // Determine if there are active edits recently
        let mut focus = "LOW".to_string();
        let path = PathBuf::from(&root_path);
        if let Some(log) = intent::load_intent_log(&path).await {
            if log.entries.iter().any(|e| e.score >= 100) {
                focus = "HIGH".to_string();
            } else if log.entries.iter().any(|e| e.score >= 50) {
                focus = "MEDIUM".to_string();
            }
        } else {
            // Fallback to last accessed timestamp within 10 minutes
            let age = Utc::now().signed_duration_since(last_acc);
            if age < Duration::minutes(2) {
                focus = "HIGH".to_string();
            } else if age < Duration::minutes(10) {
                focus = "MEDIUM".to_string();
            }
        }

        nodes.insert(id.clone(), GraphNode {
            id,
            name,
            _root_path: root_path,
            language,
            last_accessed: last_acc,
            focus_level: focus,
        });
    }

    println!("\n{} NEURON TOPOLOGICAL MEMORY GRAPH\n", "◈".bright_cyan().bold());

    // 4. Determine roots (nodes with no parents in links)
    let children_ids: HashSet<&String> = links.iter().map(|(_, child)| child).collect();
    let mut roots: Vec<&GraphNode> = nodes.values().filter(|n| !children_ids.contains(&n.id)).collect();
    roots.sort_by_key(|n| &n.name);

    // Build children mappings
    let mut child_map: HashMap<&String, Vec<&String>> = HashMap::new();
    for (parent, child) in &links {
        child_map.entry(parent).or_default().push(child);
    }

    // 5. Draw DAG layers hierarchically
    let mut visited = HashSet::new();
    for root in &roots {
        draw_node_tree(root, &nodes, &child_map, 0, &mut visited, &mut vec![]);
    }

    println!();
    Ok(())
}

fn draw_node_tree<'a>(
    node: &'a GraphNode,
    nodes: &'a HashMap<String, GraphNode>,
    child_map: &HashMap<&String, Vec<&String>>,
    depth: usize,
    visited: &mut HashSet<&'a String>,
    pipe_structure: &mut Vec<bool>,
) {
    if visited.contains(&node.id) {
        return;
    }
    visited.insert(&node.id);

    // Render indentation prefixes
    let mut prefix = String::new();
    for i in 0..depth {
        if i == depth - 1 {
            prefix.push_str("├── ");
        } else if pipe_structure[i] {
            prefix.push_str("│   ");
        } else {
            prefix.push_str("    ");
        }
    }

    // Format display name based on focus status
    let age = Utc::now().signed_duration_since(node.last_accessed);
    let name_formatted = if node.focus_level == "HIGH" {
        format!("{} [ACTIVE FOCUS]", node.name).bold().cyan()
    } else if node.focus_level == "MEDIUM" {
        format!("{}", node.name).bold().yellow()
    } else if age > Duration::hours(48) {
        format!("{}", node.name).dimmed()
    } else {
        format!("{}", node.name).white()
    };

    let details = format!(
        "({} | {})",
        node.language,
        if age < Duration::minutes(1) {
            "just now".to_string()
        } else if age < Duration::hours(1) {
            format!("{}m ago", age.num_minutes())
        } else {
            format!("{}h ago", age.num_hours())
        }
    )
    .dimmed();

    println!("{}{} {}", prefix, name_formatted, details);

    // Recurse to children
    if let Some(children) = child_map.get(&node.id) {
        let mut sorted_children: Vec<&&String> = children.iter().collect();
        sorted_children.sort_by_key(|id| &nodes[**id].name);

        for (idx, child_id) in sorted_children.iter().enumerate() {
            if let Some(child_node) = nodes.get(**child_id) {
                let is_last = idx == sorted_children.len() - 1;
                pipe_structure.push(!is_last);
                draw_node_tree(child_node, nodes, child_map, depth + 1, visited, pipe_structure);
                pipe_structure.pop();
            }
        }
    }
}

// ─── Mutation Tracer ──────────────────────────────────────────────────────────

/// Traces a structural mutation on a symbol down into child dependency workspaces.
pub async fn trace_symbol_cascade(symbol_name: &str) -> Result<()> {
    let pool = open_global_db_pool().await?;

    // 1. Locate the symbol in signature snapshots to see where it was declared
    let snapshot: Option<(String, String, String, String)> = sqlx::query_as(
        "SELECT project_id, symbol_type, signature_hash, last_seen_at FROM signature_snapshots \
         WHERE symbol_name = ?1 LIMIT 1"
    )
    .bind(symbol_name)
    .fetch_optional(&pool)
    .await?;

    let (src_project_id, sym_type, current_hash, mutated_at_str) = match snapshot {
        Some(s) => s,
        None => {
            println!(
                "\n{} Symbol '{}' is not registered in any signature snapshots. Run `neuron analyze` on the parent workspace first.\n",
                "✗".red().bold(),
                symbol_name.yellow()
            );
            return Ok(());
        }
    };

    // Find parent project details
    let (parent_name, parent_root_str): (String, String) = sqlx::query_as(
        "SELECT name, root_path FROM projects WHERE id = ?1 LIMIT 1"
    )
    .bind(&src_project_id)
    .fetch_one(&pool)
    .await?;

    println!("\n{} CASCADING MUTATION TRACER", "⚡".bright_yellow().bold());
    println!("  {} `{}` ({})", "Symbol:".dimmed(), symbol_name.bold().cyan(), sym_type);
    println!("  {} {} ({})", "Source:".dimmed(), parent_name.bright_cyan(), parent_root_str.dimmed());
    println!("  {} `{}`", "Current Hash:".dimmed(), current_hash);
    println!("  {} {}", "Last Changed:".dimmed(), mutated_at_str);
    println!("\n{} Tracing usages across consumer workspaces...\n", "🔍".cyan());

    // 2. Fetch children of this parent project
    let children: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT p.id, p.name, p.root_path FROM workspace_dependencies d \
         JOIN projects p ON p.id = d.child_id \
         WHERE d.parent_id = ?1"
    )
    .bind(&src_project_id)
    .fetch_all(&pool)
    .await?;

    if children.is_empty() {
        println!("  {} No downstream consumer projects registered for '{}'.\n", "✓".green(), parent_name);
        return Ok(());
    }

    let mutated_at = DateTime::parse_from_rfc3339(&mutated_at_str)
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now());

    let mut at_least_one_hit = false;

    // 3. Scan children for usages
    for (_child_id, child_name, child_root_str) in children {
        let child_root = Path::new(&child_root_str);
        let child_db = utils::local_db_path(child_root);
        if !child_db.exists() {
            continue;
        }

        let child_pool = search::open_local_db(&child_db).await?;
        let hits: Vec<(String,)> = sqlx::query_as(
            "SELECT DISTINCT file_path FROM memory_units WHERE content LIKE ?1 AND unit_type = 'file'"
        )
        .bind(format!("%{}%", symbol_name))
        .fetch_all(&child_pool)
        .await
        .unwrap_or_default();

        if hits.is_empty() {
            continue;
        }

        at_least_one_hit = true;
        println!("  {} `{}`:", "Consumer Workspace".bold().yellow(), child_name);

        for (file_path,) in hits {
            let full_file_path = PathBuf::from(&file_path);
            let rel_path = full_file_path.strip_prefix(child_root).unwrap_or(&full_file_path).to_string_lossy().to_string();

            // Compare file modified time against mutation time
            let mtime = match tokio::fs::metadata(&full_file_path).await {
                Ok(m) => match m.modified() {
                    Ok(t) => DateTime::<Utc>::from(t),
                    Err(_) => Utc::now() - Duration::hours(24),
                },
                Err(_) => Utc::now() - Duration::hours(24),
            };

            if mtime < mutated_at {
                println!(
                    "    ├── {}  {} {}",
                    rel_path.red(),
                    "[⚠️ MUTATED]".red().bold(),
                    "(File out-of-date, modified before parent signature change)".dimmed()
                );
            } else {
                println!(
                    "    ├── {}  {} {}",
                    rel_path.green(),
                    "[✓ Synced]".green().bold(),
                    "(Modified after parent signature change)".dimmed()
                );
            }
        }
    }

    if !at_least_one_hit {
        println!("  {} No downstream usage of '{}' detected in children.\n", "✓".green(), symbol_name);
    } else {
        println!();
    }

    Ok(())
}

pub async fn get_trace_symbol_string(symbol_name: &str) -> Result<String> {
    let pool = open_global_db_pool().await?;

    // 1. Locate the symbol in signature snapshots to see where it was declared
    let snapshot: Option<(String, String, String, String)> = sqlx::query_as(
        "SELECT project_id, symbol_type, signature_hash, last_seen_at FROM signature_snapshots \
         WHERE symbol_name = ?1 LIMIT 1"
    )
    .bind(symbol_name)
    .fetch_optional(&pool)
    .await?;

    let (src_project_id, sym_type, current_hash, mutated_at_str) = match snapshot {
        Some(s) => s,
        None => {
            return Ok(format!(
                "Symbol '{}' is not registered in any signature snapshots. Run `neuron analyze` on the parent workspace first.",
                symbol_name
            ));
        }
    };

    // Find parent project details
    let (parent_name, parent_root_str): (String, String) = sqlx::query_as(
        "SELECT name, root_path FROM projects WHERE id = ?1 LIMIT 1"
    )
    .bind(&src_project_id)
    .fetch_one(&pool)
    .await?;

    let mut out = String::new();
    out.push_str("⚡ CASCADING MUTATION TRACER\n");
    out.push_str(&format!("  Symbol: {} ({})\n", symbol_name, sym_type));
    out.push_str(&format!("  Source: {} ({})\n", parent_name, parent_root_str));
    out.push_str(&format!("  Current Hash: {}\n", current_hash));
    out.push_str(&format!("  Last Changed: {}\n", mutated_at_str));
    out.push_str("\nTracing usages across consumer workspaces...\n\n");

    // 2. Fetch children of this parent project
    let children: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT p.id, p.name, p.root_path FROM workspace_dependencies d \
         JOIN projects p ON p.id = d.child_id \
         WHERE d.parent_id = ?1"
    )
    .bind(&src_project_id)
    .fetch_all(&pool)
    .await?;

    if children.is_empty() {
        out.push_str(&format!("  No downstream consumer projects registered for '{}'.\n", parent_name));
        return Ok(out);
    }

    let mutated_at = DateTime::parse_from_rfc3339(&mutated_at_str)
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now());

    let mut at_least_one_hit = false;

    // 3. Scan children for usages
    for (_child_id, child_name, child_root_str) in children {
        let child_root = Path::new(&child_root_str);
        let child_db = utils::local_db_path(child_root);
        if !child_db.exists() {
            continue;
        }

        let child_pool = search::open_local_db(&child_db).await?;
        let hits: Vec<(String,)> = sqlx::query_as(
            "SELECT DISTINCT file_path FROM memory_units WHERE content LIKE ?1 AND unit_type = 'file'"
        )
        .bind(format!("%{}%", symbol_name))
        .fetch_all(&child_pool)
        .await
        .unwrap_or_default();

        if hits.is_empty() {
            continue;
        }

        at_least_one_hit = true;
        out.push_str(&format!("  Consumer Workspace `{}`:\n", child_name));

        for (file_path,) in hits {
            let full_file_path = PathBuf::from(&file_path);
            let rel_path = full_file_path.strip_prefix(child_root).unwrap_or(&full_file_path).to_string_lossy().to_string();

            // Compare file modified time against mutation time
            let mtime = match tokio::fs::metadata(&full_file_path).await {
                Ok(m) => match m.modified() {
                    Ok(t) => DateTime::<Utc>::from(t),
                    Err(_) => Utc::now() - Duration::hours(24),
                },
                Err(_) => Utc::now() - Duration::hours(24),
            };

            if mtime < mutated_at {
                out.push_str(&format!(
                    "    ├── {}  [⚠️ MUTATED] (File out-of-date, modified before parent signature change)\n",
                    rel_path
                ));
            } else {
                out.push_str(&format!(
                    "    ├── {}  [✓ Synced] (Modified after parent signature change)\n",
                    rel_path
                ));
            }
        }
    }

    if !at_least_one_hit {
        out.push_str(&format!("  No downstream usage of '{}' detected in children.\n", symbol_name));
    }

    Ok(out)
}
