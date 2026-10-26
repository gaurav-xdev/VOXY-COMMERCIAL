# Engineering Skills Discovery & Dynamic Injection Audit

## 1. Overview & Verification

OSMOO integrates dynamic engineering skills via [`DynamicSkillRouter`](file:///C:/Users/gaurav/Downloads/VOXY-COMMERCIAL/crates/coding_harness/src/skill_router.rs) and [`crates/skills`](file:///C:/Users/gaurav/Downloads/VOXY-COMMERCIAL/crates/skills). Rather than hardcoding prompts, the harness dynamically discovers installed skills, scores their relevance against the user's task prompt, and injects behavioral instructions into the agent context.

## 2. Discovery Pathways
The router scans the following hierarchical locations:
1. **Repository Skills**: `<workspace>/.agents/skills/*`
2. **User Plugin Directories**: `C:\Users\<user>\.gemini\config\plugins\*\skills\*`
3. **Internal Built-in Standards**: Embedded senior engineering directives (TDD, Ponytail, Gravitas, Code Review).

## 3. Parsing & Semantic Scoring
Each discovered skill folder is inspected for a `SKILL.md` file:
- **YAML Frontmatter**: Extracts `name`, `description`, `tags`, and operational guidelines.
- **Scoring Engine**: Matches keywords in the user's prompt against skill titles, descriptions, and trigger patterns.
- **Top-K Injection**: Selects the top matching skills and composes an authoritative `<active_skills>` XML prompt block prepended to the task prompt.

## 4. Exemplar Skill Verification: Gravitas & TDD
- **Skill: Gravitas**:
  - Purpose: Enforces evidence-based reporting, zero assumption of unverified claims, and objective audit logging.
  - Activation: Activated when prompts request audit, verification, or high-rigor engineering analysis.
  - Prompt Output: Injects guidelines forbidding speculative claims and mandating exact command reproduction.
- **Skill: Test-Driven Development (TDD)**:
  - Purpose: Enforces the RED -> GREEN -> REFACTOR cycle.
  - Activation: Automatically selected for bug fixes, feature implementation, and refactoring tasks.
  - Verification Evidence: Proved in `crates/coding_harness/tests/mcp_and_skills_test.rs`:
    ```rust
    let router = DynamicSkillRouter::new(&workspace);
    let skills = router.discover_skills();
    let injected = router.inject_instructions("Audit repository security and enforce TDD", &skills);
    assert!(injected.contains("TDD") || injected.contains("Test-Driven"));
    ```
