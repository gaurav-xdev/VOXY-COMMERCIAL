# PHASE 2.5 — ARCHITECTURE & DESIGN SPECIFICATION
## Deep Research & Synthesis Engine for OSMOO

### 1. Executive Summary & Purpose
The Deep Research & Synthesis Engine transforms OSMOO from a single-query look-up assistant into an autonomous, evidence-driven research investigator.
Rather than blindly calling web search or dumping unverified web pages into prompts, the engine executes a structured, multi-stage research lifecycle:
```
USER GOAL 
  ──> 1. Research Planning & Decomposition (Sub-Questions)
  ──> 2. Source Discovery & Deduplication (Browser/Web/Doc/Files)
  ──> 3. Content Extraction & HTML/Text Normalization
  ──> 4. Prompt-Injection Boundary & Untrusted Data Quarantine
  ──> 5. Source Quality & Credibility Scoring (Primary vs Secondary, Domain, Freshness)
  ──> 6. Cross-Source Comparison & Contradiction Detection
  ──> 7. Structured Synthesis & Verified Citations
  ──> 8. Long-Term Research Memory Ingestion (with Provenance)
```

---

### 2. Subsystem Architecture

```
                  +----------------------------------------------+
                  |         AgentRole::Researcher / User         |
                  +----------------------+-----------------------+
                                         |
                                         v
                         +-------------------------------+
                         |      ResearchOrchestrator     |
                         |  (State Machine & Step Budget)|
                         +---------------+---------------+
                                         |
            +----------------------------+----------------------------+
            |                            |                            |
            v                            v                            v
  +-------------------+        +-------------------+        +-------------------+
  | QueryDecomposer   |        | SourceRetriever   |        | ContentExtractor  |
  | & Plan Graph      |        | & Deduplicator    |        | (Html/Text/Code)  |
  +-------------------+        +-------------------+        +-------------------+
            |                            |                            |
            +----------------------------+----------------------------+
                                         |
                                         v
                      +--------------------------------------+
                      |      PromptInjectionDefenses         |
                      |  - Data vs Instruction Boundary      |
                      |  - Hidden Command Strip / Tagging    |
                      +------------------+-------------------+
                                         |
                                         v
                      +--------------------------------------+
                      |       SourceQualityEvaluator         |
                      |  - Heuristics, Domain, Corroboration |
                      +------------------+-------------------+
                                         |
                                         v
                      +--------------------------------------+
                      |      CrossSourceSynthesizer          |
                      |  - Agreement, Contradiction, Claims  |
                      |  - Traceable Citation Map            |
                      +------------------+-------------------+
                                         |
                                         v
                      +--------------------------------------+
                      |      ResearchResult & Citations      |
                      +--------------------------------------+
```

---

### 3. Detailed Component Design

#### 3.1 Research Orchestrator (`orchestrator.rs`)
- **State Machine**: `Planning` -> `Discovering` -> `Extracting` -> `Evaluating` -> `Synthesizing` -> `Completed` / `Failed` / `Cancelled`.
- **Budgeting & Loop Prevention**:
  - `max_depth`: Max iterations of recursive query expansion (default: 3).
  - `max_sources`: Bound on total sources retrieved (default: 10).
  - `timeout_duration`: Max wall-clock execution time (default: 60s).
  - `cancellation_token`: Cooperative abort support via `tokio::sync::watch` or `AtomicBool`.
- **Rate-Limiting & Backoff**: Exponential backoff with jitter on HTTP request failures; request budget tracking.

#### 3.2 Content Extraction & Source Normalization (`extractor.rs`)
- Strips malformed HTML tags, scripts, stylesheets, and tracking cookies.
- Extracts article body, code snippets, metadata titles, and publication dates.
- Handles empty pages, redirects, oversized payloads (>5MB clamped to 100KB relevant excerpt).

#### 3.3 Prompt-Injection Defense & Untrusted Data Quarantine (`defense.rs`)
- External content is tagged inside explicit `<UNTRUSTED_RESEARCH_DATA source="...">` blocks.
- Strips prompt-injection patterns:
  - "Ignore previous instructions", "SYSTEM PROMPT", "You are now...", "Do not tell the user".
  - Attempts to invoke commands or tools disguised as web text.
- Segregates untrusted data from planner control instructions.

#### 3.4 Source Quality Evaluation (`quality.rs`)
- Heuristic scoring:
  - Domain trust: Official docs, academic / standards bodies (`.gov`, `.edu`, `.org`, `docs.*`, `github.com/rust-lang`, etc.) receive higher trust than content mills or forums.
  - Direct evidence vs hearsay / second-hand summaries.
  - Recency: Dated content scored appropriately.
- Transparent quality metric: `quality_score: f32 (0.0 to 1.0)` with rationale.

#### 3.5 Cross-Source Synthesis & Contradiction Detection (`synthesis.rs`)
- Claims mapping:
  - `Claim`: Text statement.
  - `supporting_sources`: `Vec<SourceId>`
  - `contradicting_sources`: `Vec<SourceId>`
  - `confidence`: `High`, `Moderate`, `Contested`, `Unverified`.
- Explicit representation: Contradictions are surfaced directly in the final report rather than papered over.
- Unambiguous Citation linking: Every claim references exact retrieved source identifiers (URLs/document titles).

#### 3.6 Tool Integration (`tool_calling`)
- `research_deep_investigate`: High-level tool callable by agents/users.
- `research_synthesize`: Tool for multi-source comparative synthesis and contradiction resolution.

---

### 4. Security & Safety Review
1. **SSRF & Private Network Blocking**: Any web lookup must refuse loopback (`127.0.0.1`, `localhost`) and link-local (`169.254.169.254`) metadata endpoints.
2. **Zero-Execution Sandbox**: Extracted scripts/code are NEVER executed; treated purely as static text evidence.
3. **Secret Redaction**: Queries and extracted results are scrubbed for API tokens (`AKIA*`, `sk-*`, `ghp_*`) to prevent prompt leakage.
4. **ApprovalBroker Alignment**: Web browsing and external fetching follow security tiers (`RiskTier::Read` / `LowRisk`).
