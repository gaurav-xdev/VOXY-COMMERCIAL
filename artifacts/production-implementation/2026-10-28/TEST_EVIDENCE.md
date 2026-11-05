# OSMOO Test Evidence & Verification Ledger

**Date:** October 28, 2026  
**Operating System:** Windows 11 x64  
**Rust Toolchain:** `rustc 1.76+` (stable)  
**Execution Context:** Repository Root `C:\Users\gaurav\Downloads\VOXY-COMMERCIAL`  

---

## 1. Summary of Executed Test Suites

| Crate / Target | Test Command | Tests Run | Passed | Failed | Exit Code |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **`voxy-office`** | `cargo test -p voxy-office` | 13 | 13 | 0 | 0 |
| **`voxy-tool-calling` (unit)** | `cargo test -p voxy-tool-calling` | 22 | 22 | 0 | 0 |
| **`voxy-tool-calling` (integration)** | `cargo test -p voxy-tool-calling --test builtin_suite_test` | 4 | 4 | 0 | 0 |
| **`voxy-daemon` (unit + integration)** | `cargo test -p voxy-daemon` | 23 | 23 | 0 | 0 |
| **`voxy-coding-harness`** | `cargo test -p voxy-coding-harness` | 22 | 22 | 0 | 0 |
| **`voxy-model-router`** | `cargo test -p voxy-model-router` | 21 | 21 | 0 | 0 |
| **Total** | | **105** | **105** | **0** | **0** |

---

## 2. Test Execution Details by Crate

### 2.1 `voxy-office` (13/13 PASSED)
```text
running 13 tests
test tests::test_calendar_connector_conflict_detection ... ok
test tests::test_document_template_and_executive_report ... ok
test tests::test_filesystem_path_traversal_prevention ... ok
test tests::test_mail_connector_draft_and_approval_gate ... ok
test tests::test_office_connector_permissions ... ok
test tests::test_office_action_audit_trail ... ok
test tests::test_shopping_and_sandbox_payment_safety ... ok
test tests::test_spreadsheet_connector_parsing_and_aggregation ... ok
test tests::test_travel_connector_and_itinerary ... ok
test tests::test_persistent_sqlite_calendar_survives_reopen ... ok
test tests::test_docx_generation_and_validation ... ok
test tests::test_pptx_generation_and_validation ... ok
test tests::test_xlsx_generation_and_validation ... ok

test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.12s
```

### 2.2 `voxy-tool-calling` Unit Tests (22/22 PASSED)
```text
running 22 tests
test builtin::filesystem::tests::test_rollback_forged_and_expired_tokens_rejected ... ok
test tests::confirmation_gating_blocks_unconfirmed_destructive_tool ... ok
test tests::system_info_tool_executes_successfully ... ok
test tests::test_browser_download_path_traversal_and_dangerous_extensions ... ok
test builtin::filesystem::tests::test_device_and_unc_paths_rejected ... ok
test builtin::memory::tests::test_memory_store_rejects_secrets ... ok
test builtin::memory::tests::test_memory_store_recall_quarantine_and_isolation ... ok
test builtin::filesystem::tests::test_absolute_external_path_is_rejected ... ok
test tests::test_browser_prompt_injection_sanitization ... ok
test builtin::filesystem::tests::test_traversal_escaping_workspace_is_rejected ... ok
test tests::test_browser_tools_registered_in_registry ... ok
test builtin::filesystem::tests::test_workspace_relative_write_and_read_succeeds ... ok
test tests::test_research_deep_investigate_tool_execution ... ok
test tests::test_task_history_and_artifacts_tools ... ok
test tests::file_operations_flow_with_rollback ... ok
test builtin::filesystem::tests::test_rollback_restores_preexisting_file_bytes ... ok
test tests::tool_execution_with_approval_broker_approved ... ok
test tests::tool_execution_with_approval_broker_denied ... ok
test tests::test_browser_upload_sensitive_file_blocking ... ok
test tests::tool_registry_with_builtins ... ok
test tests::test_browser_runtime_emergency_stop ... ok
test tests::test_browser_url_validation_blocks_loopback_and_invalid_schemes ... ok

test result: ok. 22 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.34s
```

### 2.3 `voxy-tool-calling` Integration Suite (4/4 PASSED)
```text
running 4 tests
test test_mail_and_approval_workflow ... ok
test test_travel_shopping_and_sandbox_payments ... ok
test test_calendar_tools_flow ... ok
test test_office_tools_end_to_end ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.13s
```

### 2.4 `voxy-daemon` Tests (23/23 PASSED)
```text
running 16 tests
test tests::test_detect_cpu_features ... ok
test tests::test_detect_simd_no_panic ... ok
test tests::test_measure_stage_sync ... ok
test shutdown::tests::test_shutdown_creation ... ok
test shutdown::tests::test_execute_shutdown ... ok
test tests::test_get_memory_usage_no_panic ... ok
test shutdown::tests::test_register_with_flush ... ok
test shutdown::tests::test_register_simple ... ok
test tools::tests::test_emergency_stop_lifecycle ... ok
test tools::tests::test_emergency_stop_aborts_tool_execution ... ok
test tools::tests::test_parse_tool_call_none ... ok
test tools::tests::test_parse_tool_call_valid ... ok
test tools::tests::test_safe_launch_blocks_command_injection ... ok
test tests::conversation_memory_keeps_newest_when_over_budget ... ok
test tests::conversation_memory_trims_to_character_budget ... ok
test tests::conversation_memory_no_trim_with_large_budget ... ok

test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.08s

running 7 tests
test test_concurrent_channel_senders ... ok
test test_bounded_channel_backpressure ... ok
test test_cognitive_bridge_creation ... ok
test test_broadcast_channel_shutdown ... ok
test test_knowledge_validation ... ok
test test_reflection_engine_analysis ... ok
test test_runtime_guard_heartbeat ... ok

test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
```

### 2.5 `voxy-coding-harness` Tests (22/22 PASSED)
```text
running 10 tests
test tests::test_task_planner_dag ... ok
test tests::test_human_approval_flow ... ok
test tests::test_context_explorer_glob_and_grep ... ok
test tests::test_atomic_file_edit_success_and_conflict ... ok
test tests::test_token_budget_and_targeted_context_selector ... ok
test tests::test_symbol_indexer_and_repository_map ... ok
test tests::test_startup_crash_recovery ... ok
test tests::test_transaction_journal_commit_and_rollback ... ok
test tests::test_toolchain_detection_matrix ... ok
test tests::test_process_runner_bounded_execution ... ok

test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.07s

running 4 tests (claude_cli_and_process_runner_test)
test test_process_runner_safe_argument_integrity ... ok
test test_process_runner_timeout_and_termination ... ok
test test_claude_cli_live_host_detection ... ok
test test_claude_cli_backend_mock_execution ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.62s

running 1 test (end_to_end_integration_test)
test test_full_ten_criteria_end_to_end_verification ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 4.83s

running 4 tests (mcp_and_skills_test)
test test_mcp_bridge_and_skill_routing ... ok
test test_nested_plugin_skill_discovery_and_instruction_injection ... ok
test test_mcp_stdio_client_protocol_handshake_and_execution ... ok
test test_mcp_stdio_client_timeout_handling ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.45s

running 1 test (provider_enforcement_test)
test test_failure_injection_proves_zero_fallback ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

running 2 tests (real_coding_task_test)
test test_deterministic_harness_patch_and_verify_cycle ... ok
test test_model_driven_autonomous_repair_workflow ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.58s
```

### 2.6 `voxy-model-router` Tests (21/21 PASSED)
```text
running 21 tests
test tests::test_force_provider_route_complete_zero_fallback_on_error ... ok
test tests::test_auto_mode_preserves_fallback_on_error ... ok
test tests::test_force_model_route_complete_zero_fallback_on_error ... ok
test tests::test_force_provider_stt_zero_fallback ... ok
test tests::test_force_provider_tts_zero_fallback ... ok
test tests::test_force_provider_zero_fallback_on_failure ... ok
test tests::test_route_complete_with_fallback ... ok
test tests::test_route_synthesize_with_fallback ... ok
test tests::test_route_transcribe_with_fallback ... ok
test tests::test_router_cloud_only ... ok
test tests::test_router_config_default ... ok
test tests::test_router_force_model ... ok
test tests::test_router_force_model_not_found ... ok
test tests::test_router_local_only ... ok
test tests::test_router_no_providers ... ok
test tests::test_router_local_only_no_local ... ok
test tests::test_routing_mode_default ... ok
test tests::test_router_selects_local_first ... ok
test tests::test_force_provider_tts_zero_fallback_on_timeout ... ok
test tests::test_force_provider_stt_zero_fallback_on_timeout ... ok
test tests::test_force_provider_route_complete_zero_fallback_on_timeout ... ok

test result: ok. 21 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.01s
```
