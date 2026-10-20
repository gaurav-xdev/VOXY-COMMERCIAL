use voxy_desktop_ui::backend::{BackendCommand, BackendDispatcher, BackendResult};
use voxy_desktop_ui::views::SettingsFormState;

#[tokio::test]
async fn test_backend_dispatcher_settings_roundtrip() {
    // 1. Initialize test backend dispatcher with mock/temp stores
    let data_dir = tempfile::tempdir().expect("tempdir");
    let dispatcher = BackendDispatcher::create_for_test(data_dir.path()).await;

    // 2. Load default settings
    let load_res = dispatcher.dispatch(BackendCommand::LoadSettings).await;
    assert!(load_res.is_ok(), "LoadSettings should succeed: {:?}", load_res);
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

    let save_res = dispatcher.dispatch(BackendCommand::SaveSettings(modified.clone())).await;
    assert!(save_res.is_ok(), "SaveSettings should succeed: {:?}", save_res);

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

    assert!(prompt_res.is_ok(), "SendPrompt should succeed: {:?}", prompt_res);
    match prompt_res.unwrap() {
        BackendResult::PromptResponse { text, .. } => {
            assert!(!text.is_empty(), "Prompt response should not be empty");
        }
        other => panic!("Unexpected result: {:?}", other),
    }

    // Retrieve chat history from SQLite conversation store
    let hist_res = dispatcher.dispatch(BackendCommand::GetChatHistory { limit: 10, offset: 0 }).await;
    assert!(hist_res.is_ok(), "GetChatHistory should succeed: {:?}", hist_res);
    let turns = match hist_res.unwrap() {
        BackendResult::ChatHistory(t) => t,
        other => panic!("Unexpected result: {:?}", other),
    };

    // Should contain both user turn and assistant response turn
    assert!(turns.len() >= 2, "Chat history should contain at least user and assistant turns");
    assert!(turns.iter().any(|t| t.is_user && t.text.contains("calculate system status")));
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
    let empty_prompt = dispatcher.dispatch(BackendCommand::SendPrompt { text: "   ".to_string() }).await;
    assert!(empty_prompt.is_err(), "Empty prompt must fail closed");

    // 2. Empty assistant name in settings must be rejected
    let mut form = SettingsFormState::default();
    form.assistant_name = "".to_string();
    let invalid_settings = dispatcher.dispatch(BackendCommand::SaveSettings(form)).await;
    assert!(invalid_settings.is_err(), "Empty assistant name must fail validation");
}

#[tokio::test]
async fn test_backend_dispatcher_onboarding_completion() {
    let data_dir = tempfile::tempdir().expect("tempdir");
    let dispatcher = BackendDispatcher::create_for_test(data_dir.path()).await;

    let cfg = voxy_desktop_ui::components::onboarding_wizard::OnboardingConfig {
        assistant_name: "OSMOO Sentinel".to_string(),
        companion_mode: voxy_desktop_ui::components::assistant_core::CompanionMode::ParticleOrb,
        primary_llm: "ollama".to_string(),
        selected_model: "llama3.2:latest".to_string(),
        stt_provider: "whisper".to_string(),
        tts_provider: "piper".to_string(),
        active_tier: "pro".to_string(),
    };

    let res = dispatcher.dispatch(BackendCommand::CompleteOnboarding(cfg)).await;
    assert!(res.is_ok(), "CompleteOnboarding should succeed: {:?}", res);
    assert_eq!(res.unwrap(), BackendResult::OnboardingCompleted);

    // Verify settings were updated
    let load_res = dispatcher.dispatch(BackendCommand::LoadSettings).await.unwrap();
    if let BackendResult::SettingsLoaded(loaded) = load_res {
        assert_eq!(loaded.assistant_name, "OSMOO Sentinel");
        assert_eq!(loaded.ollama_model, "llama3.2:latest");
    } else {
        panic!("Expected SettingsLoaded");
    }
}

