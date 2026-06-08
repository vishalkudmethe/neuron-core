//! Context Redundancy Deduplication Compiler.
//!
//! Scans compiled prompt buffers, matches AST-style struct/class/enum declarations,
//! and replaces redundant duplicate definitions with token-saving pointer comments.

use std::collections::HashMap;
use regex::Regex;

/// Deduplicate identical structural definitions in context prompt.
pub fn deduplicate_context(content: &str) -> String {
    let re_decl = match Regex::new(r"(?m)^(pub\s+)?(struct|class|enum|interface)\s+([A-Za-z0-9_]+)\s*\{") {
        Ok(r) => r,
        Err(_) => return content.to_string(),
    };

    let temp_content = content.to_string();
    
    // Find all focal files and their positions to attribute original sources
    let re_file = Regex::new(r"### 📁 Focal File: `([^`]+)`").unwrap();
    let mut file_positions: Vec<(usize, String)> = re_file.find_iter(&temp_content)
        .map(|m| {
            let filename = m.as_str()
                .replace("### 📁 Focal File: `", "")
                .replace("`", "");
            (m.start(), filename)
        })
        .collect();
    file_positions.sort_by_key(|(pos, _)| *pos);

    let get_source_file = |pos: usize| -> String {
        let mut current_file = "shared".to_string();
        for (f_pos, f_name) in &file_positions {
            if *f_pos <= pos {
                current_file = f_name.clone();
            } else {
                break;
            }
        }
        current_file
    };

    let mut declarations: HashMap<String, String> = HashMap::new(); // body_hash -> source_file
    let mut final_output = String::new();
    let mut last_idx = 0;

    for mat in re_decl.find_iter(&temp_content) {
        let start_pos = mat.start();
        if start_pos < last_idx {
            continue;
        }

        // Find the start brace
        let open_brace_pos = mat.end() - 1;
        let mut brace_count = 1;
        let mut close_brace_pos = None;
        let bytes = temp_content.as_bytes();

        for i in (open_brace_pos + 1)..bytes.len() {
            if bytes[i] == b'{' {
                brace_count += 1;
            } else if bytes[i] == b'}' {
                brace_count -= 1;
                if brace_count == 0 {
                    close_brace_pos = Some(i);
                    break;
                }
            }
        }

        if let Some(close_pos) = close_brace_pos {
            // Find symbol name
            let decl_header = mat.as_str().trim_end_matches('{').trim();
            let name = decl_header
                .split_whitespace()
                .last()
                .unwrap_or("Unknown")
                .to_string();

            let body = temp_content[(open_brace_pos + 1)..close_pos].trim();
            let source_file = get_source_file(start_pos);

            // Unique key combining name + body layout for exact structure match
            let clean_body = body.chars().filter(|c| !c.is_whitespace()).collect::<String>();
            let key = format!("{}:{}", name, clean_body);

            final_output.push_str(&temp_content[last_idx..start_pos]);

            if let Some(first_source) = declarations.get(&key) {
                // Replace body with concise deduplication pointer comment
                final_output.push_str(&format!(
                    "{} {{ /* [Symbol body identical to {}::{} - Deduplicated to save tokens] */ }}",
                    decl_header, first_source, name
                ));
            } else {
                declarations.insert(key, source_file);
                final_output.push_str(&temp_content[start_pos..=close_pos]);
            }
            last_idx = close_pos + 1;
        }
    }

    if last_idx < temp_content.len() {
        final_output.push_str(&temp_content[last_idx..]);
    }

    final_output
}
