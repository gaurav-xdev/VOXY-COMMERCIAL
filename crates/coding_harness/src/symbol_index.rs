use crate::patcher::Patcher;
use crate::types::{HarnessError, Result};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use walkdir::WalkDir;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SymbolKind {
    Function,
    Struct,
    Enum,
    Trait,
    Class,
    Interface,
    TypeAlias,
    Constant,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Symbol {
    pub name: String,
    pub kind: SymbolKind,
    pub file_path: String,
    pub line_number: usize,
    pub signature: String,
    pub doc_comment: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RepositoryMap {
    pub file_symbols: HashMap<String, Vec<Symbol>>,
    pub total_symbols: usize,
}

impl RepositoryMap {
    /// Render a compact, token-efficient summary of the repository structure.
    /// Emits file paths and their high-level symbols, omitting function implementations.
    pub fn render_compact(&self) -> String {
        let mut out = String::new();
        let mut files: Vec<_> = self.file_symbols.keys().collect();
        files.sort();

        for file in files {
            if let Some(symbols) = self.file_symbols.get(file) {
                if symbols.is_empty() {
                    continue;
                }
                out.push_str(file);
                out.push_str(":\n");
                for s in symbols {
                    let kind_prefix = match s.kind {
                        SymbolKind::Function => "fn",
                        SymbolKind::Struct => "struct",
                        SymbolKind::Enum => "enum",
                        SymbolKind::Trait => "trait",
                        SymbolKind::Class => "class",
                        SymbolKind::Interface => "interface",
                        SymbolKind::TypeAlias => "type",
                        SymbolKind::Constant => "const",
                    };
                    out.push_str(&format!("  {} {}\n", kind_prefix, s.signature));
                }
            }
        }
        out
    }
}

const IGNORED_DIRS: &[&str] = &[
    ".git",
    "target",
    "node_modules",
    "dist",
    "build",
    ".osmoo",
    "__pycache__",
    ".venv",
    ".idea",
    ".vscode",
];

pub struct SymbolIndexer;

impl SymbolIndexer {
    /// Build a complete or scoped RepositoryMap for the given workspace.
    pub fn index_workspace(workspace_root: &Path, sub_dir: Option<&str>) -> Result<RepositoryMap> {
        let base_dir = if let Some(sp) = sub_dir {
            let rel = Patcher::canonicalize_workspace_rel(workspace_root, sp)?;
            workspace_root.join(rel)
        } else {
            workspace_root.to_path_buf()
        };

        // 1. If graphify-out/graph.json exists and we are indexing the root, try extracting from graphify
        if sub_dir.is_none() {
            let graph_file = workspace_root.join("graphify-out").join("graph.json");
            if graph_file.is_file() {
                if let Ok(map) = Self::load_from_graphify(&graph_file) {
                    if map.total_symbols > 0 {
                        return Ok(map);
                    }
                }
            }
        }

        // 2. Syntactic AST/regex extraction across workspace files
        let mut map = RepositoryMap::default();
        for entry in WalkDir::new(&base_dir)
            .into_iter()
            .filter_entry(|e| {
                let name = e.file_name().to_string_lossy();
                !IGNORED_DIRS.contains(&name.as_ref())
            })
            .flatten()
        {
            if entry.file_type().is_file() {
                let ext = entry
                    .path()
                    .extension()
                    .and_then(|s| s.to_str())
                    .unwrap_or("")
                    .to_lowercase();

                let is_code_file = matches!(
                    ext.as_str(),
                    "rs" | "ts" | "tsx" | "js" | "jsx" | "py" | "go" | "c" | "cpp" | "h" | "hpp"
                );

                if is_code_file {
                    if let Ok(content) = fs::read_to_string(entry.path()) {
                        let rel_path = entry
                            .path()
                            .strip_prefix(workspace_root)
                            .map(|p| p.to_string_lossy().replace('\\', "/"))
                            .unwrap_or_else(|_| entry.path().to_string_lossy().to_string());

                        let symbols = Self::extract_symbols_from_source(&rel_path, &content, &ext);
                        if !symbols.is_empty() {
                            map.total_symbols += symbols.len();
                            map.file_symbols.insert(rel_path, symbols);
                        }
                    }
                }
            }
        }

        Ok(map)
    }

    /// Extract symbols from graphify-out/graph.json knowledge graph if present.
    fn load_from_graphify(graph_file: &Path) -> Result<RepositoryMap> {
        let text = fs::read_to_string(graph_file).map_err(HarnessError::Io)?;
        let val: serde_json::Value = serde_json::from_str(&text).map_err(HarnessError::Json)?;

        let mut map = RepositoryMap::default();
        if let Some(nodes) = val.get("nodes").and_then(|n| n.as_array()) {
            for node in nodes {
                let label = node["label"].as_str().unwrap_or("");
                let file = node["file"].as_str().unwrap_or("");
                let line = node["line"].as_u64().unwrap_or(1) as usize;
                let kind_str = node["kind"].as_str().unwrap_or("");

                if file.is_empty() || label.is_empty() {
                    continue;
                }

                let kind = match kind_str {
                    "Function" | "Method" => SymbolKind::Function,
                    "Struct" => SymbolKind::Struct,
                    "Enum" => SymbolKind::Enum,
                    "Trait" => SymbolKind::Trait,
                    "Class" => SymbolKind::Class,
                    "Interface" => SymbolKind::Interface,
                    "TypeAlias" => SymbolKind::TypeAlias,
                    _ => continue,
                };

                let norm_file = file.replace('\\', "/");
                let sym = Symbol {
                    name: label.to_string(),
                    kind,
                    file_path: norm_file.clone(),
                    line_number: line,
                    signature: label.to_string(),
                    doc_comment: None,
                };

                map.file_symbols.entry(norm_file).or_default().push(sym);
                map.total_symbols += 1;
            }
        }

        Ok(map)
    }

    /// Extract code declarations from source content using regex patterns.
    pub fn extract_symbols_from_source(file_path: &str, content: &str, ext: &str) -> Vec<Symbol> {
        let mut symbols = Vec::new();
        let lines: Vec<&str> = content.lines().collect();

        match ext {
            "rs" => {
                let fn_re = Regex::new(r"^\s*(pub(?:\([^)]+\))?\s+)?(?:async\s+)?fn\s+([a-zA-Z0-9_]+)\s*(<[^>]+>)?\s*\(([^)]*)\)").unwrap();
                let struct_re =
                    Regex::new(r"^\s*(pub(?:\([^)]+\))?\s+)?struct\s+([a-zA-Z0-9_]+)").unwrap();
                let enum_re =
                    Regex::new(r"^\s*(pub(?:\([^)]+\))?\s+)?enum\s+([a-zA-Z0-9_]+)").unwrap();
                let trait_re =
                    Regex::new(r"^\s*(pub(?:\([^)]+\))?\s+)?trait\s+([a-zA-Z0-9_]+)").unwrap();
                let type_re =
                    Regex::new(r"^\s*(pub(?:\([^)]+\))?\s+)?type\s+([a-zA-Z0-9_]+)").unwrap();

                for (idx, line) in lines.iter().enumerate() {
                    let trimmed = line.trim();
                    if let Some(caps) = fn_re.captures(trimmed) {
                        let name = caps.get(2).map(|m| m.as_str()).unwrap_or("");
                        symbols.push(Symbol {
                            name: name.to_string(),
                            kind: SymbolKind::Function,
                            file_path: file_path.to_string(),
                            line_number: idx + 1,
                            signature: trimmed.trim_end_matches('{').trim().to_string(),
                            doc_comment: Self::lookbehind_doc(&lines, idx),
                        });
                    } else if let Some(caps) = struct_re.captures(trimmed) {
                        let name = caps.get(2).map(|m| m.as_str()).unwrap_or("");
                        symbols.push(Symbol {
                            name: name.to_string(),
                            kind: SymbolKind::Struct,
                            file_path: file_path.to_string(),
                            line_number: idx + 1,
                            signature: name.to_string(),
                            doc_comment: Self::lookbehind_doc(&lines, idx),
                        });
                    } else if let Some(caps) = enum_re.captures(trimmed) {
                        let name = caps.get(2).map(|m| m.as_str()).unwrap_or("");
                        symbols.push(Symbol {
                            name: name.to_string(),
                            kind: SymbolKind::Enum,
                            file_path: file_path.to_string(),
                            line_number: idx + 1,
                            signature: name.to_string(),
                            doc_comment: Self::lookbehind_doc(&lines, idx),
                        });
                    } else if let Some(caps) = trait_re.captures(trimmed) {
                        let name = caps.get(2).map(|m| m.as_str()).unwrap_or("");
                        symbols.push(Symbol {
                            name: name.to_string(),
                            kind: SymbolKind::Trait,
                            file_path: file_path.to_string(),
                            line_number: idx + 1,
                            signature: name.to_string(),
                            doc_comment: Self::lookbehind_doc(&lines, idx),
                        });
                    } else if let Some(caps) = type_re.captures(trimmed) {
                        let name = caps.get(2).map(|m| m.as_str()).unwrap_or("");
                        symbols.push(Symbol {
                            name: name.to_string(),
                            kind: SymbolKind::TypeAlias,
                            file_path: file_path.to_string(),
                            line_number: idx + 1,
                            signature: trimmed.trim_end_matches(';').trim().to_string(),
                            doc_comment: Self::lookbehind_doc(&lines, idx),
                        });
                    }
                }
            }
            "ts" | "tsx" | "js" | "jsx" => {
                let fn_re =
                    Regex::new(r"^\s*(export\s+)?(?:async\s+)?function\s+([a-zA-Z0-9_]+)").unwrap();
                let class_re = Regex::new(r"^\s*(export\s+)?class\s+([a-zA-Z0-9_]+)").unwrap();
                let iface_re = Regex::new(r"^\s*(export\s+)?interface\s+([a-zA-Z0-9_]+)").unwrap();
                let type_re = Regex::new(r"^\s*(export\s+)?type\s+([a-zA-Z0-9_]+)").unwrap();

                for (idx, line) in lines.iter().enumerate() {
                    let trimmed = line.trim();
                    if let Some(caps) = fn_re.captures(trimmed) {
                        let name = caps.get(2).map(|m| m.as_str()).unwrap_or("");
                        symbols.push(Symbol {
                            name: name.to_string(),
                            kind: SymbolKind::Function,
                            file_path: file_path.to_string(),
                            line_number: idx + 1,
                            signature: trimmed.trim_end_matches('{').trim().to_string(),
                            doc_comment: Self::lookbehind_doc(&lines, idx),
                        });
                    } else if let Some(caps) = class_re.captures(trimmed) {
                        let name = caps.get(2).map(|m| m.as_str()).unwrap_or("");
                        symbols.push(Symbol {
                            name: name.to_string(),
                            kind: SymbolKind::Class,
                            file_path: file_path.to_string(),
                            line_number: idx + 1,
                            signature: name.to_string(),
                            doc_comment: Self::lookbehind_doc(&lines, idx),
                        });
                    } else if let Some(caps) = iface_re.captures(trimmed) {
                        let name = caps.get(2).map(|m| m.as_str()).unwrap_or("");
                        symbols.push(Symbol {
                            name: name.to_string(),
                            kind: SymbolKind::Interface,
                            file_path: file_path.to_string(),
                            line_number: idx + 1,
                            signature: name.to_string(),
                            doc_comment: Self::lookbehind_doc(&lines, idx),
                        });
                    } else if let Some(caps) = type_re.captures(trimmed) {
                        let name = caps.get(2).map(|m| m.as_str()).unwrap_or("");
                        symbols.push(Symbol {
                            name: name.to_string(),
                            kind: SymbolKind::TypeAlias,
                            file_path: file_path.to_string(),
                            line_number: idx + 1,
                            signature: name.to_string(),
                            doc_comment: Self::lookbehind_doc(&lines, idx),
                        });
                    }
                }
            }
            "py" => {
                let def_re =
                    Regex::new(r"^\s*(?:async\s+)?def\s+([a-zA-Z0-9_]+)\s*\(([^)]*)\)").unwrap();
                let class_re = Regex::new(r"^\s*class\s+([a-zA-Z0-9_]+)").unwrap();

                for (idx, line) in lines.iter().enumerate() {
                    let trimmed = line.trim();
                    if let Some(caps) = def_re.captures(trimmed) {
                        let name = caps.get(1).map(|m| m.as_str()).unwrap_or("");
                        symbols.push(Symbol {
                            name: name.to_string(),
                            kind: SymbolKind::Function,
                            file_path: file_path.to_string(),
                            line_number: idx + 1,
                            signature: trimmed.trim_end_matches(':').trim().to_string(),
                            doc_comment: Self::lookbehind_doc(&lines, idx),
                        });
                    } else if let Some(caps) = class_re.captures(trimmed) {
                        let name = caps.get(1).map(|m| m.as_str()).unwrap_or("");
                        symbols.push(Symbol {
                            name: name.to_string(),
                            kind: SymbolKind::Class,
                            file_path: file_path.to_string(),
                            line_number: idx + 1,
                            signature: name.to_string(),
                            doc_comment: Self::lookbehind_doc(&lines, idx),
                        });
                    }
                }
            }
            _ => {}
        }

        symbols
    }

    fn lookbehind_doc(lines: &[&str], curr_idx: usize) -> Option<String> {
        if curr_idx == 0 {
            return None;
        }
        let mut doc_lines = Vec::new();
        let mut idx = curr_idx;
        while idx > 0 {
            idx -= 1;
            let l = lines[idx].trim();
            if l.starts_with("///") || l.starts_with("//!") || l.starts_with('#') {
                doc_lines.push(l.trim_start_matches(['/', '!', '#']).trim().to_string());
            } else {
                break;
            }
        }
        if doc_lines.is_empty() {
            None
        } else {
            doc_lines.reverse();
            Some(doc_lines.join(" "))
        }
    }

    /// Search for symbols by query substring.
    pub fn search_symbols<'a>(map: &'a RepositoryMap, query: &str) -> Vec<&'a Symbol> {
        let q = query.to_lowercase();
        let mut results = Vec::new();
        for symbols in map.file_symbols.values() {
            for sym in symbols {
                if sym.name.to_lowercase().contains(&q) || sym.signature.to_lowercase().contains(&q)
                {
                    results.push(sym);
                }
            }
        }
        results
    }
}
