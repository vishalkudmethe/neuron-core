//! Real-time file system and Git watcher using `notify`.
//!
//! Architecture:
//!   - `start_watcher` launches a tokio task that listens for FS events.
//!   - Events are debounced and fed into the processing pipeline:
//!       file changed → hash check → parser → SQLite upsert
//!   - Git events (HEAD changes, new commits) are detected via `.git/` watching.

use anyhow::{Context, Result};
use chrono::Utc;
use colored::Colorize;
use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tokio::sync::Mutex;
use tracing::{debug, error, info, warn};

use crate::loop_guard::LoopGuard;
use crate::manifest::NeuronManifest;
use crate::parser;
use crate::search;
use crate::utils;

// ─── Event Pipeline ───────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct FileEvent {
    pub path:       PathBuf,
    pub kind:       FileEventKind,
    pub timestamp:  chrono::DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileEventKind {
    Modified,
    Created,
    Deleted,
    GitCommit,
    GitBranchSwitch,
}

// ─── Debouncer ────────────────────────────────────────────────────────────────

/// A simple in-memory debounce map: path → last event time.
struct Debouncer {
    last_seen:   HashMap<PathBuf, Instant>,
    window_ms:   u64,
}

impl Debouncer {
    fn new(window_ms: u64) -> Self {
        Self { last_seen: HashMap::new(), window_ms }
    }

    /// Returns `true` if this event should be forwarded (not debounced).
    fn should_emit(&mut self, path: &Path) -> bool {
        let now  = Instant::now();
        let key  = path.to_path_buf();
        let emit = match self.last_seen.get(&key) {
            None       => true,
            Some(&prev) => now.duration_since(prev) > Duration::from_millis(self.window_ms),
        };
        if emit {
            self.last_seen.insert(key, now);
        }
        emit
    }
}

// ─── Entry Point ─────────────────────────────────────────────────────────────

/// Start the watcher daemon. Blocks until Ctrl-C or a fatal error.
pub async fn start_watcher(project_root: &Path) -> Result<()> {
    let manifest = NeuronManifest::load(project_root)
        .await
        .context("Cannot start watcher: manifest not found")?;

    let debounce_ms = manifest.config.watcher_debounce_ms;
    let guard_cfg   = (
        manifest.config.loop_guard_window_sec,
        manifest.config.loop_guard_threshold,
    );

    // Backup on watch-start if configured
    if manifest.config.auto_backup {
        info!("Auto-backup before watch start …");
        utils::backup_neuron_dir(project_root).await?;
    }

    println!(
        "  {} Debounce: {}ms  |  Loop guard: {}× in {}s",
        "cfg".dimmed(),
        debounce_ms,
        guard_cfg.1,
        guard_cfg.0
    );

    let (tx, mut rx) = mpsc::channel::<FileEvent>(256);
    let project_root = project_root.to_path_buf();
    let db_path      = utils::local_db_path(&project_root);

    // Set up notify watcher in a blocking thread
    let watch_root = project_root.clone();
    let tx_clone   = tx.clone();

    std::thread::spawn(move || {
        let rt = tokio::runtime::Handle::current();
        let debouncer = Arc::new(Mutex::new(Debouncer::new(debounce_ms)));

        let mut watcher: RecommendedWatcher = {
            let tx2       = tx_clone.clone();
            let db2       = debouncer.clone();
            let root2     = watch_root.clone();

            notify::recommended_watcher(move |res: notify::Result<Event>| {
                match res {
                    Ok(event) => {
                        for path in &event.paths {
                            // Skip target/, .git/objects/, etc.
                            if should_skip(path, &root2) {
                                continue;
                            }

                            let kind = classify_event(path, &event.kind);
                            if let Some(kind) = kind {
                                let mut deb = rt.block_on(db2.lock());
                                if deb.should_emit(path) {
                                    let _ = tx2.blocking_send(FileEvent {
                                        path:      path.clone(),
                                        kind,
                                        timestamp: Utc::now(),
                                    });
                                }
                            }
                        }
                    }
                    Err(e) => error!("Watcher error: {e}"),
                }
            })
            .expect("Failed to create watcher")
        };

        watcher
            .watch(&watch_root, RecursiveMode::Recursive)
            .expect("Failed to watch project root");

        info!("File watcher active on: {}", watch_root.display());
        // Keep thread alive
        loop { std::thread::sleep(Duration::from_secs(3600)); }
    });

    // ── Processing loop ───────────────────────────────────────────────────────
    let mut loop_guard = LoopGuard::new(guard_cfg.0, guard_cfg.1);
    let pool = search::open_local_db(&db_path).await?;

    println!(
        "\n  {} Watching for changes. Press {} to stop.\n",
        "👁".cyan(),
        "Ctrl-C".bold()
    );

    while let Some(event) = rx.recv().await {
        // Loop guard check
        let event_key = format!("{}", event.path.display());
        if loop_guard.record(&event_key) {
            warn!(
                "{} Loop detected on path: {}. Pausing watcher for 5s.",
                "⚡".yellow(),
                event.path.display()
            );
            tokio::time::sleep(Duration::from_secs(5)).await;
            loop_guard.reset();
            continue;
        }

        match &event.kind {
            FileEventKind::Modified | FileEventKind::Created => {
                if let Err(e) = process_file_change(&pool, &project_root, &event).await {
                    error!("Error processing {}: {e:#}", event.path.display());
                }
            }
            FileEventKind::Deleted => {
                debug!("Deleted: {}", event.path.display());
                // Optionally mark as deleted in DB — for now just log
            }
            FileEventKind::GitCommit => {
                println!(
                    "  {} New git commit detected",
                    "⚡".bright_yellow()
                );
                if let Err(e) = crate::git::index_latest_commit(&pool, &project_root).await {
                    error!("Git index error: {e:#}");
                }
            }
            FileEventKind::GitBranchSwitch => {
                println!(
                    "  {} Git branch switched — consider running {} for updated context",
                    "⇄".bright_cyan(),
                    "neuron restore".yellow()
                );
            }
        }
    }

    Ok(())
}

// ─── Processing ───────────────────────────────────────────────────────────────

async fn process_file_change(
    pool:         &sqlx::SqlitePool,
    project_root: &Path,
    event:        &FileEvent,
) -> Result<()> {
    let path = &event.path;

    // Compute new hash
    let new_hash = match utils::sha256_file(path).await {
        Ok(h)  => h,
        Err(_) => return Ok(()), // File might have been deleted between event and now
    };

    // Check existing hash in DB to avoid redundant re-indexing
    let existing: Option<(String, i64)> =
        sqlx::query_as("SELECT sha256, COUNT(*) FROM memory_units WHERE file_path = ?1 GROUP BY sha256 LIMIT 1")
            .bind(path.to_string_lossy().as_ref())
            .fetch_optional(pool)
            .await
            .unwrap_or(None);

    if let Some((existing_hash, _)) = &existing {
        if *existing_hash == new_hash {
            debug!("Unchanged: {}", path.display());
            return Ok(());
        }
    }

    let prior_symbol_count: i64 = existing.map(|(_, c)| c).unwrap_or(0);

    // Parse symbols from changed file
    let rel_path = path
        .strip_prefix(project_root)
        .unwrap_or(path)
        .to_string_lossy()
        .to_string();

    let symbols = parser::extract_symbols(path).await.unwrap_or_default();

    println!(
        "  {} {} ({} symbol(s))",
        "↺".cyan(),
        rel_path.bold(),
        symbols.len()
    );

    // Upsert into SQLite
    search::upsert_file(pool, project_root, path, &new_hash, &symbols).await?;

    // ── Evolution Ledger ─────────────────────────────────────────────────────
    // Record this change in the manifest's evolution_ledger so neuron context
    // can surface it in the RECENT ARCHITECTURAL TWEAKS section.
    if let Ok(mut manifest) = NeuronManifest::load(project_root).await {
        let new_count = symbols.len() as i64;
        let tweak = if new_count > prior_symbol_count {
            format!("`{}` — added {} symbol(s)", rel_path, new_count - prior_symbol_count)
        } else if new_count < prior_symbol_count {
            format!("`{}` — removed {} symbol(s)", rel_path, prior_symbol_count - new_count)
        } else {
            format!("`{}` — modified (symbols unchanged)", rel_path)
        };
        let entry = crate::manifest::EvolutionEntry {
            timestamp: Utc::now().format("%Y-%m-%d %H:%M UTC").to_string(),
            file_path: rel_path.clone(),
            tweak,
            reason: "Detected by neuron watch file-change pipeline".to_string(),
        };
        manifest.evolution_ledger.push(entry);
        // Cap ledger at 50 entries
        if manifest.evolution_ledger.len() > 50 {
            let drain_to = manifest.evolution_ledger.len() - 50;
            manifest.evolution_ledger.drain(..drain_to);
        }
        let _ = manifest.save(project_root).await;
    }

    Ok(())
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

fn should_skip(path: &Path, _root: &Path) -> bool {
    let path_str = path.to_string_lossy();
    // Skip Rust build artifacts, git objects, neuron backups
    path_str.contains("\\target\\")
        || path_str.contains("/target/")
        || path_str.contains(".git/objects")
        || path_str.contains(".git\\objects")
        || path_str.contains(".neuron\\backups")
        || path_str.contains(".neuron/backups")
        || path_str.ends_with(".tmp")
}

fn classify_event(path: &Path, kind: &notify::EventKind) -> Option<FileEventKind> {
    let path_str = path.to_string_lossy();

    // Git events
    if path_str.ends_with("COMMIT_EDITMSG") {
        return Some(FileEventKind::GitCommit);
    }
    if path_str.ends_with(".git/HEAD") || path_str.ends_with(".git\\HEAD") {
        return Some(FileEventKind::GitBranchSwitch);
    }

    match kind {
        notify::EventKind::Modify(_) => Some(FileEventKind::Modified),
        notify::EventKind::Create(_) => Some(FileEventKind::Created),
        notify::EventKind::Remove(_) => Some(FileEventKind::Deleted),
        _ => None,
    }
}
