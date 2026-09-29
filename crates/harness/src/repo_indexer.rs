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

                if matches!(ext.as_str(), "rs" | "toml" | "py" | "ts" | "js") {
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

        for (idx, line) in content.lines().enumerate() {
            let line_trimmed = line.trim();
            let keywords = ["fn ", "struct ", "enum ", "trait ", "type "];
            for kw in keywords {
                if let Some(pos) = line_trimmed.find(kw) {
                    let after = &line_trimmed[pos + kw.len()..];
                    let sym_name = after
                        .split(|c: char| !c.is_alphanumeric() && c != '_')
                        .next()
                        .unwrap_or_default();

                    if !sym_name.is_empty() {
                        let sym = SymbolInfo {
                            name: sym_name.to_string(),
                            kind: kw.trim().to_string(),
                            file_path: file_path.to_path_buf(),
                            line_number: idx + 1,
                        };
                        self.symbol_index
                            .entry(sym_name.to_string())
                            .or_default()
                            .push(sym);
                    }
                }
            }
        }
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
}
