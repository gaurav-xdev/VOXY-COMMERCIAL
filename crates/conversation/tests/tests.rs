use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;
use uuid::Uuid;
use voxy_personality::MoodState;

use voxy_conversation::{
    BargeInConfig, BargeInManager, ContextTracker, ConversationConfig, ConversationContext,
    ConversationError, ConversationEvent, ConversationSession, HookEvent, InMemoryBargeInManager,
    InMemoryContextTracker, InMemoryHookRegistry, InMemorySession, InMemorySessionManager,
    InMemoryTurnManager, InMemoryWakeStateManager, PersonalityHook, PersonalityHookRegistry,
    SessionId, SessionManager, SessionMetadata, SessionState, SignalPacket, TurnManager,
    TurnSource, WakeState, WakeStateManager,
};

// ---------------------------------------------------------------------------
// Config tests
// ---------------------------------------------------------------------------

#[test]
fn test_config_defaults() {
    let cfg = ConversationConfig::default();
    assert_eq!(cfg.session_timeout_seconds, 3600);
    assert_eq!(cfg.max_turns_per_session, 1000);
    assert!(cfg.enable_barge_in);
    assert!((cfg.barge_in_sensitivity - 0.5).abs() < f32::EPSILON);
    assert_eq!(cfg.idle_timeout_ms, 30000);
    assert!(cfg.wake_on_voice);
    assert!(cfg.wake_on_wake_word);
    assert_eq!(cfg.auto_sleep_after_ms, 60000);
    assert_eq!(cfg.context_retention_turns, 50);
    assert!(cfg.enable_personality_hooks);
    assert!(cfg.default_personality_id.is_none());
}

#[test]
fn test_config_custom() {
    let cfg = ConversationConfig {
        session_timeout_seconds: 7200,
        max_turns_per_session: 500,
        enable_barge_in: false,
        barge_in_sensitivity: 0.8,
        idle_timeout_ms: 15000,
        wake_on_voice: false,
        wake_on_wake_word: true,
        auto_sleep_after_ms: 30000,
        context_retention_turns: 100,
        enable_personality_hooks: false,
        default_personality_id: Some("friendly".to_string()),
    };
    assert_eq!(cfg.session_timeout_seconds, 7200);
    assert_eq!(cfg.default_personality_id, Some("friendly".to_string()));
    assert!(!cfg.enable_barge_in);
}

// ---------------------------------------------------------------------------
// Error tests
// ---------------------------------------------------------------------------

#[test]
fn test_error_display() {
    let err = ConversationError::SessionNotFound("abc".to_string());
    assert_eq!(format!("{}", err), "Session not found: abc");

    let err = ConversationError::SessionAlreadyExists("abc".to_string());
    assert_eq!(format!("{}", err), "Session already exists: abc");

    let err = ConversationError::SessionNotActive("abc".to_string());
    assert_eq!(format!("{}", err), "Session not active: abc");

    let err = ConversationError::InvalidStateTransition {
        from: "Created".to_string(),
        to: "Ended".to_string(),
    };
    assert_eq!(
        format!("{}", err),
        "Invalid state transition: from Created to Ended"
    );

    let err = ConversationError::TurnError("test".to_string());
    assert_eq!(format!("{}", err), "Turn error: test");

    let err = ConversationError::InterruptionNotAllowed;
    assert_eq!(
        format!("{}", err),
        "Interruption not allowed in current state"
    );

    let err = ConversationError::Timeout;
    assert_eq!(format!("{}", err), "Timeout");
}

#[test]
fn test_error_from_personality() {
    let pers_err = voxy_personality::PersonalityError::ProfileNotFound("p1".to_string());
    let err: ConversationError = pers_err.into();
    assert_eq!(format!("{}", err), "Profile not found: p1");
}

#[test]
fn test_error_trait() {
    use std::error::Error;
    let err = ConversationError::Timeout;
    assert!(err.source().is_none());
}

// ---------------------------------------------------------------------------
// Session tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_session_lifecycle() {
    let config = ConversationConfig::default();
    let mut session = InMemorySession::new(&config);
    assert_eq!(session.state(), SessionState::Created);

    session.start(Some("user1"), Some("dev1")).await.unwrap();
    assert_eq!(session.state(), SessionState::Active);

    session.pause().await.unwrap();
    assert_eq!(session.state(), SessionState::Paused);

    session.resume().await.unwrap();
    assert_eq!(session.state(), SessionState::Active);

    session.end().await.unwrap();
    assert_eq!(session.state(), SessionState::Ended);
}

#[tokio::test]
async fn test_session_invalid_transition() {
    let config = ConversationConfig::default();
    let mut session = InMemorySession::new(&config);

    let result = session.pause().await;
    assert!(result.is_err());

    let result = session.end().await;
    assert!(result.is_err());

    let result = session.resume().await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_session_metadata() {
    let config = ConversationConfig::default();
    let mut session = InMemorySession::new(&config);
    session.start(Some("alice"), Some("phone")).await.unwrap();
    let meta = session.metadata();
    assert_eq!(meta.user_id, Some("alice".to_string()));
    assert_eq!(meta.device_id, Some("phone".to_string()));
    assert_eq!(meta.state, SessionState::Active);
}

#[tokio::test]
async fn test_session_process_input_not_active() {
    let config = ConversationConfig::default();
    let mut session = InMemorySession::new(&config);
    let result = session.process_input("hello", true).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_session_set_personality() {
    let config = ConversationConfig::default();
    let mut session = InMemorySession::new(&config);
    session.set_personality("cheerful").await.unwrap();
    assert_eq!(
        session.metadata().personality_id,
        Some("cheerful".to_string())
    );
}

#[tokio::test]
async fn test_session_event_handler() {
    use std::sync::atomic::{AtomicBool, Ordering};
    let config = ConversationConfig::default();
    let mut session = InMemorySession::new(&config);
    let fired = Arc::new(AtomicBool::new(false));
    let fired_clone = fired.clone();
    session
        .on_event(Box::new(move |_event| {
            fired_clone.store(true, Ordering::SeqCst);
        }))
        .await
        .unwrap();
    session.start(None, None).await.unwrap();
    assert!(fired.load(Ordering::SeqCst));
}

#[tokio::test]
async fn test_session_turn_increment() {
    let config = ConversationConfig::default();
    let mut session = InMemorySession::new(&config);
    session.start(None, None).await.unwrap();
    assert_eq!(session.metadata().turn_count, 0);
    session.process_input("hi", true).await.unwrap();
    assert_eq!(session.metadata().turn_count, 1);
}

// ---------------------------------------------------------------------------
// SessionManager tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_session_manager_create_and_get() {
    let config = ConversationConfig::default();
    let manager = InMemorySessionManager::new(config);
    let session = manager.create_session().await.unwrap();
    let id = session.id().clone();
    let fetched = manager.get_session(&id).await.unwrap();
    assert_eq!(fetched.id(), &id);
}

#[tokio::test]
async fn test_session_manager_get_nonexistent() {
    let config = ConversationConfig::default();
    let manager = InMemorySessionManager::new(config);
    let id = SessionId::new();
    let result = manager.get_session(&id).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_session_manager_end_session() {
    let config = ConversationConfig::default();
    let manager = InMemorySessionManager::new(config);
    let session = manager.create_session().await.unwrap();
    let id = session.id().clone();
    manager.end_session(&id).await.unwrap();
    let result = manager.get_session(&id).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_session_manager_list_sessions() {
    let config = ConversationConfig::default();
    let manager = InMemorySessionManager::new(config);
    let _s1 = manager.create_session().await.unwrap();
    let _s2 = manager.create_session().await.unwrap();
    let list = manager.list_sessions().await.unwrap();
    assert_eq!(list.len(), 2);
}

#[tokio::test]
async fn test_session_manager_active_count() {
    let config = ConversationConfig::default();
    let manager = InMemorySessionManager::new(config);
    let _s1 = manager.create_session().await.unwrap();
    let _s2 = manager.create_session().await.unwrap();
    assert_eq!(manager.active_session_count().await, 0);
}

#[tokio::test]
async fn test_session_manager_cleanup_stale() {
    let config = ConversationConfig::default();
    let manager = InMemorySessionManager::new(config);
    let _s1 = manager.create_session().await.unwrap();
    let _s2 = manager.create_session().await.unwrap();
    let cleaned = manager.cleanup_stale_sessions(0).await.unwrap();
    assert_eq!(cleaned, 2);
}

// ---------------------------------------------------------------------------
// TurnManager tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_turn_begin_and_end() {
    let mut mgr = InMemoryTurnManager::new(50);
    let turn = mgr.begin_turn(TurnSource::UserInput).await.unwrap();
    assert_eq!(turn.source, TurnSource::UserInput);
    assert_eq!(mgr.turn_count(), 1);

    let ended = mgr.end_turn(Some("response")).await.unwrap();
    assert_eq!(ended.state, voxy_conversation::TurnState::Completed);
    assert_eq!(ended.output_text, Some("response".to_string()));
}

#[tokio::test]
async fn test_turn_interrupt() {
    let mut mgr = InMemoryTurnManager::new(50);
    mgr.begin_turn(TurnSource::UserInput).await.unwrap();
    mgr.interrupt_current().await.unwrap();
    assert!(mgr.current_turn().is_none());
    let last = mgr.last_turn().unwrap();
    assert!(last.was_interrupted);
}

#[tokio::test]
async fn test_turn_double_begin_fails() {
    let mut mgr = InMemoryTurnManager::new(50);
    mgr.begin_turn(TurnSource::UserInput).await.unwrap();
    let result = mgr.begin_turn(TurnSource::UserInput).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_turn_end_without_begin_fails() {
    let mut mgr = InMemoryTurnManager::new(50);
    let result = mgr.end_turn(Some("text")).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_turn_interrupt_without_begin_fails() {
    let mut mgr = InMemoryTurnManager::new(50);
    let result = mgr.interrupt_current().await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_turn_history() {
    let mut mgr = InMemoryTurnManager::new(50);
    mgr.begin_turn(TurnSource::UserInput).await.unwrap();
    mgr.end_turn(Some("a")).await.unwrap();
    mgr.begin_turn(TurnSource::UserInput).await.unwrap();
    mgr.end_turn(Some("b")).await.unwrap();

    let history = mgr.turn_history(10);
    assert_eq!(history.len(), 2);
    assert_eq!(history[0].output_text, Some("b".to_string()));
    assert_eq!(history[1].output_text, Some("a".to_string()));
}

#[tokio::test]
async fn test_turn_barge_in_flag() {
    let mut mgr = InMemoryTurnManager::new(50);
    assert!(!mgr.is_barge_in());
    mgr.set_barge_in(true).await;
    assert!(mgr.is_barge_in());
}

// ---------------------------------------------------------------------------
// BargeInManager tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_barge_in_no_interrupt_on_silence() {
    let config = BargeInConfig::default();
    let mut mgr = InMemoryBargeInManager::new(config);
    let packet = SignalPacket { peak_level: 0.0 };
    let result = mgr.analyze_signal(&packet).await.unwrap();
    assert!(result.is_none());
}

#[tokio::test]
async fn test_barge_in_interrupt_on_loud_audio() {
    let config = BargeInConfig {
        sensitivity: 0.1,
        ..Default::default()
    };
    let mut mgr = InMemoryBargeInManager::new(config);
    let packet = SignalPacket { peak_level: 0.5 };
    let result = mgr.analyze_signal(&packet).await.unwrap();
    assert!(result.is_some());
    assert!(mgr.is_interrupted());
}

#[tokio::test]
async fn test_barge_in_disabled() {
    let config = BargeInConfig {
        enabled: false,
        ..Default::default()
    };
    let mut mgr = InMemoryBargeInManager::new(config);
    let packet = SignalPacket { peak_level: 0.9 };
    let result = mgr.analyze_signal(&packet).await.unwrap();
    assert!(result.is_none());
    assert!(!mgr.is_interrupted());
}

#[tokio::test]
async fn test_barge_in_clear_interruption() {
    let config = BargeInConfig {
        sensitivity: 0.1,
        ..Default::default()
    };
    let mut mgr = InMemoryBargeInManager::new(config);
    let packet = SignalPacket { peak_level: 0.5 };
    mgr.analyze_signal(&packet).await.unwrap();
    assert!(mgr.is_interrupted());
    mgr.clear_interruption().await;
    assert!(!mgr.is_interrupted());
}

#[tokio::test]
async fn test_barge_in_config_change() {
    let mut mgr = InMemoryBargeInManager::new(BargeInConfig::default());
    assert!(mgr.config().enabled);
    let new_config = BargeInConfig {
        enabled: false,
        ..Default::default()
    };
    mgr.set_config(new_config).await;
    assert!(!mgr.config().enabled);
}

#[tokio::test]
async fn test_barge_in_count_and_last() {
    let config = BargeInConfig {
        sensitivity: 0.1,
        ..Default::default()
    };
    let mut mgr = InMemoryBargeInManager::new(config);
    let packet = SignalPacket { peak_level: 0.5 };
    mgr.analyze_signal(&packet).await.unwrap();
    mgr.analyze_signal(&packet).await.unwrap();
    assert_eq!(mgr.interruption_count(), 2);
    assert!(mgr.last_interruption().is_some());
}

// ---------------------------------------------------------------------------
// WakeStateManager tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_wake_initial_state() {
    let mgr = InMemoryWakeStateManager::new(60000);
    assert_eq!(mgr.state(), WakeState::Asleep);
    assert_eq!(mgr.wake_count(), 0);
}

#[tokio::test]
async fn test_wake_valid_transitions() {
    let mut mgr = InMemoryWakeStateManager::new(60000);
    mgr.wake().await.unwrap();
    assert_eq!(mgr.state(), WakeState::Awake);

    mgr.transition_to(WakeState::Listening).await.unwrap();
    assert_eq!(mgr.state(), WakeState::Listening);

    mgr.transition_to(WakeState::Processing).await.unwrap();
    assert_eq!(mgr.state(), WakeState::Processing);

    mgr.transition_to(WakeState::Awake).await.unwrap();
    assert_eq!(mgr.state(), WakeState::Awake);

    mgr.sleep().await.unwrap();
    assert_eq!(mgr.state(), WakeState::Asleep);
}

#[tokio::test]
async fn test_wake_invalid_transition() {
    let mut mgr = InMemoryWakeStateManager::new(60000);
    let result = mgr.transition_to(WakeState::Processing).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_wake_already_asleep_sleep() {
    let mut mgr = InMemoryWakeStateManager::new(60000);
    mgr.sleep().await.unwrap();
    assert_eq!(mgr.state(), WakeState::Asleep);
}

#[tokio::test]
async fn test_wake_already_awake_wake() {
    let mut mgr = InMemoryWakeStateManager::new(60000);
    mgr.wake().await.unwrap();
    mgr.wake().await.unwrap();
    assert_eq!(mgr.state(), WakeState::Awake);
    assert_eq!(mgr.wake_count(), 1);
}

#[tokio::test]
async fn test_wake_idle_timeout() {
    let mut mgr = InMemoryWakeStateManager::new(100);
    mgr.wake().await.unwrap();
    let result = mgr.tick_idle(200).await.unwrap();
    assert_eq!(result, Some(WakeState::Awake));
    assert_eq!(mgr.state(), WakeState::Asleep);
}

#[tokio::test]
async fn test_wake_idle_not_triggered_when_not_idle() {
    let mut mgr = InMemoryWakeStateManager::new(1000);
    mgr.wake().await.unwrap();
    let result = mgr.tick_idle(50).await.unwrap();
    assert!(result.is_none());
    assert_eq!(mgr.state(), WakeState::Awake);
}

// ---------------------------------------------------------------------------
// ContextTracker tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_context_set_and_get() {
    let mut ctx = InMemoryContextTracker::new(50);
    ctx.set("name", "Alice").await.unwrap();
    assert_eq!(ctx.get("name"), Some("Alice"));
    assert!(ctx.has_key("name"));
    assert_eq!(ctx.entry_count(), 1);
}

#[tokio::test]
async fn test_context_remove() {
    let mut ctx = InMemoryContextTracker::new(50);
    ctx.set("key", "value").await.unwrap();
    ctx.remove("key").await.unwrap();
    assert!(!ctx.has_key("key"));
    assert_eq!(ctx.entry_count(), 0);
}

#[tokio::test]
async fn test_context_clear() {
    let mut ctx = InMemoryContextTracker::new(50);
    ctx.set("a", "1").await.unwrap();
    ctx.set("b", "2").await.unwrap();
    ctx.clear().await.unwrap();
    assert_eq!(ctx.entry_count(), 0);
    assert!(ctx.get("a").is_none());
}

#[tokio::test]
async fn test_context_get_nonexistent() {
    let ctx = InMemoryContextTracker::new(50);
    assert_eq!(ctx.get("missing"), None);
}

#[tokio::test]
async fn test_context_topic() {
    let mut ctx = InMemoryContextTracker::new(50);
    assert!(ctx.current_topic().is_none());
    ctx.set_current_topic("weather").await;
    assert_eq!(ctx.current_topic(), Some("weather"));
}

#[tokio::test]
async fn test_context_turn_history() {
    let ctx = InMemoryContextTracker::new(50);
    let history = ctx.turn_history(10);
    assert!(history.is_empty());
}

#[test]
fn test_conversation_context_new() {
    let ctx = ConversationContext::new();
    assert!(ctx.entries.is_empty());
    assert!(ctx.current_topic.is_none());
}

// ---------------------------------------------------------------------------
// Hook tests
// ---------------------------------------------------------------------------

struct TestHook {
    name: String,
    fired: Arc<std::sync::atomic::AtomicUsize>,
}

#[async_trait]
impl PersonalityHook for TestHook {
    fn name(&self) -> &str {
        &self.name
    }

    async fn on_session_start(&self, _metadata: &SessionMetadata) -> Result<(), ConversationError> {
        self.fired.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(())
    }

    async fn on_session_end(&self, _metadata: &SessionMetadata) -> Result<(), ConversationError> {
        Ok(())
    }

    async fn on_turn_start(
        &self,
        _session_id: &SessionId,
        _turn: &voxy_conversation::Turn,
    ) -> Result<(), ConversationError> {
        Ok(())
    }

    async fn on_turn_end(
        &self,
        _session_id: &SessionId,
        _turn: &voxy_conversation::Turn,
    ) -> Result<(), ConversationError> {
        Ok(())
    }

    async fn on_mood_change(
        &self,
        _session_id: &SessionId,
        _old_mood: &MoodState,
        _new_mood: &MoodState,
    ) -> Result<(), ConversationError> {
        Ok(())
    }

    async fn on_input_received(
        &self,
        _session_id: &SessionId,
        text: &str,
    ) -> Result<String, ConversationError> {
        self.fired.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(format!("modified:{}", text))
    }

    async fn on_output_generated(
        &self,
        _session_id: &SessionId,
        text: &str,
    ) -> Result<String, ConversationError> {
        Ok(format!("enhanced:{}", text))
    }
}

#[tokio::test]
async fn test_hook_register_and_count() {
    let registry = InMemoryHookRegistry::default();
    assert_eq!(registry.hook_count(), 0);
    let hook = Box::new(TestHook {
        name: "test".to_string(),
        fired: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
    });
    registry.register_hook(hook).await.unwrap();
    assert_eq!(registry.hook_count(), 1);
}

#[tokio::test]
async fn test_hook_unregister() {
    let registry = InMemoryHookRegistry::default();
    let hook = Box::new(TestHook {
        name: "test".to_string(),
        fired: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
    });
    registry.register_hook(hook).await.unwrap();
    registry.unregister_hook("test").await.unwrap();
    assert_eq!(registry.hook_count(), 0);
}

#[tokio::test]
async fn test_hook_execute_session_start() {
    let registry = InMemoryHookRegistry::default();
    let counter = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let hook = Box::new(TestHook {
        name: "test".to_string(),
        fired: counter.clone(),
    });
    registry.register_hook(hook).await.unwrap();
    let metadata = SessionMetadata {
        id: SessionId::new(),
        state: SessionState::Created,
        created_at: Utc::now(),
        last_activity_at: Utc::now(),
        turn_count: 0,
        personality_id: None,
        user_id: None,
        device_id: None,
    };
    registry
        .execute_hooks(HookEvent::SessionStart(&metadata))
        .await
        .unwrap();
    assert_eq!(counter.load(std::sync::atomic::Ordering::SeqCst), 1);
}

#[tokio::test]
async fn test_hook_execute_input_modification() {
    struct ModifyingHook;

    #[async_trait]
    impl PersonalityHook for ModifyingHook {
        fn name(&self) -> &str {
            "modifier"
        }

        async fn on_session_start(
            &self,
            _metadata: &SessionMetadata,
        ) -> Result<(), ConversationError> {
            Ok(())
        }

        async fn on_session_end(
            &self,
            _metadata: &SessionMetadata,
        ) -> Result<(), ConversationError> {
            Ok(())
        }

        async fn on_turn_start(
            &self,
            _session_id: &SessionId,
            _turn: &voxy_conversation::Turn,
        ) -> Result<(), ConversationError> {
            Ok(())
        }

        async fn on_turn_end(
            &self,
            _session_id: &SessionId,
            _turn: &voxy_conversation::Turn,
        ) -> Result<(), ConversationError> {
            Ok(())
        }

        async fn on_mood_change(
            &self,
            _session_id: &SessionId,
            _old_mood: &MoodState,
            _new_mood: &MoodState,
        ) -> Result<(), ConversationError> {
            Ok(())
        }

        async fn on_input_received(
            &self,
            _session_id: &SessionId,
            _text: &str,
        ) -> Result<String, ConversationError> {
            Ok("intercepted".to_string())
        }

        async fn on_output_generated(
            &self,
            _session_id: &SessionId,
            _text: &str,
        ) -> Result<String, ConversationError> {
            Ok("polished".to_string())
        }
    }

    let registry = InMemoryHookRegistry::default();
    registry
        .register_hook(Box::new(ModifyingHook))
        .await
        .unwrap();

    let id = SessionId::new();
    registry
        .execute_hooks(HookEvent::InputReceived {
            session_id: &id,
            text: "hello",
        })
        .await
        .unwrap();
    let result = registry.execute_hooks(HookEvent::OutputGenerated {
        session_id: &id,
        text: "world",
    });
    // Should not error since execute_hooks handles the returned string
    assert!(result.await.is_ok());
}

// ---------------------------------------------------------------------------
// Event Display tests
// ---------------------------------------------------------------------------

#[test]
fn test_event_display() {
    let id = SessionId::new();
    let event = ConversationEvent::SessionCreated { id: id.clone() };
    let display = format!("{}", event);
    assert!(display.starts_with("SessionCreated("));

    let event = ConversationEvent::SessionStarted { id: id.clone() };
    assert!(format!("{}", event).starts_with("SessionStarted("));
}

// ---------------------------------------------------------------------------
// SessionId tests
// ---------------------------------------------------------------------------

#[test]
fn test_session_id_new() {
    let id = SessionId::new();
    assert_ne!(id, SessionId::new());
}

#[test]
fn test_session_id_from_string() {
    let uuid = Uuid::new_v4();
    let id = SessionId::from_string(&uuid.to_string()).unwrap();
    assert_eq!(id.0, uuid);
}

#[test]
fn test_session_id_from_invalid_string() {
    let result = SessionId::from_string("not-a-uuid");
    assert!(result.is_err());
}

#[test]
fn test_session_id_display() {
    let uuid = Uuid::new_v4();
    let id = SessionId(uuid);
    assert_eq!(format!("{}", id), uuid.to_string());
}

// ---------------------------------------------------------------------------
// TurnSource/TurnState Display tests
// ---------------------------------------------------------------------------

#[test]
fn test_turn_source_display() {
    assert_eq!(format!("{}", TurnSource::UserInput), "UserInput");
    assert_eq!(format!("{}", TurnSource::WakeWord), "WakeWord");
    assert_eq!(format!("{}", TurnSource::Interruption), "Interruption");
}

#[test]
fn test_wake_state_display() {
    assert_eq!(format!("{}", WakeState::Asleep), "Asleep");
    assert_eq!(format!("{}", WakeState::Processing), "Processing");
}

#[test]
fn test_session_state_display() {
    assert_eq!(format!("{}", SessionState::Created), "Created");
    assert_eq!(format!("{}", SessionState::Active), "Active");
    assert_eq!(
        format!("{}", SessionState::Failed("err".to_string())),
        "Failed(err)"
    );
}
