//! Tree-sitter multi-language symbol extractor.

use anyhow::Result;
use std::path::Path;
use tokio::fs;
use tracing::debug;

#[derive(Debug, Clone)]
pub struct Symbol {
    pub name:       String,
    pub kind:       SymbolKind,
    pub language:   String,
    pub start_line: usize,
    pub end_line:   usize,
    pub snippet:    String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
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
        "js" | "mjs" | "cjs" => Some("javascript"),
        "ts" | "mts"          => Some("typescript"),
        "tsx"                 => Some("tsx"),
        "go"                  => Some("go"),
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
        "rust"                    => extract_with_ts_rust(&source),
        "python"                  => extract_python(&source),
        "javascript" | "typescript" | "tsx" => extract_js_ts(&source),
        _                         => vec![],
    })
}

// ─── Rust ─────────────────────────────────────────────────────────────────────

fn extract_with_ts_rust(source: &str) -> Vec<Symbol> {
    let mut parser = tree_sitter::Parser::new();
    if parser.set_language(tree_sitter_rust::language()).is_err() {
        return extract_regex(source, "rust");
    }
    let tree = match parser.parse(source, None) {
        Some(t) => t,
        None    => return extract_regex(source, "rust"),
    };
    let mut symbols = vec![];
    walk_rust(tree.root_node(), source.as_bytes(), source, &mut symbols);
    symbols
}

fn walk_rust(node: tree_sitter::Node, bytes: &[u8], src: &str, out: &mut Vec<Symbol>) {
    let (kind_opt, name_field) = match node.kind() {
        "function_item" => (Some(SymbolKind::Function), "name"),
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
            out.push(Symbol { name, kind, language: "rust".to_string(), start_line: row+1, end_line: node.end_position().row+1, snippet });
        }
    }
    for i in 0..node.child_count() {
        if let Some(c) = node.child(i) { walk_rust(c, bytes, src, out); }
    }
}

// ─── Python ───────────────────────────────────────────────────────────────────

fn extract_python(source: &str) -> Vec<Symbol> {
    let mut symbols = vec![];
    for (i, line) in source.lines().enumerate() {
        let t = line.trim();
        if t.starts_with("def ") || t.starts_with("async def ") {
            if let Some(n) = name_after(t, &["async def", "def"]) {
                symbols.push(Symbol { name: n, kind: SymbolKind::Function, language: "python".to_string(), start_line: i+1, end_line: i+1, snippet: t.to_string() });
            }
        } else if t.starts_with("class ") {
            if let Some(n) = name_after(t, &["class"]) {
                symbols.push(Symbol { name: n, kind: SymbolKind::Class, language: "python".to_string(), start_line: i+1, end_line: i+1, snippet: t.to_string() });
            }
        }
    }
    symbols
}

// ─── JS / TS ──────────────────────────────────────────────────────────────────

fn extract_js_ts(source: &str) -> Vec<Symbol> {
    let mut symbols = vec![];
    for (i, line) in source.lines().enumerate() {
        let t = line.trim();
        if t.starts_with("function ") || t.starts_with("async function ") {
            if let Some(n) = name_after(t, &["async function", "function"]) {
                symbols.push(Symbol { name: n, kind: SymbolKind::Function, language: "javascript".to_string(), start_line: i+1, end_line: i+1, snippet: t.to_string() });
            }
        } else if t.starts_with("class ") || t.starts_with("export class ") {
            if let Some(n) = name_after(t, &["export class", "class"]) {
                symbols.push(Symbol { name: n, kind: SymbolKind::Class, language: "typescript".to_string(), start_line: i+1, end_line: i+1, snippet: t.to_string() });
            }
        } else if t.starts_with("interface ") || t.starts_with("export interface ") {
            if let Some(n) = name_after(t, &["export interface", "interface"]) {
                symbols.push(Symbol { name: n, kind: SymbolKind::Trait, language: "typescript".to_string(), start_line: i+1, end_line: i+1, snippet: t.to_string() });
            }
        }
    }
    symbols
}

// ─── Generic regex fallback ───────────────────────────────────────────────────

fn extract_regex(source: &str, lang: &str) -> Vec<Symbol> {
    let mut symbols = vec![];
    for (i, line) in source.lines().enumerate() {
        let t = line.trim();
        if t.contains("fn ") && t.contains('(') {
            if let Some(n) = name_after(t, &["pub async fn", "async fn", "pub fn", "fn"]) {
                symbols.push(Symbol { name: n, kind: SymbolKind::Function, language: lang.to_string(), start_line: i+1, end_line: i+1, snippet: t.to_string() });
            }
        }
    }
    symbols
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

fn name_after(line: &str, keywords: &[&str]) -> Option<String> {
    for kw in keywords {
        if line.starts_with(kw) {
            let rest: String = line[kw.len()..].trim_start()
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect();
            if !rest.is_empty() { return Some(rest); }
        }
    }
    None
}
