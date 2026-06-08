//! Neuron manifest — reads and writes `.neuron/neuron_manifest.json`.
//! The manifest is the single source of truth for project identity.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tokio::fs;
use uuid::Uuid;

// ─── Schema ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvolutionEntry {
    pub timestamp: String,
    pub file_path: String,
    pub tweak:     String,
    pub reason:    String,
}

fn default_intent() -> String {
    "A universal persistent memory layer and context compiler for AI coding agents.".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NeuronManifest {
    /// Unique project ID (UUID v4). Stable across machines.
    pub id: Uuid,

    /// Human-readable project name.
    pub name: String,

    /// Neuron spec version.
    pub version: String,

    /// Absolute path to the project root on this machine.
    pub root_path: PathBuf,

    /// Primary programming language.
    pub language: String,

    /// When this project was first initialized.
    pub created_at: DateTime<Utc>,

    /// When the manifest was last read or written.
    pub last_accessed: DateTime<Utc>,

    /// Compressed high-level summary of the overall project goal.
    #[serde(default = "default_intent")]
    pub top_level_intent: String,

    /// Evolution ledger tracking changes across sessions.
    #[serde(default)]
    pub evolution_ledger: Vec<EvolutionEntry>,

    /// User-defined tags for searching/grouping.
    #[serde(default)]
    pub tags: Vec<String>,

    /// Watcher + guard configuration.
    #[serde(default)]
    pub config: ManifestConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestConfig {
    /// Debounce window (ms) for file watcher events.
    pub watcher_debounce_ms: u64,

    /// Loop guard sliding window (seconds).
    pub loop_guard_window_sec: u64,

    /// Number of identical events in window before alert.
    pub loop_guard_threshold: u32,

    /// Auto-backup on `neuron watch` start.
    pub auto_backup: bool,

    /// Backup before switching projects.
    pub backup_on_switch: bool,

    /// Max conversation snapshot files to retain.
    pub max_conversation_snapshots: u32,

    /// Glob patterns to ignore (appended to .gitignore rules).
    pub ignored_paths: Vec<String>,
}

impl Default for ManifestConfig {
    fn default() -> Self {
        Self {
            watcher_debounce_ms: 300,
            loop_guard_window_sec: 60,
            loop_guard_threshold: 5,
            auto_backup: true,
            backup_on_switch: true,
            max_conversation_snapshots: 100,
            ignored_paths: vec![
                "target/".to_string(),
                ".git/".to_string(),
                "node_modules/".to_string(),
                "*.tmp".to_string(),
                "*.lock".to_string(),
            ],
        }
    }
}

// ─── Read / Write ─────────────────────────────────────────────────────────────

impl NeuronManifest {
    /// Create a brand-new manifest for a project being initialized.
    pub fn new(name: &str, root_path: &Path, language: &str) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            name: name.to_string(),
            version: "1.0.0".to_string(),
            root_path: root_path.to_path_buf(),
            language: language.to_string(),
            created_at: now,
            last_accessed: now,
            top_level_intent: default_intent(),
            evolution_ledger: vec![],
            tags: vec![],
            config: ManifestConfig::default(),
        }
    }

    /// Load manifest from `<project_root>/.neuron/neuron_manifest.json`.
    pub async fn load(project_root: &Path) -> Result<Self> {
        let path = manifest_path(project_root);
        let raw = fs::read_to_string(&path)
            .await
            .with_context(|| format!("Failed to read manifest at: {}", path.display()))?;
        let mut manifest: NeuronManifest = serde_json::from_str(&raw)
            .with_context(|| format!("Invalid JSON in manifest: {}", path.display()))?;

        // Refresh last_accessed on every load
        manifest.last_accessed = Utc::now();
        manifest.save(project_root).await?;
        Ok(manifest)
    }

    /// Write manifest to `<project_root>/.neuron/neuron_manifest.json`.
    /// Uses temp-file + rename for atomic writes.
    pub async fn save(&self, project_root: &Path) -> Result<()> {
        let final_path = manifest_path(project_root);
        let tmp_path   = final_path.with_extension("json.tmp");

        let json = serde_json::to_string_pretty(self)
            .context("Failed to serialize manifest")?;

        fs::write(&tmp_path, json)
            .await
            .with_context(|| format!("Failed to write temp manifest: {}", tmp_path.display()))?;

        fs::rename(&tmp_path, &final_path)
            .await
            .with_context(|| format!("Failed to rename manifest into place: {}", final_path.display()))?;

        Ok(())
    }
}

/// Full path to the manifest JSON file.
pub fn manifest_path(project_root: &Path) -> PathBuf {
    project_root.join(".neuron").join("neuron_manifest.json")
}
