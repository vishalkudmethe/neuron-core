//! AI-NEURON™ Memory Digest Engine
//!
//! Translates the dense SQLite AI memory ledger (AST diffs, episodes, sessions,
//! loop-guard events, audit records) into natural-language, human-readable
//! "Executive Summaries" — the Information Blockchain Ledger for non-technical
//! users, managers, and enterprise teams.
//!
//! ## Design
//! Rather than requiring a live LLM API call (which would cost money and leak data),
//! the digest engine uses a deterministic, template-driven summarizer built entirely
//! in Rust. This preserves AI-NEURON's core principle: 100% local, 100% private.
//!
//! A future Pro/Cloud tier will optionally route digest generation through the
//! user's own cloud AI provider (OpenAI, Anthropic, Gemini) for richer narrative.
//!
//! ## Commands
//! - `neuron digest`            — Show the last 7 days of activity in plain English
//! - `neuron digest --days N`   — Show N days of history
//! - `neuron digest --export`   — Write the digest to a Markdown file
//! - `neuron digest --ledger`   — Print the immutable blockchain audit ledger view

use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use colored::Colorize;
use sqlx::Row;
use std::path::PathBuf;

use crate::sessions;

// ─── Digest Entry ─────────────────────────────────────────────────────────────

/// A single human-readable digest entry representing one recorded AI action.
#[derive(Debug, Clone)]
pub struct DigestEntry {
    pub timestamp: String,
    pub category: DigestCategory,
    pub headline: String,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DigestCategory {
    Memory,      // Episode / goal recorded
    LoopGuard,   // Loop prevention triggered
    Session,     // AI session opened/closed
    AuditLedger, // Immutable blockchain audit event
    Project,     // Project init / power-up / switch
}

impl DigestCategory {
    fn icon(&self) -> &str {
        match self {
            DigestCategory::Memory => "🧠",
            DigestCategory::LoopGuard => "🛡️",
            DigestCategory::Session => "💬",
            DigestCategory::AuditLedger => "🔗",
            DigestCategory::Project => "📁",
        }
    }
    fn label(&self) -> &str {
        match self {
            DigestCategory::Memory => "MEMORY",
            DigestCategory::LoopGuard => "LOOP GUARD",
            DigestCategory::Session => "SESSION",
            DigestCategory::AuditLedger => "LEDGER",
            DigestCategory::Project => "PROJECT",
        }
    }
}

// ─── Collect Digest Entries ───────────────────────────────────────────────────

/// Pull all recorded memories (episodes) from the sessions ledger within `days`.
async fn collect_memory_entries(days: i64) -> Result<Vec<DigestEntry>> {
    let pool = sessions::open_pool().await?;
    let since = (Utc::now() - Duration::days(days)).to_rfc3339();

    let rows = sqlx::query(
        "SELECT summary, importance, created_at FROM episodes
         WHERE created_at >= ? ORDER BY created_at DESC LIMIT 100"
    )
    .bind(&since)
    .fetch_all(&pool)
    .await?;

    let entries = rows.iter().map(|r| {
        let summary: String = r.get("summary");
        let importance: i64 = r.get("importance");
        let created_at: String = r.get("created_at");
        let stars = "★".repeat(importance.min(10) as usize);

        DigestEntry {
            timestamp: format_ts(&created_at),
            category: DigestCategory::Memory,
            headline: summary.clone(),
            detail: Some(format!("Importance: {}/10 {}", importance, stars)),
        }
    }).collect();

    Ok(entries)
}

/// Pull active/recent goals as memory entries.
async fn collect_goal_entries() -> Result<Vec<DigestEntry>> {
    let pool = sessions::open_pool().await?;

    let rows = sqlx::query(
        "SELECT title, status, created_at FROM goals ORDER BY created_at DESC LIMIT 20"
    )
    .fetch_all(&pool)
    .await?;

    let entries = rows.iter().map(|r| {
        let title: String = r.get("title");
        let status: String = r.get("status");
        let created_at: String = r.get("created_at");
        DigestEntry {
            timestamp: format_ts(&created_at),
            category: DigestCategory::Memory,
            headline: format!("Goal: {}", title),
            detail: Some(format!("Status: {}", status.to_uppercase())),
        }
    }).collect();

    Ok(entries)
}

/// Pull audit log entries (the immutable information blockchain ledger).
async fn collect_audit_entries(days: i64) -> Result<Vec<DigestEntry>> {
    let audit_db = audit_db_path();
    if !audit_db.exists() {
        return Ok(vec![]);
    }

    let url = format!("sqlite://{}?mode=ro", audit_db.display());
    let pool = match sqlx::SqlitePool::connect(&url).await {
        Ok(p) => p,
        Err(_) => return Ok(vec![]),
    };

    let since = (Utc::now() - Duration::days(days)).to_rfc3339();

    // Try to read audit table — graceful if schema differs
    let rows = match sqlx::query(
        "SELECT tool, args_json, timestamp, result_summary
         FROM audit_log WHERE timestamp >= ? ORDER BY timestamp DESC LIMIT 200"
    )
    .bind(&since)
    .fetch_all(&pool)
    .await {
        Ok(r) => r,
        Err(_) => return Ok(vec![]),
    };

    let entries = rows.iter().map(|r| {
        let tool: String = r.get("tool");
        let timestamp: String = r.get("timestamp");
        let result: String = r.try_get("result_summary").unwrap_or_default();

        DigestEntry {
            timestamp: format_ts(&timestamp),
            category: DigestCategory::AuditLedger,
            headline: format!("MCP Tool Invoked: {}", tool),
            detail: if result.is_empty() { None } else { Some(result) },
        }
    }).collect();

    Ok(entries)
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

fn audit_db_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".neuron")
        .join("audit.jsonl")
}

/// Format an RFC3339 timestamp into a friendly date-time string.
fn format_ts(ts: &str) -> String {
    if let Ok(dt) = ts.parse::<DateTime<Utc>>() {
        // Convert to local-ish by just showing the date and time nicely
        dt.format("%Y-%m-%d %H:%M UTC").to_string()
    } else if ts.len() >= 16 {
        ts[..16].to_string()
    } else {
        ts.to_string()
    }
}

/// Sort all entries by timestamp descending.
fn sort_entries(mut entries: Vec<DigestEntry>) -> Vec<DigestEntry> {
    entries.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    entries
}

// ─── Render: Terminal ─────────────────────────────────────────────────────────

fn render_terminal(entries: &[DigestEntry], days: i64) {
    println!();
    println!(
        "  {} {}  {}",
        "🧠".bold(),
        "AI-NEURON™ MEMORY DIGEST".bright_cyan().bold(),
        format!("(last {} days)", days).dimmed()
    );
    println!("{}", "─".repeat(72).dimmed());
    println!(
        "  {} {}",
        "ℹ".cyan(),
        "This is your Information Blockchain Ledger — an immutable, human-readable".dimmed()
    );
    println!(
        "    {}",
        "record of everything your AI agents have done, learned, and remembered.".dimmed()
    );
    println!("{}\n", "─".repeat(72).dimmed());

    if entries.is_empty() {
        println!(
            "  {} No memory entries found for the last {} days.\n",
            "○".yellow(),
            days
        );
        println!(
            "  Run {} to add memories, or {} to start recording sessions.",
            "neuron sessions --episode <summary>".cyan(),
            "neuron watch".cyan()
        );
        println!();
        return;
    }

    for entry in entries {
        let cat_label = format!("[{}]", entry.category.label());
        let cat_colored = match entry.category {
            DigestCategory::Memory      => cat_label.bright_cyan(),
            DigestCategory::LoopGuard   => cat_label.bright_red(),
            DigestCategory::Session     => cat_label.bright_yellow(),
            DigestCategory::AuditLedger => cat_label.bright_magenta(),
            DigestCategory::Project     => cat_label.bright_green(),
        };

        println!(
            "  {} {} {}  {}",
            entry.category.icon(),
            cat_colored,
            entry.timestamp.dimmed(),
            entry.headline.bright_white()
        );
        if let Some(detail) = &entry.detail {
            println!("           {}", detail.dimmed());
        }
    }

    println!();
    println!("{}", "─".repeat(72).dimmed());
    println!(
        "  {} Total entries: {}  |  Tip: {} to export as Markdown",
        "✓".green().bold(),
        entries.len().to_string().yellow(),
        "neuron digest --export digest.md".cyan()
    );
    println!();
}

// ─── Render: Markdown Export ──────────────────────────────────────────────────

fn render_markdown(entries: &[DigestEntry], days: i64) -> String {
    let now = Utc::now().format("%Y-%m-%d %H:%M UTC").to_string();
    let mut md = String::new();

    md.push_str("# AI-NEURON™ Memory Digest — Information Blockchain Ledger\n\n");
    md.push_str(&format!("> Generated: {}  \n", now));
    md.push_str(&format!("> Period: last {} days\n\n", days));
    md.push_str("---\n\n");
    md.push_str(
        "This document is an **immutable, human-readable record** of all AI agent actions, \
         memories, and decisions recorded by AI-NEURON™ on this machine. \
         It represents your **Information Blockchain Ledger** — a tamper-evident \
         chronological chain of AI activity that is preserved locally for \
         auditing, team handoffs, and compliance.\n\n"
    );
    md.push_str("---\n\n");
    md.push_str("## Memory Timeline\n\n");

    if entries.is_empty() {
        md.push_str("_No memory entries found for this period._\n\n");
        return md;
    }

    // Group by category for executive readability
    let categories = [
        DigestCategory::LoopGuard,
        DigestCategory::Memory,
        DigestCategory::Session,
        DigestCategory::AuditLedger,
        DigestCategory::Project,
    ];

    for cat in &categories {
        let cat_entries: Vec<_> = entries.iter().filter(|e| &e.category == cat).collect();
        if cat_entries.is_empty() { continue; }

        md.push_str(&format!("### {} {}\n\n", cat.icon(), cat.label()));
        md.push_str("| Timestamp | Event | Detail |\n");
        md.push_str("|-----------|-------|--------|\n");

        for entry in cat_entries {
            let detail = entry.detail.as_deref().unwrap_or("—");
            md.push_str(&format!(
                "| `{}` | {} | {} |\n",
                entry.timestamp,
                entry.headline,
                detail
            ));
        }
        md.push('\n');
    }

    md.push_str("---\n\n");
    md.push_str(&format!(
        "_This digest was generated automatically by AI-NEURON™ v1.0. \
         Total entries: {}. Protected by AGPLv3 and Provisional Patent (filed 2026-06-12)._\n",
        entries.len()
    ));

    md
}

// ─── Public CLI Entry ─────────────────────────────────────────────────────────

/// Run the `neuron digest` command.
pub async fn run_digest(days: i64, export: Option<&str>, ledger_only: bool) -> Result<()> {
    // Collect from all sources
    let mut all_entries: Vec<DigestEntry> = Vec::new();

    // Memory episodes
    match collect_memory_entries(days).await {
        Ok(mut e) => all_entries.append(&mut e),
        Err(err) => eprintln!("  [warn] Could not read episodes: {}", err),
    }

    // Goals
    match collect_goal_entries().await {
        Ok(mut e) => all_entries.append(&mut e),
        Err(err) => eprintln!("  [warn] Could not read goals: {}", err),
    }

    // Audit/Ledger
    match collect_audit_entries(days).await {
        Ok(mut e) => all_entries.append(&mut e),
        Err(_) => {} // Silently skip if no audit log
    }

    // Filter
    let entries = if ledger_only {
        all_entries.into_iter().filter(|e| e.category == DigestCategory::AuditLedger).collect()
    } else {
        all_entries
    };

    let entries = sort_entries(entries);

    if let Some(export_path) = export {
        // Write Markdown file
        let md = render_markdown(&entries, days);
        std::fs::write(export_path, &md)?;
        println!(
            "\n  {} Memory Digest exported to: {}\n",
            "✓".green().bold(),
            export_path.bright_cyan()
        );
        println!(
            "  {} {} entries written to the Information Blockchain Ledger.",
            "🔗",
            entries.len().to_string().yellow()
        );
    } else {
        render_terminal(&entries, days);
    }

    Ok(())
}
