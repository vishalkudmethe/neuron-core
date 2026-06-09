//! AI-Neuron Sessions™ — Personal AI Memory Daemon (v0.1 — Identity Ledger)
//!
//! This module solves the fundamental LLM personalization problem:
//!   - Session amnesia (every tab starts from zero)
//!   - Cross-tab context contamination
//!   - Cold-start personalization delay
//!
//! Architecture: A persistent SQLite ledger (`~/.neuron/sessions.db`) stores
//! a structured behavioral profile, episodic memory, and active session map.
//! At any new LLM session start, `get_context_block()` returns a token-efficient
//! (~2000 token max) context injection string ready for any MCP-compatible LLM.
//!
//! This is AI-Neuron Sessions™ v0.1 — the Identity Ledger foundation.
//! v0.2 will expose this via an MCP `get_user_context` tool.

use anyhow::Result;
use chrono::Utc;
use colored::Colorize;
use serde::{Deserialize, Serialize};
use sqlx::{Row, SqlitePool};
use std::path::PathBuf;
use uuid::Uuid;

// ─── Schema ───────────────────────────────────────────────────────────────────

/// DDL executed on first run to create the sessions database.
const SESSIONS_SCHEMA: &str = r#"
-- Behavioral profile: key/value store for persistent user signals.
-- Examples: expertise_rust=expert, style=direct, response_length=concise
CREATE TABLE IF NOT EXISTS profile (
    key         TEXT PRIMARY KEY,
    value       TEXT NOT NULL,
    updated_at  TEXT NOT NULL
);

-- Episodic memory: significant events the user has stated or that Neuron observed.
-- e.g., "Solved production Redis timeout outage on 2026-06-01"
CREATE TABLE IF NOT EXISTS episodes (
    id          TEXT PRIMARY KEY,
    summary     TEXT NOT NULL,
    tags        TEXT NOT NULL DEFAULT '[]',   -- JSON array of topic tags
    importance  INTEGER NOT NULL DEFAULT 5,   -- 1 (low) to 10 (critical)
    created_at  TEXT NOT NULL
);

-- Active session map: one row per open LLM tab / context window.
-- Tracks what each tab is currently discussing for cross-tab coherence.
CREATE TABLE IF NOT EXISTS active_sessions (
    tab_id      TEXT PRIMARY KEY,
    topic       TEXT NOT NULL,
    llm         TEXT NOT NULL DEFAULT 'unknown',  -- e.g., gemini, claude, gpt
    started_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL
);

-- Working goals: current user objectives (short-term, mission-critical items).
CREATE TABLE IF NOT EXISTS goals (
    id          TEXT PRIMARY KEY,
    title       TEXT NOT NULL,
    status      TEXT NOT NULL DEFAULT 'active',  -- active | paused | done
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL
);

-- Projects cross-reference: link sessions to known Neuron Core projects.
CREATE TABLE IF NOT EXISTS session_projects (
    project_name    TEXT NOT NULL,
    root_path       TEXT NOT NULL,
    last_active_at  TEXT NOT NULL,
    PRIMARY KEY (project_name)
);

-- Enterprise Corporate Hubs (Master Brains mapped to this Child Brain)
CREATE TABLE IF NOT EXISTS corporate_hubs (
    id            TEXT PRIMARY KEY,
    domain        TEXT NOT NULL,
    endpoint_url  TEXT NOT NULL,
    access_token  TEXT NOT NULL,
    active        INTEGER NOT NULL DEFAULT 1,
    created_at    TEXT NOT NULL
);

-- Synced memories and sanitized context milestones contributed to Corporate Hub
CREATE TABLE IF NOT EXISTS synced_memories (
    id                TEXT PRIMARY KEY,
    local_episode_id  TEXT,
    title             TEXT NOT NULL,
    content           TEXT NOT NULL,
    synced_at         TEXT NOT NULL,
    author            TEXT NOT NULL
);
-- FTS5 index for fast episode search
CREATE VIRTUAL TABLE IF NOT EXISTS episodes_fts USING fts5(
    id UNINDEXED, summary, tags,
    content='episodes', content_rowid='rowid'
);
"#;

// ─── DB Path ──────────────────────────────────────────────────────────────────

pub fn sessions_db_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".neuron")
        .join("sessions.db")
}

// ─── Pool ─────────────────────────────────────────────────────────────────────

pub async fn open_pool() -> Result<SqlitePool> {
    let path = sessions_db_path();
    if let Some(p) = path.parent() {
        std::fs::create_dir_all(p)?;
    }
    let url = format!("sqlite://{}?mode=rwc", path.display());
    let pool = SqlitePool::connect(&url).await?;
    sqlx::query(SESSIONS_SCHEMA).execute(&pool).await?;
    Ok(pool)
}

// ─── Profile Operations ───────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
pub struct ProfileEntry {
    pub key: String,
    pub value: String,
    pub updated_at: String,
}

/// Upsert a single profile key/value pair.
pub async fn set_profile(pool: &SqlitePool, key: &str, value: &str) -> Result<()> {
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO profile (key, value, updated_at) VALUES (?, ?, ?)
         ON CONFLICT(key) DO UPDATE SET value=excluded.value, updated_at=excluded.updated_at"
    )
    .bind(key)
    .bind(value)
    .bind(&now)
    .execute(pool)
    .await?;
    Ok(())
}

/// Read all profile entries.
pub async fn get_profile(pool: &SqlitePool) -> Result<Vec<ProfileEntry>> {
    let rows = sqlx::query("SELECT key, value, updated_at FROM profile ORDER BY key")
        .fetch_all(pool)
        .await?;
    Ok(rows
        .iter()
        .map(|r| ProfileEntry {
            key: r.get("key"),
            value: r.get("value"),
            updated_at: r.get("updated_at"),
        })
        .collect())
}

// ─── Episode Operations ───────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
pub struct Episode {
    pub id: String,
    pub summary: String,
    pub tags: Vec<String>,
    pub importance: i64,
    pub created_at: String,
}

/// Add a new episodic memory event.
pub async fn add_episode(
    pool: &SqlitePool,
    summary: &str,
    tags: &[String],
    importance: i64,
) -> Result<String> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    let tags_json = serde_json::to_string(tags)?;
    sqlx::query(
        "INSERT INTO episodes (id, summary, tags, importance, created_at)
         VALUES (?, ?, ?, ?, ?)"
    )
    .bind(&id)
    .bind(summary)
    .bind(&tags_json)
    .bind(importance)
    .bind(&now)
    .execute(pool)
    .await?;
    // Keep FTS index in sync
    sqlx::query("INSERT INTO episodes_fts(rowid, id, summary, tags) SELECT rowid, id, summary, tags FROM episodes WHERE id=?")
        .bind(&id)
        .execute(pool)
        .await
        .ok(); // non-fatal
    Ok(id)
}

/// Retrieve top N episodes ordered by importance descending.
pub async fn get_top_episodes(pool: &SqlitePool, limit: i64) -> Result<Vec<Episode>> {
    let rows = sqlx::query(
        "SELECT id, summary, tags, importance, created_at
         FROM episodes ORDER BY importance DESC, created_at DESC LIMIT ?"
    )
    .bind(limit)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .iter()
        .map(|r| {
            let tags_str: String = r.get("tags");
            let tags: Vec<String> =
                serde_json::from_str(&tags_str).unwrap_or_default();
            Episode {
                id: r.get("id"),
                summary: r.get("summary"),
                tags,
                importance: r.get("importance"),
                created_at: r.get("created_at"),
            }
        })
        .collect())
}

// ─── Active Session Map ───────────────────────────────────────────────────────

/// Register or update an active LLM tab session.
pub async fn upsert_session(
    pool: &SqlitePool,
    tab_id: &str,
    topic: &str,
    llm: &str,
) -> Result<()> {
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO active_sessions (tab_id, topic, llm, started_at, updated_at)
         VALUES (?, ?, ?, ?, ?)
         ON CONFLICT(tab_id) DO UPDATE SET topic=excluded.topic, updated_at=excluded.updated_at"
    )
    .bind(tab_id)
    .bind(topic)
    .bind(llm)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await?;
    Ok(())
}

/// Remove a closed tab from the active session map.
pub async fn close_session(pool: &SqlitePool, tab_id: &str) -> Result<()> {
    sqlx::query("DELETE FROM active_sessions WHERE tab_id=?")
        .bind(tab_id)
        .execute(pool)
        .await?;
    Ok(())
}

// ─── Goals ────────────────────────────────────────────────────────────────────

pub async fn add_goal(pool: &SqlitePool, title: &str) -> Result<String> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO goals (id, title, status, created_at, updated_at) VALUES (?, ?, 'active', ?, ?)"
    )
    .bind(&id)
    .bind(title)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await?;
    Ok(id)
}

pub async fn get_active_goals(pool: &SqlitePool) -> Result<Vec<String>> {
    let rows =
        sqlx::query("SELECT title FROM goals WHERE status='active' ORDER BY created_at DESC LIMIT 5")
            .fetch_all(pool)
            .await?;
    Ok(rows.iter().map(|r| r.get("title")).collect())
}

// ─── Context Injection Block ──────────────────────────────────────────────────

/// Generates the structured context injection string that is prepended to any
/// new LLM session prompt. Token-efficient (~2000 tokens max by design).
///
/// Output format (plain text, MCP-injectable):
/// ```
/// [AI-NEURON SESSIONS™ CONTEXT BLOCK]
/// USER_PROFILE: expertise_rust=expert | style=direct | lang=English
/// ACTIVE_TABS: [Tab A: Rust debugging] [Tab B: THIS SESSION]
/// CURRENT_GOALS: Launch AI-NEURON, achieve 5000 GitHub stars
/// RECENT_EPISODES: Solved production outage (2026-06-01) | Deployed ai-neuron.org (2026-06-08)
/// INSTRUCTION: Do not repeat explanations already given. User knows these topics deeply.
/// [END CONTEXT BLOCK]
/// ```
pub async fn get_context_block(pool: &SqlitePool, this_tab_id: &str) -> Result<String> {
    // 1. Profile
    let profile = get_profile(pool).await?;
    let profile_str = profile
        .iter()
        .map(|e| format!("{}={}", e.key, e.value))
        .collect::<Vec<_>>()
        .join(" | ");

    // 2. Active tabs (excluding this one)
    let tab_rows = sqlx::query("SELECT tab_id, topic, llm FROM active_sessions ORDER BY updated_at DESC")
        .fetch_all(pool)
        .await?;
    let tabs_str = tab_rows
        .iter()
        .map(|r| {
            let tid: String = r.get("tab_id");
            let topic: String = r.get("topic");
            if tid == this_tab_id {
                format!("[{tid}: THIS SESSION]")
            } else {
                format!("[{tid}: {topic}]")
            }
        })
        .collect::<Vec<_>>()
        .join(" ");

    // 3. Goals
    let goals = get_active_goals(pool).await?;
    let goals_str = goals.join(", ");

    // 4. Top episodes (top 5 by importance)
    let episodes = get_top_episodes(pool, 5).await?;
    let ep_str = episodes
        .iter()
        .map(|e| {
            let date = if e.created_at.len() >= 10 { &e.created_at[..10] } else { &e.created_at };
            format!("{} ({})", e.summary, date)
        })
        .collect::<Vec<_>>()
        .join(" | ");

    let block = format!(
        "[AI-NEURON SESSIONS™ CONTEXT BLOCK]\n\
         USER_PROFILE: {profile_str}\n\
         ACTIVE_TABS: {tabs_str}\n\
         CURRENT_GOALS: {goals_str}\n\
         RECENT_EPISODES: {ep_str}\n\
         INSTRUCTION: Do not repeat prior explanations. Adapt to the user's stated expertise and style.\n\
         [END CONTEXT BLOCK]"
    );

    Ok(block)
}

// ─── CLI: neuron sessions ─────────────────────────────────────────────────────

#[derive(Debug)]
pub enum SessionsCmd {
    Init,
    SetProfile { key: String, value: String },
    AddEpisode { summary: String, importance: i64 },
    AddGoal { title: String },
    Context { tab: String },
    LogSession { tab: String, topic: String, llm: String },
    CloseSession { tab: String },
    Show,
}

pub async fn run_sessions_cli(cmd: SessionsCmd) -> Result<()> {
    let pool = open_pool().await?;

    match cmd {
        SessionsCmd::Init => {
            println!(
                "{} AI-Neuron Sessions™ identity ledger initialized at {}",
                "✓".green().bold(),
                sessions_db_path().display().to_string().cyan()
            );
        }

        SessionsCmd::SetProfile { key, value } => {
            set_profile(&pool, &key, &value).await?;
            println!(
                "{} Profile updated: {} = {}",
                "✓".green().bold(),
                key.bright_cyan(),
                value.yellow()
            );
        }

        SessionsCmd::AddEpisode { summary, importance } => {
            let id = add_episode(&pool, &summary, &[], importance).await?;
            println!(
                "{} Episode recorded [importance {}/10]: {}",
                "✓".green().bold(),
                importance.to_string().yellow(),
                id.dimmed()
            );
        }

        SessionsCmd::AddGoal { title } => {
            let id = add_goal(&pool, &title).await?;
            println!(
                "{} Goal added: {} [{}]",
                "✓".green().bold(),
                title.bright_cyan(),
                id.dimmed()
            );
        }

        SessionsCmd::Context { tab } => {
            let block = get_context_block(&pool, &tab).await?;
            println!("\n{}\n", block.bright_cyan());
        }

        SessionsCmd::LogSession { tab, topic, llm } => {
            upsert_session(&pool, &tab, &topic, &llm).await?;
            println!(
                "{} Session logged: [{}] {} on {}",
                "✓".green().bold(),
                tab.yellow(),
                topic.bright_cyan(),
                llm.dimmed()
            );
        }

        SessionsCmd::CloseSession { tab } => {
            close_session(&pool, &tab).await?;
            println!("{} Session closed: [{}]", "✓".green().bold(), tab.yellow());
        }

        SessionsCmd::Show => {
            println!(
                "\n  {} {}\n",
                "AI-NEURON SESSIONS™".bright_cyan().bold(),
                "Identity Ledger".white()
            );

            // Profile
            let profile = get_profile(&pool).await?;
            println!("  {} Profile Fields:", "▸".cyan());
            if profile.is_empty() {
                println!("    (none — use `neuron sessions set-profile <key> <value>`)");
            } else {
                for e in &profile {
                    println!("    {} = {}", e.key.bright_white(), e.value.yellow());
                }
            }

            // Goals
            let goals = get_active_goals(&pool).await?;
            println!("\n  {} Active Goals:", "▸".cyan());
            if goals.is_empty() {
                println!("    (none — use `neuron sessions add-goal <title>`)");
            } else {
                for g in &goals {
                    println!("    • {}", g.bright_white());
                }
            }

            // Episodes
            let episodes = get_top_episodes(&pool, 5).await?;
            println!("\n  {} Top Episodes:", "▸".cyan());
            if episodes.is_empty() {
                println!("    (none — use `neuron sessions add-episode <summary>`)");
            } else {
                for ep in &episodes {
                    println!(
                        "    [{}] {} {}",
                        ep.importance.to_string().yellow(),
                        ep.summary.bright_white(),
                        ep.created_at[..10].to_string().dimmed()
                    );
                }
            }

            // Active sessions
            let tab_rows = sqlx::query(
                "SELECT tab_id, topic, llm, updated_at FROM active_sessions ORDER BY updated_at DESC"
            )
            .fetch_all(&pool)
            .await?;
            println!("\n  {} Active LLM Sessions:", "▸".cyan());
            if tab_rows.is_empty() {
                println!("    (none — use `neuron sessions log <tab_id> <topic> <llm>`)");
            } else {
                for r in &tab_rows {
                    let tid: String = r.get("tab_id");
                    let topic: String = r.get("topic");
                    let llm: String = r.get("llm");
                    println!("    [{}] {} on {}", tid.yellow(), topic.bright_cyan(), llm.dimmed());
                }
            }

            println!();
        }
    }

    Ok(())
}
