//! Conversation snapshot engine.
//! Saves timestamped markdown snapshots to `.neuron/conversations/`.

use anyhow::Result;
use chrono::Utc;
use colored::Colorize;
use std::path::Path;
use tokio::fs;

use crate::git;
use crate::utils;

pub async fn save_snapshot(project_root: &Path, note: Option<&str>) -> Result<()> {
    let conv_dir = project_root.join(".neuron").join("conversations");
    utils::ensure_dir(&conv_dir).await?;

    let now       = Utc::now();
    let timestamp = now.format("%Y-%m-%d_%H-%M-%S").to_string();
    let filename  = format!("{timestamp}.md");
    let path      = conv_dir.join(&filename);

    let branch  = git::current_branch(project_root).unwrap_or_else(|_| "unknown".to_string());
    let commit  = git::last_commit_message(project_root).unwrap_or_else(|_| "(none)".to_string());
    let note_md = note.map(|n| format!("\n## Session Note\n{n}\n")).unwrap_or_default();

    let content = format!(
        r#"# Neuron Conversation Snapshot
**Timestamp**: {}
**Git Branch**: {}
**Last Commit**: {}
{}
## Context at Snapshot Time
*(Add agent context, decisions, and next steps here.)*

## Files Changed This Session
*(Populated by `neuron watch` in future versions.)*

## Open Questions
- [ ] *(none recorded)*
"#,
        now.to_rfc3339(),
        branch,
        commit,
        note_md
    );

    fs::write(&path, &content).await?;

    // Prune old snapshots (keep max 100)
    prune_old_snapshots(&conv_dir, 100).await?;

    println!(
        "\n{} Snapshot saved: {}\n",
        "📸".cyan(),
        filename.bold()
    );
    Ok(())
}

async fn prune_old_snapshots(conv_dir: &Path, max_keep: usize) -> Result<()> {
    let mut entries = fs::read_dir(conv_dir).await?;
    let mut files = vec![];
    while let Some(e) = entries.next_entry().await? {
        if e.path().extension().and_then(|x| x.to_str()) == Some("md") {
            files.push(e.path());
        }
    }
    files.sort();
    if files.len() > max_keep {
        for old in &files[..files.len() - max_keep] {
            fs::remove_file(old).await.ok();
        }
    }
    Ok(())
}
