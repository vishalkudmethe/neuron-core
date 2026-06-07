//! Shared utility functions used across all Neuron modules.

use anyhow::{Context, Result};
use chrono::Utc;
use colored::Colorize;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use tokio::fs;
use tracing::info;

// ─── Path Utilities ───────────────────────────────────────────────────────────

/// Walk upward from `start` to find the nearest directory containing `.neuron/`.
/// Returns the project root (parent of `.neuron/`), NOT the `.neuron/` path itself.
pub fn find_neuron_root(start: &Path) -> Option<PathBuf> {
    let mut current = start.to_path_buf();
    loop {
        let candidate = current.join(".neuron");
        if candidate.is_dir() {
            return Some(current);
        }
        if !current.pop() {
            break;
        }
    }
    None
}

/// Given a project root, return the `.neuron/` directory path.
pub fn neuron_dir(project_root: &Path) -> PathBuf {
    project_root.join(".neuron")
}

/// Given a project root, return the local SQLite index path.
pub fn local_db_path(project_root: &Path) -> PathBuf {
    neuron_dir(project_root).join("index.sqlite")
}

/// Return the global index path: `~/.neuron/global_index.sqlite`
pub fn global_db_path() -> Result<PathBuf> {
    let home = dirs::home_dir()
        .ok_or_else(|| anyhow::anyhow!("Cannot determine home directory"))?;
    Ok(home.join(".neuron").join("global_index.sqlite"))
}

/// Return the global neuron config directory: `~/.neuron/`
pub fn global_neuron_dir() -> Result<PathBuf> {
    let home = dirs::home_dir()
        .ok_or_else(|| anyhow::anyhow!("Cannot determine home directory"))?;
    Ok(home.join(".neuron"))
}

/// Create directory and all parents if they don't exist.
pub async fn ensure_dir(path: &Path) -> Result<()> {
    fs::create_dir_all(path)
        .await
        .with_context(|| format!("Failed to create directory: {}", path.display()))?;
    Ok(())
}

// ─── Hashing ─────────────────────────────────────────────────────────────────

/// Compute SHA-256 of a file's contents. Returns hex string.
pub async fn sha256_file(path: &Path) -> Result<String> {
    let bytes = fs::read(path)
        .await
        .with_context(|| format!("Failed to read file for hashing: {}", path.display()))?;
    let hash = Sha256::digest(&bytes);
    Ok(hex::encode(hash))
}

/// Compute SHA-256 of a string slice.
pub fn sha256_str(s: &str) -> String {
    let hash = Sha256::digest(s.as_bytes());
    hex::encode(hash)
}

// ─── Machine Identity ─────────────────────────────────────────────────────────

/// Deterministic machine ID from hostname + username.
/// Uses COMPUTERNAME (Windows) / HOSTNAME (Unix) env vars with std fallback.
pub fn machine_id() -> String {
    let hostname = std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .unwrap_or_else(|_| "unknown-host".to_string());
    let username = std::env::var("USERNAME")
        .or_else(|_| std::env::var("USER"))
        .unwrap_or_else(|_| "unknown-user".to_string());
    sha256_str(&format!("{hostname}:{username}"))[..16].to_string()
}

// ─── Backup ───────────────────────────────────────────────────────────────────

/// Create a timestamped backup of `.neuron/` inside `.neuron/backups/`.
pub async fn backup_neuron_dir(project_root: &Path) -> Result<()> {
    let neuron_path = neuron_dir(project_root);
    let backups_dir = neuron_path.join("backups");
    ensure_dir(&backups_dir).await?;

    let timestamp = Utc::now().format("%Y%m%d_%H%M%S").to_string();
    let backup_path = backups_dir.join(format!("backup_{timestamp}"));

    copy_dir_all(&neuron_path, &backup_path).await?;

    info!(
        "Backup created at: {}",
        backup_path.display().to_string().green()
    );
    println!(
        "  {} Backup → {}",
        "✓".green(),
        backup_path.display().to_string().dimmed()
    );
    Ok(())
}

/// Recursively copy a directory. Used for backups.
fn copy_dir_all<'a>(
    src: &'a Path,
    dst: &'a Path,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<()>> + Send + 'a>> {
    Box::pin(async move {
        ensure_dir(dst).await?;
        let mut entries = fs::read_dir(src)
            .await
            .with_context(|| format!("Cannot read dir: {}", src.display()))?;

        while let Some(entry) = entries.next_entry().await? {
            let file_type = entry.file_type().await?;
            let src_path = entry.path();
            let dst_path = dst.join(entry.file_name());

            // Skip backups/ to avoid recursive explosion
            if src_path.ends_with("backups") {
                continue;
            }

            if file_type.is_dir() {
                copy_dir_all(&src_path, &dst_path).await?;
            } else {
                fs::copy(&src_path, &dst_path)
                    .await
                    .with_context(|| {
                        format!("Copy failed: {} → {}", src_path.display(), dst_path.display())
                    })?;
            }
        }
        Ok(())
    })
}

// ─── Formatting ───────────────────────────────────────────────────────────────

/// Format a chrono Duration as a human-readable string.
pub fn format_duration(d: chrono::Duration) -> String {
    if d.num_seconds() < 60 {
        format!("{}s ago", d.num_seconds())
    } else if d.num_minutes() < 60 {
        format!("{}m ago", d.num_minutes())
    } else if d.num_hours() < 24 {
        format!("{}h ago", d.num_hours())
    } else {
        format!("{}d ago", d.num_days())
    }
}

/// Truncate a string to `max_len` chars, appending `…` if truncated.
pub fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}…", &s[..max_len.saturating_sub(1)])
    }
}
