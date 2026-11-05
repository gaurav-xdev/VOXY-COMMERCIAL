use crate::symbol_index::{RepositoryMap, Symbol};
use crate::types::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetConfig {
    pub max_prompt_tokens: usize,
    pub max_repo_map_tokens: usize,
    pub max_file_context_tokens: usize,
    pub reserve_completion_tokens: usize,
}

impl Default for BudgetConfig {
    fn default() -> Self {
        Self {
            max_prompt_tokens: 16_000,
            max_repo_map_tokens: 2_000,
            max_file_context_tokens: 8_000,
            reserve_completion_tokens: 4_000,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EfficiencyMetrics {
    pub prompt_tokens_estimated: usize,
    pub tokens_saved_by_compression: usize,
    pub repo_map_tokens: usize,
    pub files_indexed: usize,
    pub symbols_provided: usize,
    pub cache_hits: usize,
}

pub struct TokenEstimator;

impl TokenEstimator {
    /// Heuristic token estimator for source code and English text.
    /// Code has higher token density (~3.5-3.8 chars per token); English prose has ~4 chars per token.
    pub fn estimate_tokens(text: &str) -> usize {
        if text.is_empty() {
            return 0;
        }

        let char_count = text.chars().count();
        let word_count = text.split_whitespace().count();

        // Blended estimation: (chars / 3.8 + words * 1.2) / 2
        let est = ((char_count as f32 / 3.8) + (word_count as f32 * 1.2)) / 2.0;
        est.ceil() as usize
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelectedContextChunk {
    pub file_path: String,
    pub start_line: usize,
    pub end_line: usize,
    pub content: String,
    pub token_estimate: usize,
    pub relevance_score: usize,
}

pub struct TargetedContextSelector;

impl TargetedContextSelector {
    /// Select relevant symbol signatures and targeted file slices within a token budget.
    pub fn select_context(
        workspace_root: &Path,
        repo_map: &RepositoryMap,
        task_prompt: &str,
        budget: &BudgetConfig,
    ) -> Result<(String, EfficiencyMetrics)> {
        let mut metrics = EfficiencyMetrics::default();
        let prompt_lower = task_prompt.to_lowercase();
        let query_tokens: Vec<&str> = prompt_lower
            .split(|c: char| !c.is_alphanumeric())
            .filter(|s| s.len() >= 3)
            .collect();

        // 1. Build filtered repository map fitting max_repo_map_tokens
        let mut relevant_symbols: Vec<(&Symbol, usize)> = Vec::new();
        for symbols in repo_map.file_symbols.values() {
            for sym in symbols {
                let name_lower = sym.name.to_lowercase();
                let mut score = 0usize;

                for qt in &query_tokens {
                    if name_lower == *qt {
                        score += 20;
                    } else if name_lower.contains(qt) {
                        score += 10;
                    }
                }

                if score > 0 {
                    relevant_symbols.push((sym, score));
                }
            }
        }

        relevant_symbols.sort_by_key(|a| std::cmp::Reverse(a.1));

        let mut repo_map_str = String::new();
        let mut current_map_tokens = 0usize;

        let mut files_included = HashSet::new();
        for (sym, _) in &relevant_symbols {
            let line = format!("{}: {} {}\n", sym.file_path, sym.name, sym.signature);
            let cost = TokenEstimator::estimate_tokens(&line);
            if current_map_tokens + cost <= budget.max_repo_map_tokens {
                repo_map_str.push_str(&line);
                current_map_tokens += cost;
                files_included.insert(sym.file_path.clone());
                metrics.symbols_provided += 1;
            } else {
                break;
            }
        }
        metrics.repo_map_tokens = current_map_tokens;
        metrics.files_indexed = repo_map.file_symbols.len();

        // 2. Select targeted file excerpt chunks fitting max_file_context_tokens
        let mut excerpt_chunks: Vec<SelectedContextChunk> = Vec::new();
        let mut file_context_tokens = 0usize;

        for file_path in &files_included {
            let full_path = workspace_root.join(file_path);
            if let Ok(content) = fs::read_to_string(&full_path) {
                let lines: Vec<&str> = content.lines().collect();
                let total_lines = lines.len();

                // Find symbols in this file that matched
                let file_syms: Vec<&Symbol> = relevant_symbols
                    .iter()
                    .filter(|(s, _)| s.file_path == *file_path)
                    .map(|(s, _)| *s)
                    .collect();

                for sym in file_syms.iter().take(2) {
                    let start = sym.line_number.saturating_sub(5).max(1);
                    let end = (sym.line_number + 25).min(total_lines);

                    let slice_lines: Vec<&str> = lines
                        .iter()
                        .skip(start - 1)
                        .take(end - start + 1)
                        .copied()
                        .collect();

                    let slice_text = slice_lines.join("\n");
                    let cost = TokenEstimator::estimate_tokens(&slice_text);

                    if file_context_tokens + cost <= budget.max_file_context_tokens {
                        file_context_tokens += cost;
                        excerpt_chunks.push(SelectedContextChunk {
                            file_path: file_path.clone(),
                            start_line: start,
                            end_line: end,
                            content: slice_text,
                            token_estimate: cost,
                            relevance_score: 10,
                        });
                    }
                }
            }
        }

        // 3. Assemble compact context string
        let mut context_builder = String::new();
        if !repo_map_str.is_empty() {
            context_builder.push_str("<relevant_repository_symbols>\n");
            context_builder.push_str(&repo_map_str);
            context_builder.push_str("</relevant_repository_symbols>\n\n");
        }

        if !excerpt_chunks.is_empty() {
            context_builder.push_str("<relevant_file_excerpts>\n");
            for chunk in excerpt_chunks {
                context_builder.push_str(&format!(
                    "--- {} (lines {}-{}) ---\n{}\n\n",
                    chunk.file_path, chunk.start_line, chunk.end_line, chunk.content
                ));
            }
            context_builder.push_str("</relevant_file_excerpts>\n");
        }

        let total_est = TokenEstimator::estimate_tokens(&context_builder);
        metrics.prompt_tokens_estimated = total_est;

        // Estimate tokens saved compared to raw dump of all involved files
        let raw_total_chars: usize = files_included
            .iter()
            .filter_map(|f| fs::read_to_string(workspace_root.join(f)).ok())
            .map(|c| c.len())
            .sum();
        let raw_tokens = (raw_total_chars as f32 / 3.8).ceil() as usize;
        metrics.tokens_saved_by_compression = raw_tokens.saturating_sub(total_est);

        Ok((context_builder, metrics))
    }
}

pub struct ContextCompressor;

impl ContextCompressor {
    /// Deduplicate identical file outputs and merge overlapping ranges.
    pub fn deduplicate_chunks(mut chunks: Vec<SelectedContextChunk>) -> Vec<SelectedContextChunk> {
        if chunks.len() <= 1 {
            return chunks;
        }

        chunks.sort_by(|a, b| {
            a.file_path
                .cmp(&b.file_path)
                .then(a.start_line.cmp(&b.start_line))
        });

        let mut merged: Vec<SelectedContextChunk> = Vec::new();
        for chunk in chunks {
            if let Some(last) = merged.last_mut() {
                if last.file_path == chunk.file_path && chunk.start_line <= last.end_line + 5 {
                    // Merge overlapping or adjacent regions
                    last.end_line = last.end_line.max(chunk.end_line);
                    last.content = format!("{}\n{}", last.content, chunk.content);
                    last.token_estimate = TokenEstimator::estimate_tokens(&last.content);
                    continue;
                }
            }
            merged.push(chunk);
        }

        merged
    }
}
