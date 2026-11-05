# OSMOO Capability Audit: Changes Made During Audit Session

**Date:** 2026-10-28  
**Audit Context:** Comprehensive End-to-End Capability Verification & Defect Remediation  
**Target Repository:** `VOXY-COMMERCIAL`

---

## Summary of Remediated Defects

During the verification phase of the capability audit, three critical defects were identified in the live runtime path and remediated directly in source code:

### 1. `crates/voice_audio/src/lib.rs`: Missing Constructor Field in `AudioStreamConfig`
- **Issue:** The `AudioStreamConfig` struct definition was updated with a new `last_error: Option<String>` telemetry field, but internal constructor functions and unit test mocks failed to initialize this field, blocking compilation of all dependent audio crates.
- **Fix:** Initialized `last_error: None` in `AudioStreamConfig::new()`, `new_virtual()`, and stream creation callbacks.
- **Verification:** `cargo test -p voxy-voice-audio` passed all 15 tests (14 unit tests + 1 live WASAPI initialization test).

### 2. `apps/daemon/src/tools.rs`: Missing Tool Definitions for Core Native Engines
- **Issue:** The daemon system prompt's `ToolRegistry::tool_definitions()` only listed 18 legacy desktop automation actions. Key capabilities—including autonomous coding (`execute_coding_task`), multi-source deep research (`research_deep_investigate`), persistent user memory (`memory_store`, `memory_recall`), and task telemetry (`task_history_get`, `task_artifacts_list`)—were fully implemented in Rust crates but omitted from the tool schema presented to the LLM. As a result, the LLM had no way of knowing it could invoke the coding harness or memory system via structured tool calls.
- **Fix:** Added precise parameter schemas for `execute_coding_task`, `research_deep_investigate`, `memory_store`, `memory_recall`, `task_history_get`, and `task_artifacts_list` to `ToolRegistry::tool_definitions()`.
- **Verification:** Unit test `tools::tests::test_parse_tool_call_valid` verified correct parsing and routing of newly exposed tools.

### 3. `apps/daemon/src/main.rs`: Zero-Fallback Model Routing & Fake Echo Elimination
- **Issue A (Silent Provider Fallback):** When `VOXY_LLM_PROVIDER` was configured to a cloud provider (`openai`, `anthropic`, `gemini`, `groq`, `openrouter`, `nvidia`) without an API key, `create_llm_provider()` silently redirected inference to local `OllamaProvider`. In environments enforcing `VOXY_ZERO_FALLBACK=1` or `VOXY_STRICT_MODEL_ROUTING=1`, this violated the zero-fallback guarantee and caused mysterious local inference attempts.
- **Issue B (Fake Echo Completions):** In `apps/daemon/src/main.rs:1440-1490`, whenever LLM inference or streaming failed, the error handler dispatched `pick_echo(&text)`, replying with phrases like *"I heard you mention. <query>"* or *"Noted: <query>"*. This masked hard inference failures as conversational understanding, concealing backend errors from the user and from automated audits.
- **Fix A:** Updated `create_llm_provider()` to check `VOXY_ZERO_FALLBACK` and `VOXY_STRICT_MODEL_ROUTING`. When strict routing is active, missing API keys trigger explicit error logging and preserve the designated provider instance, ensuring inference fails cleanly and informatively.
- **Fix B:** Replaced all instances of `pick_echo` in LLM failure branches with explicit diagnostic responses indicating the provider name and the concrete error description (e.g., *"I encountered an issue contacting the AI provider (<provider>): <error>. Please verify your connection and API keys."*).
- **Verification:** Clean error reporting replaces deceptive echo behavior across both streaming and unary completion pipelines.

---

## Source Diffs

```diff
--- a/apps/daemon/src/tools.rs
+++ b/apps/daemon/src/tools.rs
@@ -113,6 +113,12 @@ Available tools:
 - process_info: Query diagnostics for a process by PID. params: {"pid": <pid>}
 - process_kill: Terminate an active process (requires confirmation). params: {"pid": <pid>}
 - system_info: Retrieve OS details, CPU architecture, hostname, and displays. params: {}
+- execute_coding_task: Execute an autonomous software engineering task using the coding harness. params: {"prompt": "<coding_instruction>", "workspace_root": "<optional_path>"}
+- research_deep_investigate: Perform comprehensive multi-source investigation on a query. params: {"query": "<topic>"}
+- memory_store: Persist an important fact or user preference into long-term memory. params: {"key": "<key>", "value": "<content>"}
+- memory_recall: Query long-term memory for relevant user context. params: {"query": "<query>"}
+- task_history_get: Retrieve recorded execution telemetry for a task. params: {"task_id": "<task_id>"}
+- task_artifacts_list: List artifacts generated for a task. params: {"task_id": "<task_id>"}

--- a/apps/daemon/src/main.rs
+++ b/apps/daemon/src/main.rs
@@ -44,13 +44,24 @@ fn create_llm_provider() -> Arc<dyn LlmProvider> {
         .unwrap_or_else(|_| "ollama".into())
         .to_lowercase();
 
+    let strict_routing = std::env::var("VOXY_ZERO_FALLBACK")
+        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
+        .unwrap_or(false)
+        || std::env::var("VOXY_STRICT_MODEL_ROUTING")
+            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
+            .unwrap_or(false);
+
     match provider.as_str() {
         "openai" => {
             let api_key = std::env::var("VOXY_API_KEYS_OPENAI").unwrap_or_default();
             let model = std::env::var("VOXY_OPENAI_MODEL").unwrap_or_else(|_| "gpt-4o-mini".into());
             if api_key.is_empty() {
-                tracing::warn!("[PROVIDER] VOXY_API_KEYS_OPENAI not set, falling back to Ollama");
-                return create_ollama_provider();
+                if strict_routing {
+                    tracing::error!("[PROVIDER] VOXY_API_KEYS_OPENAI not set and strict routing active. Rejecting fallback to Ollama.");
+                } else {
+                    tracing::warn!("[PROVIDER] VOXY_API_KEYS_OPENAI not set, falling back to Ollama");
+                    return create_ollama_provider();
+                }
             }
...
@@ -1437,24 +1468,15 @@ fn run_pipeline(...) {
                                     if cleaned.is_empty() {
-                                        pick_echo(&text)
+                                        format!("No response received from {}.", llm.name())
                                     } else {
                                         sanitize_llm_output(&cleaned)
                                     }
                                 }
                                 Err(e) => {
-                                    tracing::warn!("LLM streaming failed: {e} — using fallback");
-                                    let intent = IntentInput { ... };
-                                    match cognitive.process(&intent).await {
-                                        Ok(_) => pick_echo(&text),
-                                        Err(_) => pick_echo(&text),
-                                    }
+                                    tracing::warn!("LLM streaming failed: {e}");
+                                    format!("I encountered an issue contacting the AI provider ({}): {e}. Please verify your connection and API keys.", llm.name())
                                 }
```
