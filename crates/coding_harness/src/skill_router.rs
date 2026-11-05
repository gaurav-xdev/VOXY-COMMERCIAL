use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::Path;
use walkdir::WalkDir;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillSuggestion {
    pub name: String,
    pub description: String,
    pub matched_triggers: Vec<String>,
}

pub struct SkillRouter;

impl SkillRouter {
    pub fn suggest_skills_for_file(file_path: &Path) -> Vec<SkillSuggestion> {
        let mut suggestions = Vec::new();
        let ext = file_path.extension().and_then(|e| e.to_str()).unwrap_or("");

        match ext {
            "rs" => {
                suggestions.push(SkillSuggestion {
                    name: "clean-code".into(),
                    description:
                        "Rust clean code invariants, ownership patterns, and clippy alignment"
                            .into(),
                    matched_triggers: vec!["rs".into(), "rust".into()],
                });
                suggestions.push(SkillSuggestion {
                    name: "tdd-workflow".into(),
                    description: "Test-Driven Development RED-GREEN-REFACTOR cycle".into(),
                    matched_triggers: vec!["rs".into(), "tests".into()],
                });
            }
            "ts" | "tsx" => {
                suggestions.push(SkillSuggestion {
                    name: "typescript-expert".into(),
                    description: "TypeScript compiler strict mode, types, and modern patterns"
                        .into(),
                    matched_triggers: vec!["ts".into(), "tsx".into()],
                });
            }
            "js" | "jsx" => {
                suggestions.push(SkillSuggestion {
                    name: "javascript-pro".into(),
                    description: "Modern JavaScript ECMAScript patterns and runtime safety".into(),
                    matched_triggers: vec!["js".into()],
                });
            }
            "py" => {
                suggestions.push(SkillSuggestion {
                    name: "python-pro".into(),
                    description: "Python 3 typing, pytest fixtures, and clean architecture".into(),
                    matched_triggers: vec!["py".into()],
                });
            }
            "go" => {
                suggestions.push(SkillSuggestion {
                    name: "golang-pro".into(),
                    description: "Go concurrency, error wrapping, and idiomatic APIs".into(),
                    matched_triggers: vec!["go".into()],
                });
            }
            "java" => {
                suggestions.push(SkillSuggestion {
                    name: "java-pro".into(),
                    description: "Java JDK 21 modern records, pattern matching, and JUnit 5".into(),
                    matched_triggers: vec!["java".into()],
                });
            }
            "c" | "cpp" | "h" | "hpp" => {
                suggestions.push(SkillSuggestion {
                    name: "c-pro".into(),
                    description: "Clang/C++ memory safety, ASAN validation, and RAII".into(),
                    matched_triggers: vec!["c".into(), "cpp".into()],
                });
            }
            "sql" => {
                suggestions.push(SkillSuggestion {
                    name: "sql-pro".into(),
                    description: "SQL transactions, ACID constraints, and query planning".into(),
                    matched_triggers: vec!["sql".into()],
                });
            }
            _ => {}
        }

        suggestions
    }

    pub fn suggest_skills_for_changes(files: &[&Path]) -> Vec<SkillSuggestion> {
        let mut seen = HashSet::new();
        let mut all_suggestions = Vec::new();

        for file in files {
            for skill in Self::suggest_skills_for_file(file) {
                if seen.insert(skill.name.clone()) {
                    all_suggestions.push(skill);
                }
            }
        }

        all_suggestions
    }

    /// Discover installed skills with a `SKILL.md` under any given search roots.
    /// Traverses up to depth 5 to support nested plugin layouts, e.g.:
    /// `.gemini/config/plugins/<plugin>/skills/<skill>/SKILL.md`
    /// or `<workspace>/.agents/skills/<skill>/SKILL.md`.
    pub fn discover_installed_skills(search_roots: &[&Path]) -> Vec<SkillSuggestion> {
        let mut discovered = Vec::new();
        let mut seen = HashSet::new();

        for root in search_roots {
            if !root.exists() {
                continue;
            }

            for entry in WalkDir::new(root)
                .max_depth(5)
                .follow_links(false)
                .into_iter()
                .filter_map(|e| e.ok())
            {
                if entry.file_type().is_file() {
                    let file_name = entry.file_name().to_string_lossy();
                    if file_name.eq_ignore_ascii_case("SKILL.md") {
                        if let Some(parent) = entry.path().parent() {
                            let name = parent
                                .file_name()
                                .and_then(|n| n.to_str())
                                .unwrap_or("unknown")
                                .to_string();

                            if seen.insert(name.clone()) {
                                let desc = Self::extract_description_from_skill_file(entry.path())
                                    .unwrap_or_else(|| format!("Engineering skill '{}'", name));
                                discovered.push(SkillSuggestion {
                                    name,
                                    description: desc,
                                    matched_triggers: vec!["installed".into()],
                                });
                            }
                        }
                    }
                }
            }
        }

        discovered
    }

    /// Load the raw instruction content of a skill from disk across search roots,
    /// searching both direct and nested plugin hierarchies.
    pub fn load_skill_instructions(skill_name: &str, search_roots: &[&Path]) -> Option<String> {
        for root in search_roots {
            if !root.exists() {
                continue;
            }

            for entry in WalkDir::new(root)
                .max_depth(5)
                .follow_links(false)
                .into_iter()
                .filter_map(|e| e.ok())
            {
                if entry.file_type().is_file() {
                    let file_name = entry.file_name().to_string_lossy();
                    if file_name.eq_ignore_ascii_case("SKILL.md") {
                        if let Some(parent) = entry.path().parent() {
                            if let Some(dir_name) = parent.file_name().and_then(|n| n.to_str()) {
                                if dir_name.eq_ignore_ascii_case(skill_name) {
                                    if let Ok(content) = std::fs::read_to_string(entry.path()) {
                                        return Some(content);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        None
    }

    /// Select skills dynamically based on task prompt text and relevance scoring.
    pub fn select_skills_for_prompt(prompt: &str, search_roots: &[&Path]) -> Vec<SkillSuggestion> {
        let all_skills = Self::discover_installed_skills(search_roots);
        let prompt_lower = prompt.to_lowercase();
        let mut scored = Vec::new();

        for skill in all_skills {
            let name_lower = skill.name.to_lowercase();
            let desc_lower = skill.description.to_lowercase();
            let mut score = 0usize;

            // 1. Direct name match
            if prompt_lower.contains(&name_lower) {
                score += 15;
            }

            // 2. Token match on name (split by non-alphanumeric, e.g. hyphens)
            for token in name_lower.split(|c: char| !c.is_alphanumeric()) {
                if token.len() >= 3 {
                    if prompt_lower.contains(token) {
                        score += 5;
                    } else {
                        // Match prefix or stem (e.g. "audit" matches "auditor")
                        for pw in prompt_lower.split(|c: char| !c.is_alphanumeric()) {
                            if pw.len() >= 4 && (token.starts_with(pw) || pw.starts_with(token)) {
                                score += 3;
                                break;
                            }
                        }
                    }
                }
            }

            // 3. Match against description tokens (split by non-alphanumeric)
            for word in desc_lower.split(|c: char| !c.is_alphanumeric()) {
                if word.len() >= 4 && prompt_lower.contains(word) {
                    score += 2;
                }
            }

            if score > 0 {
                scored.push((score, skill));
            }
        }

        scored.sort_by_key(|a| std::cmp::Reverse(a.0));
        scored.into_iter().map(|(_, s)| s).take(3).collect()
    }

    /// Extract description from frontmatter or first paragraph of SKILL.md.
    fn extract_description_from_skill_file(path: &Path) -> Option<String> {
        let content = std::fs::read_to_string(path).ok()?;
        let lines: Vec<&str> = content.lines().collect();
        if lines.first().map(|l| l.trim()) == Some("---") {
            let mut desc_lines = Vec::new();
            let mut in_desc = false;
            for line in lines.iter().skip(1) {
                let trimmed = line.trim();
                if trimmed == "---" {
                    break;
                }
                if trimmed.starts_with("description:") {
                    in_desc = true;
                    let rest = trimmed.trim_start_matches("description:").trim();
                    if !rest.is_empty() && rest != ">" && rest != "|" {
                        desc_lines.push(rest);
                    }
                } else if in_desc {
                    if trimmed.contains(':') && !trimmed.starts_with('-') {
                        break;
                    }
                    if !trimmed.is_empty() {
                        desc_lines.push(trimmed);
                    }
                }
            }
            if !desc_lines.is_empty() {
                return Some(desc_lines.join(" "));
            }
        }

        // Fallback: return first non-header non-empty line
        lines
            .into_iter()
            .find(|l| !l.trim().is_empty() && !l.starts_with('#') && !l.starts_with("---"))
            .map(|l| l.trim().to_string())
    }

    /// Compose an executable prompt injection block containing the full rules of the active skills.
    pub fn compose_skill_system_prompt(
        skills: &[SkillSuggestion],
        search_roots: &[&Path],
    ) -> String {
        let mut prompt = String::new();
        if skills.is_empty() {
            return prompt;
        }

        prompt.push_str("\n<active_engineering_skills>\n");
        for skill in skills {
            prompt.push_str(&format!("## Skill: {}\n", skill.name));
            prompt.push_str(&format!("Description: {}\n", skill.description));
            if let Some(instructions) = Self::load_skill_instructions(&skill.name, search_roots) {
                prompt.push_str("### Instructions & Constraints:\n");
                prompt.push_str(&instructions);
                prompt.push_str("\n\n");
            } else {
                prompt.push_str("(Using standard engineering invariants for this domain)\n\n");
            }
        }
        prompt.push_str("</active_engineering_skills>\n");
        prompt
    }
}
