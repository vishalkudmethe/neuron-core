//! Live Stream Context Compiler — compiles hyper-focused context payloads
//! around the active editing path for the /v1/context/stream HTTP endpoint.

use anyhow::Result;
use std::path::{Path, PathBuf};
use crate::{config::NeuronConfig, dedup, intent, manifest::NeuronManifest, sanitize, search, utils};

pub async fn compile_stream_context(project_root: &Path) -> Result<String> {
    let manifest = NeuronManifest::load(project_root).await?;
    let config = NeuronConfig::load(project_root).await;

    // Character capacity based on the active profile's token cap (approx 4 chars per token)
    let char_cap = config.token_cap * 4;

    let mut out = String::new();
    out.push_str("# NEURON LIVE STREAM CONTEXT\n");
    out.push_str(&format!("> **Project:** {} | **Mode:** Active Focus Stream\n\n", manifest.name));

    // ── 1. Check for Active Execution Failure ─────────────────────────────────
    if let Some(err_log) = intent::load_fresh_error_log(project_root).await {
        out.push_str("## 🔴 Active Execution Failure\n");
        out.push_str(&format!("**Command:** `{}`\n", err_log.command));
        out.push_str("**Error stderr:**\n```\n");
        out.push_str(&err_log.stderr);
        out.push_str("\n```\n\n");
    }

    // ── 2. Look up highest-scoring files from intent_log ──────────────────────
    let intent_log = intent::load_intent_log(project_root).await;
    let mut focus_files = vec![];
    if let Some(log) = intent_log {
        for entry in log.entries {
            // entries with score >= 50 are modified within 10 mins
            if entry.score >= 50 {
                focus_files.push(entry.file_path);
            }
        }
    }

    // If no active edit files, fallback to main files or output a warning
    if focus_files.is_empty() {
        out.push_str("*(No files under active edit within the last 10 minutes. Edit some files to populate stream context.)*\n");
        return Ok(out);
    }

    // Process up to 3 focus files to keep it hyper-focused
    let focus_files: Vec<String> = focus_files.into_iter().take(3).collect();

    let db_path = utils::local_db_path(project_root);
    let pool = search::open_local_db(&db_path).await?;

    for file_path_str in &focus_files {
        let path = PathBuf::from(file_path_str);
        if !path.exists() {
            continue;
        }

        let rel_path = path.strip_prefix(project_root).unwrap_or(&path).to_string_lossy().to_string();
        out.push_str(&format!("### 📁 Focal File: `{}`\n", rel_path));

        // Read full source content
        if let Ok(content) = tokio::fs::read_to_string(&path).await {
            let sanitized = sanitize::sanitize_content(&content);
            out.push_str("```\n");
            out.push_str(&sanitized);
            out.push_str("\n```\n\n");
        }

        // ── 3. Adjacent internal module dependencies ───────────────────────────
        if let Some(parent_dir) = path.parent() {
            out.push_str("#### 🧩 Adjacent Structural Context\n");
            let mut entries = tokio::fs::read_dir(parent_dir).await?;
            let mut adj_count = 0;
            while let Some(entry) = entries.next_entry().await.unwrap_or(None) {
                let adj_path = entry.path();
                if adj_path.is_file() && adj_path != path {
                    let adj_rel = adj_path.strip_prefix(project_root).unwrap_or(&adj_path).to_string_lossy().to_string();
                    let symbols: Vec<(String, String, Option<String>)> = sqlx::query_as(
                        "SELECT symbol_name, symbol_type, semantic_intent FROM memory_units \
                         WHERE file_path = ?1 AND unit_type != 'file' AND symbol_type IN ('class', 'struct', 'interface', 'method', 'function') LIMIT 3"
                    )
                    .bind(adj_path.to_string_lossy().as_ref())
                    .fetch_all(&pool)
                    .await
                    .unwrap_or_default();

                    if !symbols.is_empty() {
                        out.push_str(&format!("*   **{}**:\n", adj_rel));
                        for (name, kind, intent) in symbols {
                            let clean_name = sanitize::sanitize_content(&name);
                            let intent_str = intent.filter(|i| !i.is_empty()).unwrap_or_else(|| "—".to_string());
                            let clean_intent = sanitize::sanitize_content(&intent_str);
                            out.push_str(&format!("      - `{}` ({}) - *Intent:* {}\n", clean_name, kind, clean_intent));
                        }
                        adj_count += 1;
                        if adj_count >= 3 {
                            break;
                        }
                    }
                }
            }
            if adj_count == 0 {
                out.push_str("*(No adjacent structures found)*\n");
            }
            out.push_str("\n");
        }

        // ── 4. Active signature mutations (v11) ───────────────────────────────
        if let Ok(current_project_id) = crate::dependency::project_id_for_alias(&manifest.name).await {
            if let Ok(parent_ids) = crate::dependency::get_parent_ids(&current_project_id).await {
                for parent_id in &parent_ids {
                    if let Ok(mutations) = crate::dependency::get_recent_mutations(parent_id, 48).await {
                        let mut mutations_in_file = vec![];
                        for (sym_name, sym_type, changed_at) in mutations {
                            let count: i64 = sqlx::query_scalar(
                                "SELECT COUNT(*) FROM memory_units WHERE file_path = ?1 AND content LIKE ?2"
                            )
                            .bind(path.to_string_lossy().as_ref())
                            .bind(format!("%{}%", sym_name))
                            .fetch_one(&pool)
                            .await
                            .unwrap_or(0);

                            if count > 0 {
                                mutations_in_file.push(format!("*   `{}` ({}) — changed at `{}`", sym_name, sym_type, &changed_at[..19]));
                            }
                        }

                        if !mutations_in_file.is_empty() {
                            out.push_str("#### ⚠️ At-Risk Parent Mutations Used Here\n");
                            out.push_str(&mutations_in_file.join("\n"));
                            out.push_str("\n\n");
                        }
                    }
                }
            }
        }
    }

    // Apply token cap limits
    if out.len() > char_cap {
        let warning = format!(
            "\n\n... [STREAM CONTEXT TRUNCATED TO FIT {} TOKEN BUDGET] ...",
            config.token_cap
        );
        let truncate_len = char_cap.saturating_sub(warning.len());
        out = format!("{}{}", &out[..truncate_len], warning);
    }

    // ── 5. AST deduplication pass (v14) ──────────────────────────────────────
    let out = dedup::deduplicate_context(&out);

    Ok(out)
}
