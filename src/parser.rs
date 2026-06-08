//! Tree-sitter and custom AST-based symbol extractor.

use anyhow::Result;
use std::path::Path;
use tokio::fs;
use tracing::debug;

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct Symbol {
    pub name:            String,
    pub kind:            SymbolKind,
    pub language:        String,
    pub start_line:      usize,
    pub end_line:        usize,
    pub snippet:         String,
    pub semantic_intent: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub enum SymbolKind {
    Function, Method, Struct, Enum, Trait,
    Class, Module, Import, Constant, Other,
}

impl std::fmt::Display for SymbolKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Function => "function", Self::Method   => "method",
            Self::Struct   => "struct",   Self::Enum     => "enum",
            Self::Trait    => "trait",    Self::Class    => "class",
            Self::Module   => "module",   Self::Import   => "import",
            Self::Constant => "constant", Self::Other    => "other",
        };
        write!(f, "{s}")
    }
}

pub fn detect_language(path: &Path) -> Option<&'static str> {
    match path.extension()?.to_str()? {
        "rs"                  => Some("rust"),
        "py" | "pyw"          => Some("python"),
        "js" | "mjs" | "cjs"  => Some("javascript"),
        "ts" | "mts"          => Some("typescript"),
        "tsx"                 => Some("tsx"),
        "go"                  => Some("go"),
        "java"                => Some("java"),
        "dart"                => Some("dart"),
        _                     => None,
    }
}

pub async fn extract_symbols(path: &Path) -> Result<Vec<Symbol>> {
    let lang = match detect_language(path) {
        Some(l) => l,
        None    => return Ok(vec![]),
    };
    let source = match fs::read_to_string(path).await {
        Ok(s)  => s,
        Err(e) => { debug!("Cannot read {}: {e}", path.display()); return Ok(vec![]); }
    };
    
    Ok(match lang {
        "rust"                  => extract_rust(&source),
        "python"                => extract_python_ts(&source),
        "javascript"            => extract_javascript(&source),
        "typescript" | "tsx"    => extract_typescript(&source),
        "java"                  => extract_java(&source),
        "dart"                  => extract_dart(&source),
        _                       => vec![],
    })
}

// ─── Preceding Comments Lookback ──────────────────────────────────────────────

fn extract_preceding_comments(source: &str, start_row: usize) -> String {
    let lines: Vec<&str> = source.lines().collect();
    if start_row == 0 {
        return String::new();
    }
    
    let mut comment_lines = Vec::new();
    let mut curr = start_row;
    
    while curr > 0 {
        curr -= 1;
        let line = lines[curr].trim();
        if line.is_empty() {
            break;
        }
        
        if line.starts_with("///") {
            comment_lines.push(line["///".len()..].trim().to_string());
        } else if line.starts_with("//") {
            comment_lines.push(line["//".len()..].trim().to_string());
        } else if line.starts_with("/**") {
            comment_lines.push(line["/**".len()..].trim().to_string());
        } else if line.starts_with("/*") {
            comment_lines.push(line["/*".len()..].trim().to_string());
        } else if line.starts_with("*") {
            comment_lines.push(line["*".len()..].trim().to_string());
        } else {
            break;
        }
    }
    
    comment_lines.reverse();
    comment_lines.join("\n")
}

// ─── Rust ─────────────────────────────────────────────────────────────────────

fn extract_rust(source: &str) -> Vec<Symbol> {
    let mut parser = tree_sitter::Parser::new();
    if parser.set_language(tree_sitter_rust::language()).is_err() {
        return vec![];
    }
    let tree = match parser.parse(source, None) {
        Some(t) => t,
        None    => return vec![],
    };
    let mut symbols = vec![];
    walk_rust(tree.root_node(), source.as_bytes(), source, false, &mut symbols);
    symbols
}

fn walk_rust(node: tree_sitter::Node, bytes: &[u8], src: &str, in_impl: bool, out: &mut Vec<Symbol>) {
    let mut current_in_impl = in_impl;
    if node.kind() == "impl_item" {
        current_in_impl = true;
    }

    let (kind_opt, name_field) = match node.kind() {
        "function_item" => {
            let k = if current_in_impl { SymbolKind::Method } else { SymbolKind::Function };
            (Some(k), "name")
        }
        "struct_item"   => (Some(SymbolKind::Struct),   "name"),
        "enum_item"     => (Some(SymbolKind::Enum),     "name"),
        "trait_item"    => (Some(SymbolKind::Trait),    "name"),
        _               => (None, ""),
    };

    if let Some(kind) = kind_opt {
        if let Some(nn) = node.child_by_field_name(name_field) {
            let name    = nn.utf8_text(bytes).unwrap_or("").to_string();
            let row     = node.start_position().row;
            let snippet = src.lines().nth(row).unwrap_or("").trim().to_string();
            let semantic_intent = extract_preceding_comments(src, row);
            out.push(Symbol {
                name,
                kind,
                language: "rust".to_string(),
                start_line: row + 1,
                end_line: node.end_position().row + 1,
                snippet,
                semantic_intent,
            });
        }
    }

    for i in 0..node.child_count() {
        if let Some(c) = node.child(i) {
            walk_rust(c, bytes, src, current_in_impl, out);
        }
    }
}

// ─── Python ───────────────────────────────────────────────────────────────────

fn extract_python_ts(source: &str) -> Vec<Symbol> {
    let mut parser = tree_sitter::Parser::new();
    if parser.set_language(tree_sitter_python::language()).is_err() {
        return vec![];
    }
    let tree = match parser.parse(source, None) {
        Some(t) => t,
        None    => return vec![],
    };
    let mut symbols = vec![];
    walk_python(tree.root_node(), source.as_bytes(), source, false, &mut symbols);
    symbols
}

fn walk_python(node: tree_sitter::Node, bytes: &[u8], src: &str, in_class: bool, out: &mut Vec<Symbol>) {
    let mut current_in_class = in_class;
    if node.kind() == "class_definition" {
        current_in_class = true;
    }

    let (kind_opt, name_field) = match node.kind() {
        "function_definition" => {
            let k = if current_in_class { SymbolKind::Method } else { SymbolKind::Function };
            (Some(k), "name")
        }
        "class_definition" => (Some(SymbolKind::Class), "name"),
        _ => (None, ""),
    };

    if let Some(kind) = kind_opt {
        if let Some(nn) = node.child_by_field_name(name_field) {
            let name    = nn.utf8_text(bytes).unwrap_or("").to_string();
            let row     = node.start_position().row;
            let snippet = src.lines().nth(row).unwrap_or("").trim().to_string();
            let semantic_intent = extract_preceding_comments(src, row);
            out.push(Symbol {
                name,
                kind,
                language: "python".to_string(),
                start_line: row + 1,
                end_line: node.end_position().row + 1,
                snippet,
                semantic_intent,
            });
        }
    }

    for i in 0..node.child_count() {
        if let Some(c) = node.child(i) {
            walk_python(c, bytes, src, current_in_class, out);
        }
    }
}

// ─── JS / TS ──────────────────────────────────────────────────────────────────

fn extract_javascript(source: &str) -> Vec<Symbol> {
    let mut parser = tree_sitter::Parser::new();
    if parser.set_language(tree_sitter_javascript::language()).is_err() {
        return vec![];
    }
    let tree = match parser.parse(source, None) {
        Some(t) => t,
        None    => return vec![],
    };
    let mut symbols = vec![];
    walk_js_ts(tree.root_node(), source.as_bytes(), source, "javascript", &mut symbols);
    symbols
}

fn extract_typescript(source: &str) -> Vec<Symbol> {
    let mut parser = tree_sitter::Parser::new();
    if parser.set_language(tree_sitter_typescript::language_typescript()).is_err() {
        return vec![];
    }
    let tree = match parser.parse(source, None) {
        Some(t) => t,
        None    => return vec![],
    };
    let mut symbols = vec![];
    walk_js_ts(tree.root_node(), source.as_bytes(), source, "typescript", &mut symbols);
    symbols
}

fn walk_js_ts(node: tree_sitter::Node, bytes: &[u8], src: &str, lang: &str, out: &mut Vec<Symbol>) {
    let (kind_opt, name_field) = match node.kind() {
        "function_declaration" | "function" => (Some(SymbolKind::Function), "name"),
        "class_declaration" | "class"       => (Some(SymbolKind::Class), "name"),
        "method_definition"                 => (Some(SymbolKind::Method), "name"),
        "interface_declaration"             => (Some(SymbolKind::Trait), "name"),
        _                                   => (None, ""),
    };

    if let Some(kind) = kind_opt {
        if let Some(nn) = node.child_by_field_name(name_field) {
            let name    = nn.utf8_text(bytes).unwrap_or("").to_string();
            let row     = node.start_position().row;
            let snippet = src.lines().nth(row).unwrap_or("").trim().to_string();
            let semantic_intent = extract_preceding_comments(src, row);
            out.push(Symbol {
                name,
                kind,
                language: lang.to_string(),
                start_line: row + 1,
                end_line: node.end_position().row + 1,
                snippet,
                semantic_intent,
            });
        } else {
            let mut name_opt = None;
            for i in 0..node.child_count() {
                if let Some(c) = node.child(i) {
                    if c.kind() == "identifier" || c.kind() == "property_identifier" {
                        name_opt = Some(c.utf8_text(bytes).unwrap_or("").to_string());
                        break;
                    }
                }
            }
            if let Some(name) = name_opt {
                if !name.is_empty() {
                    let row     = node.start_position().row;
                    let snippet = src.lines().nth(row).unwrap_or("").trim().to_string();
                    let semantic_intent = extract_preceding_comments(src, row);
                    out.push(Symbol {
                        name,
                        kind,
                        language: lang.to_string(),
                        start_line: row + 1,
                        end_line: node.end_position().row + 1,
                        snippet,
                        semantic_intent,
                    });
                }
            }
        }
    }

    for i in 0..node.child_count() {
        if let Some(c) = node.child(i) {
            walk_js_ts(c, bytes, src, lang, out);
        }
    }
}

// ─── Java Custom Parser ───────────────────────────────────────────────────────

fn extract_java(source: &str) -> Vec<Symbol> {
    let mut symbols = vec![];
    let lines: Vec<&str> = source.lines().collect();
    
    let re_class = regex::Regex::new(r#"(?:class|interface|enum)\s+([a-zA-Z_][a-zA-Z0-9_]*)"#).unwrap();
    let re_method = regex::Regex::new(
        r#"(?:public|protected|private|static|\s)+\s+([a-zA-Z_][a-zA-Z0-9_<>\?]*)\s+([a-zA-Z_][a-zA-Z0-9_]*)\s*\([^\)]*\)\s*(?:throws\s+[a-zA-Z0-9_,\s]+)?\s*\{"#
    ).unwrap();
    
    for (i, line) in lines.iter().enumerate() {
        let t = line.trim();
        if t.starts_with("//") || t.starts_with("*") || t.starts_with("/*") {
            continue;
        }
        
        if let Some(cap) = re_class.captures(t) {
            let name = cap.get(1).unwrap().as_str().to_string();
            let kind = if t.contains("interface") {
                SymbolKind::Trait
            } else if t.contains("enum") {
                SymbolKind::Enum
            } else {
                SymbolKind::Class
            };
            let semantic_intent = extract_preceding_comments(source, i);
            symbols.push(Symbol {
                name,
                kind,
                language: "java".to_string(),
                start_line: i + 1,
                end_line: i + 1,
                snippet: t.to_string(),
                semantic_intent,
            });
        } else if let Some(cap) = re_method.captures(t) {
            let ret_type = cap.get(1).unwrap().as_str();
            let name = cap.get(2).unwrap().as_str().to_string();
            
            if !["if", "while", "for", "switch", "return", "new", "catch"].contains(&ret_type) {
                let semantic_intent = extract_preceding_comments(source, i);
                symbols.push(Symbol {
                    name,
                    kind: SymbolKind::Method,
                    language: "java".to_string(),
                    start_line: i + 1,
                    end_line: i + 1,
                    snippet: t.to_string(),
                    semantic_intent,
                });
            }
        }
    }
    symbols
}

// ─── Dart Custom Parser ───────────────────────────────────────────────────────

fn extract_dart(source: &str) -> Vec<Symbol> {
    let mut symbols = vec![];
    let lines: Vec<&str> = source.lines().collect();
    
    let re_class = regex::Regex::new(r#"\bclass\s+([a-zA-Z_][a-zA-Z0-9_]*)"#).unwrap();
    let re_method = regex::Regex::new(r#"\b([a-zA-Z_][a-zA-Z0-9_<>\?]*)\s+([a-zA-Z_][a-zA-Z0-9_]*)\s*\([^\)]*\)\s*\{"#).unwrap();
    
    for (i, line) in lines.iter().enumerate() {
        let t = line.trim();
        if t.starts_with("//") || t.starts_with("*") || t.starts_with("/*") {
            continue;
        }
        
        if let Some(cap) = re_class.captures(t) {
            let name = cap.get(1).unwrap().as_str().to_string();
            let semantic_intent = extract_preceding_comments(source, i);
            symbols.push(Symbol {
                name,
                kind: SymbolKind::Class,
                language: "dart".to_string(),
                start_line: i + 1,
                end_line: i + 1,
                snippet: t.to_string(),
                semantic_intent,
            });
        } else if let Some(cap) = re_method.captures(t) {
            let ret_type = cap.get(1).unwrap().as_str();
            let name = cap.get(2).unwrap().as_str().to_string();
            
            if !["if", "while", "for", "switch", "return", "new", "catch", "assert"].contains(&ret_type) {
                let semantic_intent = extract_preceding_comments(source, i);
                symbols.push(Symbol {
                    name,
                    kind: SymbolKind::Method,
                    language: "dart".to_string(),
                    start_line: i + 1,
                    end_line: i + 1,
                    snippet: t.to_string(),
                    semantic_intent,
                });
            }
        }
    }
    symbols
}
