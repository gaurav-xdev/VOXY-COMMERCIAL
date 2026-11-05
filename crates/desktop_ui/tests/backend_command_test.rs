use voxy_desktop_ui::backend::{BackendCommand, BackendDispatcher, BackendResult};
use voxy_desktop_ui::views::SettingsFormState;

#[tokio::test]
async fn test_backend_dispatcher_settings_roundtrip() {
    // 1. Initialize test backend dispatcher with mock/temp stores
    let data_dir = tempfile::tempdir().expect("tempdir");
    let dispatcher = BackendDispatcher::create_for_test(data_dir.path()).await;

    // 2. Load default settings
    let load_res = dispatcher.dispatch(BackendCommand::LoadSettings).await;
    assert!(
        load_res.is_ok(),
        "LoadSettings should succeed: {:?}",
        load_res
    );
    let initial_form = match load_res.unwrap() {
        BackendResult::SettingsLoaded(form) => form,
        other => panic!("Unexpected result: {:?}", other),
    };
    assert_eq!(initial_form.assistant_name, "OSMOO");

    // 3. Mutate and Save settings
    let mut modified = initial_form.clone();
    modified.assistant_name = "OSMOO Prime System".to_string();
    modified.primary_llm = "ollama".to_string();
    modified.audio_sample_rate = 96000;

    let save_res = dispatcher
        .dispatch(BackendCommand::SaveSettings(modified.clone()))
        .await;
    assert!(
        save_res.is_ok(),
        "SaveSettings should succeed: {:?}",
        save_res
    );

    // 4. Reload and verify persistence
    let reload_res = dispatcher.dispatch(BackendCommand::LoadSettings).await;
    let loaded = match reload_res.unwrap() {
        BackendResult::SettingsLoaded(form) => form,
        other => panic!("Unexpected result: {:?}", other),
    };
    assert_eq!(loaded.assistant_name, "OSMOO Prime System");
    assert_eq!(loaded.audio_sample_rate, 96000);
}

#[tokio::test]
async fn test_backend_dispatcher_prompt_and_conversation_persistence() {
    let data_dir = tempfile::tempdir().expect("tempdir");
    let dispatcher = BackendDispatcher::create_for_test(data_dir.path()).await;

    // Dispatch prompt
    let prompt_res = dispatcher
        .dispatch(BackendCommand::SendPrompt {
            text: "Hello OSMOO, calculate system status".to_string(),
        })
        .await;

    assert!(
        prompt_res.is_ok(),
        "SendPrompt should succeed: {:?}",
        prompt_res
    );
    match prompt_res.unwrap() {
        BackendResult::PromptResponse { text, .. } => {
            assert!(!text.is_empty(), "Prompt response should not be empty");
        }
        other => panic!("Unexpected result: {:?}", other),
    }

    // Retrieve chat history from SQLite conversation store
    let hist_res = dispatcher
        .dispatch(BackendCommand::GetChatHistory {
            limit: 10,
            offset: 0,
        })
        .await;
    assert!(
        hist_res.is_ok(),
        "GetChatHistory should succeed: {:?}",
        hist_res
    );
    let turns = match hist_res.unwrap() {
        BackendResult::ChatHistory(t) => t,
        other => panic!("Unexpected result: {:?}", other),
    };

    // Should contain both user turn and assistant response turn
    assert!(
        turns.len() >= 2,
        "Chat history should contain at least user and assistant turns"
    );
    assert!(turns
        .iter()
        .any(|t| t.is_user && t.text.contains("calculate system status")));
    assert!(turns.iter().any(|t| !t.is_user));
}

#[tokio::test]
async fn test_backend_dispatcher_emergency_stop() {
    let data_dir = tempfile::tempdir().expect("tempdir");
    let dispatcher = BackendDispatcher::create_for_test(data_dir.path()).await;

    let stop_res = dispatcher.dispatch(BackendCommand::EmergencyStop).await;
    assert!(stop_res.is_ok());
    assert_eq!(stop_res.unwrap(), BackendResult::EmergencyStopTriggered);
}

#[tokio::test]
async fn test_backend_dispatcher_validation_fail_closed() {
    let data_dir = tempfile::tempdir().expect("tempdir");
    let dispatcher = BackendDispatcher::create_for_test(data_dir.path()).await;

    // 1. Empty prompt must be rejected
    let empty_prompt = dispatcher
        .dispatch(BackendCommand::SendPrompt {
            text: "   ".to_string(),
        })
        .await;
    assert!(empty_prompt.is_err(), "Empty prompt must fail closed");

    // 2. Empty assistant name in settings must be rejected
    let mut form = SettingsFormState::default();
    form.assistant_name = "".to_string();
    let invalid_settings = dispatcher
        .dispatch(BackendCommand::SaveSettings(form))
        .await;
    assert!(
        invalid_settings.is_err(),
        "Empty assistant name must fail validation"
    );
}

#[tokio::test]
async fn test_backend_dispatcher_onboarding_completion() {
    let data_dir = tempfile::tempdir().expect("tempdir");
    let dispatcher = BackendDispatcher::create_for_test(data_dir.path()).await;

    let cfg = voxy_desktop_ui::components::onboarding_wizard::OnboardingConfig {
        assistant_name: "OSMOO Sentinel".to_string(),
        companion_mode: voxy_desktop_ui::components::assistant_core::CompanionMode::Cat,
        primary_llm: "ollama".to_string(),
        selected_model: "llama3.2:latest".to_string(),
        api_key: None,
        stt_provider: "whisper".to_string(),
        tts_provider: "piper".to_string(),
        wake_word_enabled: true,
        companion_scale: 1.0,
        companion_position: "bottom-right".to_string(),
        autowalk_enabled: true,
        proactive_speech_enabled: true,
        start_on_boot: true,
        active_tier: "pro".to_string(),
    };

    let res = dispatcher
        .dispatch(BackendCommand::CompleteOnboarding(cfg))
        .await;
    assert!(res.is_ok(), "CompleteOnboarding should succeed: {:?}", res);
    assert_eq!(res.unwrap(), BackendResult::OnboardingCompleted);

    // Verify settings were updated
    let load_res = dispatcher
        .dispatch(BackendCommand::LoadSettings)
        .await
        .unwrap();
    if let BackendResult::SettingsLoaded(loaded) = load_res {
        assert_eq!(loaded.assistant_name, "OSMOO Sentinel");
        assert_eq!(loaded.ollama_model, "llama3.2:latest");
    } else {
        panic!("Expected SettingsLoaded");
    }
}

#[tokio::test]
async fn test_backend_dispatcher_native_tool_system_info() {
    let data_dir = tempfile::tempdir().expect("tempdir");
    let dispatcher = BackendDispatcher::create_for_test(data_dir.path()).await;

    // Dispatch "system info" native command
    let res = dispatcher
        .dispatch(BackendCommand::SendPrompt {
            text: "system info".to_string(),
        })
        .await;

    assert!(res.is_ok(), "Native tool execution should succeed");
    match res.unwrap() {
        BackendResult::PromptResponse { text, .. } => {
            assert!(
                text.contains("OSMOO Native System Diagnostics"),
                "Response must contain formatted system diagnostics"
            );
            assert!(
                text.contains("Operating System"),
                "Response must include Operating System info"
            );
            assert!(
                text.contains("Architecture"),
                "Response must include Architecture info"
            );
        }
        other => panic!("Expected PromptResponse, got {:?}", other),
    }
}

#[tokio::test]
async fn test_backend_dispatcher_native_tool_windows_and_processes() {
    let data_dir = tempfile::tempdir().expect("tempdir");
    let dispatcher = BackendDispatcher::create_for_test(data_dir.path()).await;

    // 1. Dispatch "list windows"
    let win_res = dispatcher
        .dispatch(BackendCommand::SendPrompt {
            text: "list windows".to_string(),
        })
        .await;
    assert!(win_res.is_ok());
    match win_res.unwrap() {
        BackendResult::PromptResponse { text, .. } => {
            assert!(
                text.contains("Active Windows") || text.contains("No active top-level windows"),
                "Response must contain window listing header"
            );
        }
        other => panic!("Expected PromptResponse, got {:?}", other),
    }

    // 2. Dispatch "list processes"
    let proc_res = dispatcher
        .dispatch(BackendCommand::SendPrompt {
            text: "list processes".to_string(),
        })
        .await;
    assert!(proc_res.is_ok());
    match proc_res.unwrap() {
        BackendResult::PromptResponse { text, .. } => {
            assert!(
                text.contains("Running Processes"),
                "Response must contain process listing header"
            );
            assert!(
                text.contains("PID:"),
                "Response must contain PID identifiers"
            );
        }
        other => panic!("Expected PromptResponse, got {:?}", other),
    }
}

#[tokio::test]
async fn test_backend_dispatcher_system_telemetry_metrics() {
    let data_dir = tempfile::tempdir().expect("tempdir");
    let dispatcher = BackendDispatcher::create_for_test(data_dir.path()).await;

    let res = dispatcher.dispatch(BackendCommand::GetSystemTelemetry).await;
    assert!(res.is_ok());
    match res.unwrap() {
        BackendResult::SystemTelemetry {
            active_llm,
            memory_used_mb,
            cpu_usage_pct,
            ..
        } => {
            assert!(!active_llm.is_empty(), "Active LLM should not be empty");
            assert!(
                memory_used_mb > 0,
                "Real system memory telemetry must be greater than 0 MB, got {}",
                memory_used_mb
            );
            assert!(
                cpu_usage_pct >= 0.0,
                "CPU usage percent must be non-negative"
            );
        }
        other => panic!("Expected SystemTelemetry, got {:?}", other),
    }
}

#[tokio::test]
async fn test_backend_dispatcher_settings_10_sections_roundtrip() {
    let data_dir = tempfile::tempdir().expect("tempdir");
    let dispatcher = BackendDispatcher::create_for_test(data_dir.path()).await;

    let load_res = dispatcher.dispatch(BackendCommand::LoadSettings).await.unwrap();
    let initial_form = match load_res {
        BackendResult::SettingsLoaded(f) => f,
        other => panic!("Unexpected result: {:?}", other),
    };

    let mut form = initial_form.clone();
    // 1. General & Profile
    form.user_name = "Jane Operator".to_string();
    form.user_email = "jane@osmoo.in".to_string();
    form.organization = "Sovereign Labs".to_string();
    form.role = "Lead Architect".to_string();
    form.language = "en-gb".to_string();
    form.theme = "light".to_string();
    form.autolaunch = true;
    form.close_behavior = "quit".to_string();
    form.auto_update = true;
    form.update_channel = "beta".to_string();

    // 2. Models
    form.primary_llm = "ollama".to_string();
    form.coding_model = "qwen2.5-coder:7b".to_string();
    form.voice_model = "llama3.2:3b".to_string();
    form.embedding_model = "nomic-embed-text".to_string();
    form.enable_fallback = false;

    // 3. Voice
    form.voice_enabled = true;
    form.wake_words = vec!["Hey OSMOO".to_string(), "Computer".to_string()];
    form.vad_sensitivity = 0.65;
    form.speech_speed = 1.15;
    form.speech_pitch = 1.05;
    form.interruption_barge_in = true;
    form.noise_suppression = true;
    form.echo_cancellation = true;

    // 4. Companion
    form.companion_character_id = "dragon".to_string();
    form.companion_size = 1.25;
    form.companion_position = "top-right".to_string();
    form.companion_expression = "curious".to_string();
    form.companion_lip_sync = true;
    form.companion_animations = true;
    form.companion_draggable = true;
    form.desktop_overlay_mode = true;

    // 5. Security
    form.file_system_access = "workspace_only".to_string();
    form.browser_automation = "prompt".to_string();
    form.shell_execution = "require_confirmation".to_string();
    form.require_approval_sensitive = true;
    form.require_approval_financial = true;
    form.audit_logging_enabled = true;

    // 6. Memory
    form.memory_enabled = true;
    form.memory_retention_days = 90;
    form.auto_consolidate = true;
    form.private_session = false;
    form.user_persona_notes = "Always format tables nicely.".to_string();

    // 7. Automation
    form.background_tasks_allowed = true;
    form.pause_on_battery = true;
    form.max_concurrent_tasks = 4;
    form.task_timeout_seconds = 120;
    form.action_risk_threshold = "prompt_risky".to_string();

    // 8. Performance
    form.max_memory_mb = 4096;
    form.max_cpu_percent = 70.0;
    form.gpu_acceleration = true;
    form.model_unload_timeout_minutes = 20;

    let save_res = dispatcher.dispatch(BackendCommand::SaveSettings(form.clone())).await;
    assert!(save_res.is_ok(), "SaveSettings failed: {:?}", save_res);

    let reloaded = match dispatcher.dispatch(BackendCommand::LoadSettings).await.unwrap() {
        BackendResult::SettingsLoaded(f) => f,
        other => panic!("Unexpected result: {:?}", other),
    };

    assert_eq!(reloaded.user_name, "Jane Operator");
    assert_eq!(reloaded.organization, "Sovereign Labs");
    assert_eq!(reloaded.coding_model, "qwen2.5-coder:7b");
    assert_eq!(reloaded.companion_character_id, "dragon");
    assert_eq!(reloaded.companion_size, 1.25);
    assert_eq!(reloaded.max_concurrent_tasks, 4);
    assert_eq!(reloaded.user_persona_notes, "Always format tables nicely.");
    assert_eq!(reloaded.max_memory_mb, 4096);
    assert!(!reloaded.enable_fallback);
}

#[tokio::test]
async fn test_backend_dispatcher_settings_validation_limits() {
    let data_dir = tempfile::tempdir().expect("tempdir");
    let dispatcher = BackendDispatcher::create_for_test(data_dir.path()).await;

    // 1. Exceeding 10 wake words must fail validation
    let mut form = SettingsFormState::default();
    form.wake_words = (0..11).map(|i| format!("Hotword {i}")).collect();
    let err = dispatcher.dispatch(BackendCommand::SaveSettings(form)).await;
    assert!(err.is_err());
    assert!(err.unwrap_err().to_string().contains("Wake words count exceeds"));

    // 2. Companion size out of bounds (< 0.2 or > 3.0) must fail
    let mut form2 = SettingsFormState::default();
    form2.companion_size = 5.0;
    let err2 = dispatcher.dispatch(BackendCommand::SaveSettings(form2)).await;
    assert!(err2.is_err());
    assert!(err2.unwrap_err().to_string().contains("Companion scale must be between"));

    // 3. Speech speed out of bounds (< 0.25 or > 3.0) must fail
    let mut form3 = SettingsFormState::default();
    form3.speech_speed = 0.1;
    let err3 = dispatcher.dispatch(BackendCommand::SaveSettings(form3)).await;
    assert!(err3.is_err());
    assert!(err3.unwrap_err().to_string().contains("Speech speed must be between"));

    // 4. Invalid email format must fail
    let mut form4 = SettingsFormState::default();
    form4.user_email = "invalid_email_without_at".to_string();
    let err4 = dispatcher.dispatch(BackendCommand::SaveSettings(form4)).await;
    assert!(err4.is_err());
    assert!(err4.unwrap_err().to_string().contains("Invalid operator email address"));
}

#[tokio::test]
async fn test_backend_dispatcher_settings_strict_routing_zero_silent_fallback() {
    let data_dir = tempfile::tempdir().expect("tempdir");
    let dispatcher = BackendDispatcher::create_for_test(data_dir.path()).await;

    let mut form = SettingsFormState::default();
    form.primary_llm = "ollama".to_string();
    form.enable_fallback = false;

    let res = dispatcher.dispatch(BackendCommand::SaveSettings(form)).await;
    assert!(res.is_ok());

    // When enable_fallback is false, router mode is ForceProvider("ollama")
    // and fallback is strictly forbidden.
    assert!(!dispatcher.model_router().allows_fallback());
}

#[tokio::test]
async fn test_backend_dispatcher_diagnostics_export_redacted() {
    let data_dir = tempfile::tempdir().expect("tempdir");
    let dispatcher = BackendDispatcher::create_for_test(data_dir.path()).await;

    let res = dispatcher.dispatch(BackendCommand::ExportDiagnostics).await;
    assert!(res.is_ok());
    match res.unwrap() {
        BackendResult::DiagnosticsExported { sanitized_report } => {
            assert!(sanitized_report.contains("=== OSMOO SANITIZED SYSTEM DIAGNOSTICS ==="));
            assert!(sanitized_report.contains("OSMOO Version: v0.1.0-commercial"));
            assert!(sanitized_report.contains("[DPAPI REDACTED"));
            assert!(!sanitized_report.contains("sk-"));
        }
        other => panic!("Unexpected result: {:?}", other),
    }
}

#[tokio::test]
async fn test_backend_dispatcher_purge_stored_memories() {
    let data_dir = tempfile::tempdir().expect("tempdir");
    let dispatcher = BackendDispatcher::create_for_test(data_dir.path()).await;

    let res = dispatcher.dispatch(BackendCommand::PurgeStoredMemories).await;
    assert!(res.is_ok());
    match res.unwrap() {
        BackendResult::MemoriesPurged { .. } => {}
        other => panic!("Unexpected result: {:?}", other),
    }
}
