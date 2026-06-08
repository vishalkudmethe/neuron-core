//! Team sync stubs and export functionality.

use anyhow::Result;
use chrono::Utc;
use colored::Colorize;
use std::path::Path;

use crate::utils;

/// Export `.neuron/` as a portable `.tar.gz` archive.
pub async fn export_archive(project_root: &Path, output: Option<&str>) -> Result<()> {
    let neuron_dir = utils::neuron_dir(project_root);
    let timestamp  = Utc::now().format("%Y%m%d_%H%M%S").to_string();
    let out_file   = output
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("neuron_export_{timestamp}.tar.gz"));

    println!(
        "\n{} Exporting .neuron/ → {}\n",
        "📦".cyan(),
        out_file.bold().yellow()
    );

    // Note: actual tar.gz creation requires the `tar` crate (not included in MVP).
    // This stub shows intent. In full implementation, use `tokio::process::Command`
    // to shell out to `tar -czf <out> .neuron/`, or add the `tar` crate.
    println!(
        "  {} Archive creation will be available in v5.1.\n  For now, copy {} manually.\n",
        "ℹ".cyan(),
        neuron_dir.display().to_string().cyan()
    );

    Ok(())
}

/// Stub: Future team sync (push/pull memory to shared store).
#[allow(dead_code)]
pub async fn sync_to_remote(_project_root: &Path, _remote_url: &str) -> Result<()> {
    println!(
        "\n{} Team sync is planned for v7. Stay tuned.\n",
        "ℹ".cyan()
    );
    Ok(())
}
