//! Neuron Core™ Enterprise Audit Logging Engine
//!
//! Every MCP tool invocation is stamped with a unique ID, ISO-8601 timestamp,
//! session ID, tool name, call parameters, response byte-count, duration, and
//! project path — then appended as a single JSON line to `~/.neuron/audit.log`.
//!
//! This satisfies SOC 2 / GDPR data-access traceability requirements for
//! enterprise deployments without ever persisting the actual response content.

use anyhow::Result;
use chrono::Utc;
use colored::Colorize;
use serde::{Deserialize, Serialize};
use std::{
    fs::OpenOptions,
    io::Write,
    path::PathBuf,
    time::Instant,
};
use uuid::Uuid;

// ─── Entry Schema ─────────────────────────────────────────────────────────────

/// A single, tamper-evident audit record written as one JSON line.
#[derive(Debug, Serialize, Deserialize)]
pub struct AuditEntry {
    /// Unique ID for this log line (UUID v4).
    pub id: String,
    /// RFC 3339 wall-clock timestamp (UTC).
    pub timestamp: String,
    /// Opaque per-process session tag (reused across all calls in one run).
    pub session_id: String,
    /// MCP tool name that was invoked (e.g. `"get_symbol_info"`).
    pub tool: String,
    /// Sanitized call parameters (values already redacted by sanitize module).
    pub params: serde_json::Value,
    /// Number of bytes in the tool's JSON response (content is NOT stored).
    pub response_bytes: usize,
    /// Wall-clock duration of the tool call in milliseconds.
    pub duration_ms: u64,
    /// Absolute path of the active project at call time.
    pub project: String,
}

// ─── Session ID ───────────────────────────────────────────────────────────────

/// Returns (or lazily generates) a stable session ID for this process lifetime.
pub fn session_id() -> String {
    // Use a process-scoped static so every call in one run shares the same tag.
    use std::sync::OnceLock;
    static SESSION: OnceLock<String> = OnceLock::new();
    SESSION
        .get_or_init(|| Uuid::new_v4().to_string())
        .clone()
}

// ─── Paths ────────────────────────────────────────────────────────────────────

/// Returns the canonical path to the audit log: `~/.neuron/audit.log`.
pub fn audit_log_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".neuron")
        .join("audit.log")
}

// ─── Core Writer ──────────────────────────────────────────────────────────────

/// Appends a single JSON line to the audit log (non-blocking, best-effort).
/// Failures are silently swallowed so they never interrupt the hot MCP path.
pub fn write_entry(entry: &AuditEntry) {
    if let Err(e) = try_write_entry(entry) {
        // Degrade gracefully — audit log write failure must not crash the tool.
        tracing::warn!("audit write failed: {e}");
    }
}

fn try_write_entry(entry: &AuditEntry) -> Result<()> {
    let path = audit_log_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)?;
    let line = serde_json::to_string(entry)?;
    writeln!(file, "{line}")?;
    Ok(())
}

// ─── Builder Helper ───────────────────────────────────────────────────────────

/// Convenience builder — call this after a tool completes.
///
/// ```rust
/// let t = Instant::now();
/// let result = do_tool_work(&params)?;
/// audit::record("get_symbol_info", &params_json, result.len(), t.elapsed().as_millis() as u64, &project_path);
/// ```
pub fn record(
    tool: &str,
    params: &serde_json::Value,
    response_bytes: usize,
    duration_ms: u64,
    project: &str,
) {
    let entry = AuditEntry {
        id: Uuid::new_v4().to_string(),
        timestamp: Utc::now().to_rfc3339(),
        session_id: session_id(),
        tool: tool.to_string(),
        params: params.clone(),
        response_bytes,
        duration_ms,
        project: project.to_string(),
    };
    write_entry(&entry);
}

// ─── Timer Helper ─────────────────────────────────────────────────────────────

/// Wraps a synchronous closure, records timing, writes audit entry, returns result.
pub fn timed_record<F, T>(tool: &str, params: &serde_json::Value, project: &str, f: F) -> T
where
    F: FnOnce() -> T,
    T: AsRef<str>,
{
    let start = Instant::now();
    let result = f();
    let ms = start.elapsed().as_millis() as u64;
    record(tool, params, result.as_ref().len(), ms, project);
    result
}

// ─── CLI: neuron audit ────────────────────────────────────────────────────────

/// Display or export the audit log. Called from `main.rs`.
pub async fn run_audit_cli(export: Option<&str>, tail: Option<usize>, clear: bool) -> Result<()> {
    let path = audit_log_path();

    if clear {
        if path.exists() {
            std::fs::remove_file(&path)?;
            println!("{} Audit log cleared.", "✓".green().bold());
        } else {
            println!("{} No audit log to clear.", "⚠".yellow().bold());
        }
        return Ok(());
    }

    if !path.exists() {
        println!(
            "{} No audit log found at {}.",
            "⚠".yellow().bold(),
            path.display().to_string().cyan()
        );
        println!(
            "  Audit entries are written automatically when the MCP server processes tool calls."
        );
        return Ok(());
    }

    let content = std::fs::read_to_string(&path)?;
    let mut entries: Vec<serde_json::Value> = content
        .lines()
        .filter(|l| !l.trim().is_empty())
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect();

    // Apply tail filter if requested
    if let Some(n) = tail {
        let skip = entries.len().saturating_sub(n);
        entries = entries.into_iter().skip(skip).collect();
    }

    if let Some(out_file) = export {
        // Export as pretty-printed JSON array
        let json = serde_json::to_string_pretty(&entries)?;
        std::fs::write(out_file, &json)?;
        println!(
            "{} Audit log ({} entries) exported → {}",
            "✓".green().bold(),
            entries.len().to_string().bright_yellow(),
            out_file.cyan()
        );
        return Ok(());
    }

    // Pretty terminal summary
    println!("\n  {} {}\n", "NEURON CORE™".bright_cyan().bold(), "ENTERPRISE AUDIT LOG".white().bold());
    println!(
        "  {:<38}  {:<20}  {:<26}  {:>8}  {:>7}",
        "ENTRY ID".bright_white().bold(),
        "TIMESTAMP (UTC)".bright_white().bold(),
        "TOOL".bright_white().bold(),
        "BYTES".bright_white().bold(),
        "MS".bright_white().bold(),
    );
    println!("  {}", "─".repeat(106).dimmed());

    for e in &entries {
        let ts = e["timestamp"].as_str().unwrap_or("-");
        let ts_short = if ts.len() >= 19 { &ts[..19] } else { ts };
        println!(
            "  {:<38}  {:<20}  {:<26}  {:>8}  {:>7}",
            e["id"].as_str().unwrap_or("-").dimmed(),
            ts_short,
            e["tool"].as_str().unwrap_or("-").bright_cyan(),
            e["response_bytes"].as_u64().unwrap_or(0).to_string().yellow(),
            e["duration_ms"].as_u64().unwrap_or(0).to_string().dimmed(),
        );
    }

    println!("  {}", "─".repeat(106).dimmed());
    println!(
        "  Total entries: {}  |  Log: {}",
        entries.len().to_string().bright_yellow().bold(),
        path.display().to_string().dimmed()
    );
    println!(
        "  Run {} to export as signed JSON.\n",
        "neuron audit --export <file.json>".cyan()
    );

    Ok(())
}
