//! Repository Discovery, Indexing & Semantic Symbol Search.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolInfo {
    pub name: String,
    pub kind: String, // "fn", "struct", "enum", "trait", "impl"
    pub file_path: PathBuf,
    pub line_number: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileMetadata {
    pub relative_path: PathBuf,
    pub absolute_path: PathBuf,
    pub extension: String,
    pub size_bytes: u64,
}

pub struct RepositoryIndexer {
    root_path: PathBuf,
    files: Vec<FileMetadata>,
    symbol_index: HashMap<String, Vec<SymbolInfo>>,
}

impl RepositoryIndexer {
    pub fn new(root: impl AsRef<Path>) -> Self {
        Self {
            root_path: root.as_ref().to_path_buf(),
            files: Vec::new(),
            symbol_index: HashMap::new(),
        }
    }

    /// Indexes the repository, skipping ignored directories (target, .git, node_modules).
    pub fn index_repository(&mut self) -> Result<usize, std::io::Error> {
        self.files.clear();
        self.symbol_index.clear();

        let walker = WalkDir::new(&self.root_path)
            .follow_links(false)
            .max_depth(10)
            .into_iter()
            .filter_entry(|e| {
                let name = e.file_name().to_string_lossy();
                name != ".git" && name != "target" && name != "node_modules" && name != ".idea"
            });

        for entry in walker.filter_map(|e| e.ok()) {
            if entry.file_type().is_file() {
                let path = entry.path().to_path_buf();
                let ext = path
                    .extension()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_default();

                let metadata = entry.metadata()?;
                let relative = path
                    .strip_prefix(&self.root_path)
                    .unwrap_or(&path)
                    .to_path_buf();

                self.files.push(FileMetadata {
                    relative_path: relative,
                    absolute_path: path.clone(),
                    extension: ext.clone(),
                    size_bytes: metadata.len(),
                });

                if matches!(ext.as_str(), "rs" | "toml" | "py" | "ts" | "js" | "tsx" | "jsx" | "go" | "ps1") {
                    self.extract_symbols(&path);
                }
            }
        }

        Ok(self.files.len())
    }

    fn extract_symbols(&mut self, file_path: &Path) {
        let content = match std::fs::read_to_string(file_path) {
            Ok(c) => c,
            Err(_) => return,
        };

        let ext = file_path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        for (idx, line) in content.lines().enumerate() {
            let line_trimmed = line.trim();
            if line_trimmed.is_empty() || line_trimmed.starts_with("//") || line_trimmed.starts_with('#') {
                continue;
            }

            match ext.as_str() {
                "rs" => {
                    let keywords = ["pub fn ", "fn ", "pub struct ", "struct ", "pub enum ", "enum ", "pub trait ", "trait ", "pub type ", "type "];
                    for kw in keywords {
                        if let Some(pos) = line_trimmed.find(kw) {
                            let after = &line_trimmed[pos + kw.len()..];
                            let sym_name = after
                                .split(|c: char| !c.is_alphanumeric() && c != '_')
                                .next()
                                .unwrap_or_default();
                            if !sym_name.is_empty() {
                                let kind = kw.trim_start_matches("pub ").trim();
                                self.add_symbol(sym_name, kind, file_path, idx + 1);
                                break;
                            }
                        }
                    }
                }
                "ts" | "js" | "tsx" | "jsx" => {
                    let keywords = [
                        "export function ", "function ",
                        "export class ", "class ",
                        "export interface ", "interface ",
                        "export type ", "type ",
                        "export const ", "const ",
                    ];
                    for kw in keywords {
                        if let Some(pos) = line_trimmed.find(kw) {
                            let after = &line_trimmed[pos + kw.len()..];
                            let sym_name = after
                                .split(|c: char| !c.is_alphanumeric() && c != '_')
                                .next()
                                .unwrap_or_default();
                            if !sym_name.is_empty() {
                                let kind = kw.trim_start_matches("export ").trim();
                                self.add_symbol(sym_name, kind, file_path, idx + 1);
                                break;
                            }
                        }
                    }
                }
                "py" => {
                    let keywords = ["def ", "async def ", "class "];
                    for kw in keywords {
                        if line_trimmed.starts_with(kw) {
                            let after = &line_trimmed[kw.len()..];
                            let sym_name = after
                                .split(|c: char| !c.is_alphanumeric() && c != '_')
                                .next()
                                .unwrap_or_default();
                            if !sym_name.is_empty() {
                                self.add_symbol(sym_name, kw.trim(), file_path, idx + 1);
                                break;
                            }
                        }
                    }
                }
                "go" => {
                    let keywords = ["func ", "type "];
                    for kw in keywords {
                        if line_trimmed.starts_with(kw) {
                            let after = &line_trimmed[kw.len()..];
                            let sym_name = after
                                .split(|c: char| !c.is_alphanumeric() && c != '_')
                                .next()
                                .unwrap_or_default();
                            if !sym_name.is_empty() {
                                self.add_symbol(sym_name, kw.trim(), file_path, idx + 1);
                                break;
                            }
                        }
                    }
                }
                "ps1" => {
                    let keywords = ["function ", "filter ", "class "];
                    for kw in keywords {
                        if line_trimmed.to_lowercase().starts_with(kw) {
                            let after = &line_trimmed[kw.len()..];
                            let sym_name = after
                                .split(|c: char| !c.is_alphanumeric() && c != '-' && c != '_')
                                .next()
                                .unwrap_or_default();
                            if !sym_name.is_empty() {
                                self.add_symbol(sym_name, kw.trim(), file_path, idx + 1);
                                break;
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }

    fn add_symbol(&mut self, name: &str, kind: &str, file_path: &Path, line: usize) {
        let sym = SymbolInfo {
            name: name.to_string(),
            kind: kind.to_string(),
            file_path: file_path.to_path_buf(),
            line_number: line,
        };
        self.symbol_index
            .entry(name.to_string())
            .or_default()
            .push(sym);
    }

    /// Queries symbols by exact or partial name.
    pub fn find_symbols(&self, query: &str) -> Vec<SymbolInfo> {
        let query_lower = query.to_lowercase();
        let mut results = Vec::new();
        for (name, syms) in &self.symbol_index {
            if name.to_lowercase().contains(&query_lower) {
                results.extend(syms.clone());
            }
        }
        results
    }

    /// Lists indexed files matching a file extension.
    pub fn find_files_by_extension(&self, ext: &str) -> Vec<&FileMetadata> {
        self.files
            .iter()
            .filter(|f| f.extension.eq_ignore_ascii_case(ext))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_harness_repository_discovery() {
        let dir = tempdir().unwrap();
        let root = dir.path();

        // Create dummy repo files
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(
            root.join("src").join("main.rs"),
            "pub struct ServerConfig {}\npub fn run_server() {}\n",
        )
        .unwrap();

        let mut indexer = RepositoryIndexer::new(root);
        let count = indexer.index_repository().unwrap();
        assert!(count >= 1);

        let structs = indexer.find_symbols("ServerConfig");
        assert_eq!(structs.len(), 1);
        assert_eq!(structs[0].kind, "struct");
        assert_eq!(structs[0].line_number, 1);

        let funcs = indexer.find_symbols("run_server");
        assert_eq!(funcs.len(), 1);
        assert_eq!(funcs[0].kind, "fn");
    }

    #[test]
    fn test_harness_multilang_symbol_extraction() {
        let dir = tempdir().unwrap();
        let root = dir.path();

        // TS file
        std::fs::write(
            root.join("api.ts"),
            "export interface UserProfile { id: string; }\nexport function fetchUser() {}\n",
        ).unwrap();

        // Python file
        std::fs::write(
            root.join("worker.py"),
            "class TaskQueue:\n    pass\n\ndef process_job():\n    pass\n",
        ).unwrap();

        let mut indexer = RepositoryIndexer::new(root);
        indexer.index_repository().unwrap();

        let interfaces = indexer.find_symbols("UserProfile");
        assert_eq!(interfaces.len(), 1);
        assert_eq!(interfaces[0].kind, "interface");

        let ts_fn = indexer.find_symbols("fetchUser");
        assert_eq!(ts_fn.len(), 1);
        assert_eq!(ts_fn[0].kind, "function");

        let py_class = indexer.find_symbols("TaskQueue");
        assert_eq!(py_class.len(), 1);
        assert_eq!(py_class[0].kind, "class");

        let py_fn = indexer.find_symbols("process_job");
        assert_eq!(py_fn.len(), 1);
        assert_eq!(py_fn[0].kind, "def");
    }
}
