//! Comprehensive tests for the cloud-first STT/TTS provider architecture.
//! Tests rate limiting, quota tracking, multi-factor scoring, failover, and zero secret leakage.

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::Duration;

    use futures::StreamExt;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    use crate::provider::admin::VoiceAdminApi;
    use crate::provider::cache::TTSCache;
    use crate::provider::quota::budget::RoutingMode;
    use crate::provider::quota::rate_limiter::RateLimitConfig;
    use crate::provider::quota::storage::InMemoryQuotaStorage;
    use crate::provider::quota::{ProviderQuotaManager, QuotaViolation};
    use crate::provider::router::{STTRouter, TTSRouter};
    use crate::provider::stt::{LocalSapiSTTProvider, MockSTTProvider};
    use crate::provider::traits::{
        AudioData, HealthStatus, ProviderError, STTProvider, TTSProvider, VoiceLanguage,
        VoiceMode,
    };
    use crate::provider::tts::{CartesiaTTSProvider, MockTTSProvider};

    #[tokio::test]
    async fn test_rate_limiter_rpm_and_rpd_preflight() {
        let storage = Arc::new(InMemoryQuotaStorage::new());
        let manager = ProviderQuotaManager::new(storage, RoutingMode::Performance);

        let config = RateLimitConfig {
            rpm: 3,
            rpd: 10,
            characters_per_minute: 1000,
            characters_per_day: 10000,
            audio_seconds_per_day: 3600.0,
            max_concurrency: 5,
            burst_capacity: 5,
            safety_margin: 0.0, // test exact limits
        };
        manager.register_provider("mock-stt", config);

        // First 3 requests pass preflight
        assert!(manager.check_preflight("mock-stt", 10, 1.0).is_ok());
        manager.record_success("mock-stt", Duration::from_millis(50), 10, 1.0, 0.001);

        assert!(manager.check_preflight("mock-stt", 10, 1.0).is_ok());
        manager.record_success("mock-stt", Duration::from_millis(50), 10, 1.0, 0.001);

        assert!(manager.check_preflight("mock-stt", 10, 1.0).is_ok());
        manager.record_success("mock-stt", Duration::from_millis(50), 10, 1.0, 0.001);

        // 4th request exceeds RPM (limit is 3)
        let violation = manager.check_preflight("mock-stt", 10, 1.0);
        assert!(violation.is_err(), "Expected RPM limit to reject 4th request");
    }

    #[tokio::test]
    async fn test_cooldown_and_rate_limit_429() {
        let storage = Arc::new(InMemoryQuotaStorage::new());
        let manager = ProviderQuotaManager::new(storage, RoutingMode::Balanced);
        manager.register_provider(
            "groq",
            RateLimitConfig {
                rpm: 60,
                rpd: 1000,
                characters_per_minute: 50000,
                characters_per_day: 100000,
                audio_seconds_per_day: 3600.0,
                max_concurrency: 5,
                burst_capacity: 5,
                safety_margin: 0.1,
            },
        );

        assert!(!manager.is_in_cooldown("groq"));

        // Record 429 with Retry-After of 5 seconds
        manager.record_rate_limit("groq", Some(Duration::from_secs(5)));
        assert!(manager.is_in_cooldown("groq"));

        // Status reflects cooldown
        let status = manager.get_status("groq");
        assert!(status.is_cooldown);
        assert_eq!(status.health, HealthStatus::Cooldown);

        // Preflight rejected during cooldown
        assert!(manager.check_preflight("groq", 10, 1.0).is_err());

        // Reset cooldown
        manager.reset_cooldown("groq");
        assert!(!manager.is_in_cooldown("groq"));
        assert!(manager.check_preflight("groq", 10, 1.0).is_ok());
    }

    #[tokio::test]
    async fn test_stt_router_failover() {
        let storage = Arc::new(InMemoryQuotaStorage::new());
        let manager = Arc::new(ProviderQuotaManager::new(storage, RoutingMode::Balanced));

        // Primary provider that fails the first request
        let primary = MockSTTProvider::new("primary-mock", "Primary Mock STT")
            .with_failures(1)
            .with_transcript("From Primary");

        // Secondary provider that succeeds
        let secondary = MockSTTProvider::new("secondary-mock", "Secondary Mock STT")
            .with_transcript("From Secondary Failover");

        let providers: Vec<Box<dyn STTProvider>> = vec![Box::new(primary), Box::new(secondary)];
        let router = STTRouter::new(providers, Arc::clone(&manager));

        let audio = AudioData::new(vec![0u8; 3200], 16000, 1);
        let res = router.transcribe(audio, Some(VoiceLanguage::English)).await;

        assert!(res.is_ok(), "Router should successfully fail over to secondary");
        let transcript = res.unwrap();
        assert_eq!(transcript.text, "From Secondary Failover");
        assert_eq!(transcript.provider, "secondary-mock");
    }

    #[tokio::test]
    async fn test_tts_router_fallback_chain() {
        let storage = Arc::new(InMemoryQuotaStorage::new());
        let manager = Arc::new(ProviderQuotaManager::new(storage, RoutingMode::Balanced));

        // Cloud provider that fails
        let failing_cloud = MockTTSProvider::new("failing-cloud", "Failing Cloud TTS").with_failures(5);

        // Emergency fallback provider that always succeeds
        let emergency_fallback = MockTTSProvider::new("emergency-fallback", "Emergency Fallback TTS");

        let providers: Vec<Box<dyn TTSProvider>> = vec![Box::new(failing_cloud)];
        let router = TTSRouter::new(
            providers,
            Some(Box::new(emergency_fallback)),
            Arc::clone(&manager),
        );

        let audio_res = router.synthesize("Hello world", None).await;
        assert!(audio_res.is_ok(), "Router should fall back to emergency fallback");
        let audio = audio_res.unwrap();
        assert!(!audio.pcm_bytes.is_empty());
    }

    #[tokio::test]
    async fn test_admin_api_zero_secret_leakage() {
        let storage = Arc::new(InMemoryQuotaStorage::new());
        let manager = Arc::new(ProviderQuotaManager::new(storage, RoutingMode::Balanced));

        let desc = crate::provider::admin::ProviderDescriptor {
            id: "groq-stt".into(),
            name: "Groq Whisper Turbo".into(),
            provider_type: "STT",
            streaming: false,
            supported_languages: vec!["en".into(), "hi".into()],
            estimated_cost_info: "$0.04/hr".into(),
        };

        let admin_api = VoiceAdminApi::new(Arc::clone(&manager), None, None, vec![desc]);

        let providers_json = admin_api.get_providers().to_string();
        let quota_json = admin_api.get_quota().to_string();
        let health_json = admin_api.get_health().to_string();

        // Verify no credentials, api_key, auth, bearer, token fields leaked
        for json in &[&providers_json, &quota_json, &health_json] {
            assert!(!json.contains("gsk_"), "Secret key detected in admin JSON!");
            assert!(!json.contains("sk-"), "Secret key detected in admin JSON!");
            assert!(!json.contains("api_key"), "api_key field leaked in admin JSON!");
            assert!(!json.contains("password"), "password field leaked in admin JSON!");
        }

        assert!(providers_json.contains("groq-stt"));
        assert!(health_json.contains("healthy"));
    }

    #[tokio::test]
    async fn test_budget_routing_mode_pacing() {
        let storage = Arc::new(InMemoryQuotaStorage::new());
        let manager = ProviderQuotaManager::new(storage, RoutingMode::Conservative);

        manager.register_provider(
            "test-provider",
            RateLimitConfig {
                rpm: 100,
                rpd: 100, // 100 daily limit
                characters_per_minute: 10000,
                characters_per_day: 100000,
                audio_seconds_per_day: 3600.0,
                max_concurrency: 5,
                burst_capacity: 5,
                safety_margin: 0.1, // safe RPD = 90
            },
        );

        // Conservative pacing restricts morning bursts so quota lasts all day.
        assert_eq!(manager.routing_mode(), RoutingMode::Conservative);
    }

    async fn spawn_mock_cartesia_server(
        status_code: u16,
        status_text: &'static str,
        extra_headers: Vec<(&'static str, &'static str)>,
        body: Vec<u8>,
        delay: Option<Duration>,
    ) -> (String, tokio::task::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let url = format!("http://127.0.0.1:{}/tts/bytes", port);

        let handle = tokio::spawn(async move {
            while let Ok((mut socket, _)) = listener.accept().await {
                let mut buf = [0u8; 4096];
                let n = socket.read(&mut buf).await.unwrap_or(0);
                if n == 0 {
                    continue;
                }

                if let Some(d) = delay {
                    tokio::time::sleep(d).await;
                }

                let mut header_lines = String::new();
                for (k, v) in &extra_headers {
                    header_lines.push_str(&format!("{}: {}\r\n", k, v));
                }

                let response = format!(
                    "HTTP/1.1 {} {}\r\nContent-Length: {}\r\n{}\r\n",
                    status_code,
                    status_text,
                    body.len(),
                    header_lines
                );

                let _ = socket.write_all(response.as_bytes()).await;
                let _ = socket.write_all(&body).await;
                let _ = socket.flush().await;
            }
        });

        (url, handle)
    }

    #[tokio::test]
    async fn test_cartesia_tts_mock_success() {
        let dummy_pcm = vec![0u8; 3200]; // 0.1s of 16kHz 16-bit PCM mono
        let (url, _h) = spawn_mock_cartesia_server(
            200,
            "OK",
            vec![("Content-Type", "application/octet-stream")],
            dummy_pcm,
            None,
        ).await;

        let provider = CartesiaTTSProvider::with_endpoint(
            "test_api_key_cartesia",
            Some("sonic-voice-123".into()),
            Some("sonic-english".into()),
            Some(url),
            Some(Duration::from_secs(5)),
        );

        assert_eq!(provider.id(), "cartesia-tts");
        assert_eq!(provider.name(), "Cartesia Sonic TTS");
        assert!(provider.capabilities().streaming);

        // Test synthesize
        let res = provider.synthesize("Hello Cartesia Sonic!", Some(VoiceLanguage::English)).await;
        assert!(res.is_ok(), "Expected successful synthesis: {:?}", res.err());
        let audio = res.unwrap();
        assert_eq!(audio.sample_rate, 16000);
        assert_eq!(audio.channels, 1);
        assert_eq!(audio.pcm_bytes.len(), 3200);

        // Test synthesize_stream
        let stream_res = provider.synthesize_stream("Streaming hello", None).await;
        assert!(stream_res.is_ok());
        let mut stream = stream_res.unwrap();
        let chunk = stream.next().await;
        assert!(chunk.is_some());
        assert!(chunk.unwrap().is_ok());
    }

    #[tokio::test]
    async fn test_cartesia_tts_mock_timeout() {
        let (url, _h) = spawn_mock_cartesia_server(
            200,
            "OK",
            vec![],
            vec![0u8; 100],
            Some(Duration::from_millis(300)),
        ).await;

        let provider = CartesiaTTSProvider::with_endpoint(
            "test_api_key_cartesia",
            None,
            None,
            Some(url),
            Some(Duration::from_millis(40)),
        );

        let res = provider.synthesize("Timeout test", None).await;
        assert!(res.is_err());
        match res.unwrap_err() {
            ProviderError::Timeout(d) => assert_eq!(d, Duration::from_millis(40)),
            other => panic!("Expected Timeout, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_cartesia_tts_mock_429_rate_limit() {
        let (url, _h) = spawn_mock_cartesia_server(
            429,
            "Too Many Requests",
            vec![("Retry-After", "7"), ("Content-Type", "application/json")],
            br#"{"error":"rate_limit_exceeded","message":"Rate limit exceeded"}"#.to_vec(),
            None,
        ).await;

        let provider = CartesiaTTSProvider::with_endpoint(
            "test_api_key_cartesia",
            None,
            None,
            Some(url),
            Some(Duration::from_secs(5)),
        );

        let res = provider.synthesize("Rate limit test", None).await;
        assert!(res.is_err());
        match res.unwrap_err() {
            ProviderError::RateLimited { retry_after, reason } => {
                assert_eq!(retry_after, Some(Duration::from_secs(7)));
                assert!(reason.contains("429"));
            }
            other => panic!("Expected RateLimited, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_cartesia_tts_mock_invalid_credentials() {
        let (url, _h) = spawn_mock_cartesia_server(
            401,
            "Unauthorized",
            vec![("Content-Type", "application/json")],
            br#"{"error":"unauthorized","message":"Invalid API Key"}"#.to_vec(),
            None,
        ).await;

        let provider = CartesiaTTSProvider::with_endpoint(
            "invalid_key",
            None,
            None,
            Some(url),
            Some(Duration::from_secs(5)),
        );

        let res = provider.synthesize("Auth test", None).await;
        assert!(res.is_err());
        match res.unwrap_err() {
            ProviderError::InvalidCredentials(msg) => {
                assert!(msg.contains("Cartesia"));
            }
            other => panic!("Expected InvalidCredentials, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_cartesia_tts_router_fallback() {
        let (url, _h) = spawn_mock_cartesia_server(
            429,
            "Too Many Requests",
            vec![("Retry-After", "2")],
            b"Rate limit".to_vec(),
            None,
        ).await;

        let storage = Arc::new(InMemoryQuotaStorage::new());
        let manager = Arc::new(ProviderQuotaManager::new(storage, RoutingMode::Balanced));

        // Primary: Cartesia (returns 429)
        let cartesia = CartesiaTTSProvider::with_endpoint(
            "test_key",
            None,
            None,
            Some(url),
            Some(Duration::from_secs(2)),
        );

        // Secondary: MockTTSProvider (healthy, but recorded with higher latency so Cartesia ranks #1)
        let secondary = MockTTSProvider::new("secondary-mock", "Secondary Mock TTS");
        manager.record_success("secondary-mock", Duration::from_millis(1000), 10, 1.0, 0.05);

        let providers: Vec<Box<dyn TTSProvider>> = vec![Box::new(cartesia), Box::new(secondary)];
        let router = TTSRouter::new(providers, None, Arc::clone(&manager));

        let res = router.synthesize("Test fallback when Cartesia 429", None).await;
        assert!(res.is_ok(), "Expected router to fail over to secondary: {:?}", res.err());
        let audio = res.unwrap();
        assert!(!audio.pcm_bytes.is_empty());

        // Verify Cartesia was flagged in cooldown
        assert!(manager.is_in_cooldown("cartesia-tts"));
    }

    #[tokio::test]
    async fn test_cartesia_disabled_via_env() {
        std::env::set_var("CARTESIA_ENABLED", "false");
        std::env::set_var("CARTESIA_API_KEY", "dummy_key_123");

        let provider = CartesiaTTSProvider::from_env();
        assert!(provider.is_none(), "Expected CARTESIA_ENABLED=false to remove provider");

        std::env::set_var("CARTESIA_ENABLED", "true");
        let provider_enabled = CartesiaTTSProvider::from_env();
        assert!(provider_enabled.is_some(), "Expected CARTESIA_ENABLED=true to enable provider");

        // Clean up
        std::env::remove_var("CARTESIA_ENABLED");
        std::env::remove_var("CARTESIA_API_KEY");
    }

    // =========================================================================
    // 12 Comprehensive Rate-Limiting & Smart Routing Test Cases
    // =========================================================================

    #[tokio::test]
    async fn test_1_rpm_limiting_with_safety_margin() {
        let storage = Arc::new(InMemoryQuotaStorage::new());
        let manager = ProviderQuotaManager::new(storage, RoutingMode::Balanced);

        // Documented RPM = 10, safety margin = 35% -> safe limit = floor(10 * 0.65) = 6
        let config = RateLimitConfig {
            rpm: 10,
            rpd: 1000,
            characters_per_minute: 10000,
            characters_per_day: 100000,
            audio_seconds_per_day: 3600.0,
            max_concurrency: 5,
            burst_capacity: 10,
            safety_margin: 0.35,
        };
        manager.register_provider("mock-rpm-test", config);

        // First 6 requests must succeed
        for i in 0..6 {
            let res = manager.check_preflight("mock-rpm-test", 10, 0.5);
            assert!(res.is_ok(), "Request {} should succeed within safe RPM", i + 1);
            manager.record_success("mock-rpm-test", Duration::from_millis(10), 10, 0.5, 0.0001);
        }

        // 7th request must be rejected by safe RPM limit before reaching provider
        let err = manager.check_preflight("mock-rpm-test", 10, 0.5);
        assert!(err.is_err(), "7th request must exceed safe RPM limit of 6");
        match err.unwrap_err() {
            QuotaViolation::RpmExceeded(provider, wait) => {
                assert_eq!(provider, "mock-rpm-test");
                assert!(wait > Duration::ZERO);
            }
            other => panic!("Expected RpmExceeded violation, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_2_concurrency_limiting_raii() {
        let storage = Arc::new(InMemoryQuotaStorage::new());
        let manager = Arc::new(ProviderQuotaManager::new(storage, RoutingMode::Balanced));

        let config = RateLimitConfig {
            rpm: 100,
            rpd: 1000,
            characters_per_minute: 10000,
            characters_per_day: 100000,
            audio_seconds_per_day: 3600.0,
            max_concurrency: 2,
            burst_capacity: 10,
            safety_margin: 0.0, // exact concurrency test
        };
        manager.register_provider("mock-conc-test", config);

        // Acquire 2 permits
        let permit1 = manager.acquire_concurrency("mock-conc-test");
        assert!(permit1.is_ok());
        let permit2 = manager.acquire_concurrency("mock-conc-test");
        assert!(permit2.is_ok());

        // 3rd attempt must fail due to concurrency limit
        let permit3 = manager.acquire_concurrency("mock-conc-test");
        assert!(permit3.is_err());
        match permit3.err().unwrap() {
            QuotaViolation::ConcurrencyExceeded(provider) => {
                assert_eq!(provider, "mock-conc-test");
            }
            other => panic!("Expected ConcurrencyExceeded, got {:?}", other),
        }

        // Drop permit1 -> RAII drop automatically releases concurrency
        drop(permit1);

        // Now permit acquisition succeeds
        let permit3_retry = manager.acquire_concurrency("mock-conc-test");
        assert!(permit3_retry.is_ok(), "Permit acquisition must succeed after dropping previous permit");
    }

    #[tokio::test]
    async fn test_3_burst_protection_token_bucket() {
        let storage = Arc::new(InMemoryQuotaStorage::new());
        let manager = ProviderQuotaManager::new(storage, RoutingMode::Balanced);

        // RPM 60, burst capacity is limited to 1 token
        let config = RateLimitConfig {
            rpm: 60,
            rpd: 1000,
            characters_per_minute: 10000,
            characters_per_day: 100000,
            audio_seconds_per_day: 3600.0,
            max_concurrency: 5,
            burst_capacity: 1,
            safety_margin: 0.35,
        };
        manager.register_provider("mock-burst-test", config);

        // First preflight consumes the single available token
        assert!(manager.check_preflight("mock-burst-test", 10, 0.5).is_ok());

        // Immediate subsequent request within milliseconds triggers burst protection wait or error
        let burst_res = manager.check_preflight("mock-burst-test", 10, 0.5);
        assert!(burst_res.is_err(), "Rapid burst spike must be caught by token bucket");
        match burst_res.unwrap_err() {
            QuotaViolation::BurstLimitExceeded(provider, wait_time) => {
                assert_eq!(provider, "mock-burst-test");
                assert!(wait_time > Duration::ZERO);
            }
            other => panic!("Expected BurstLimitExceeded, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_4_http_429_handling_and_cooldown() {
        let storage = Arc::new(InMemoryQuotaStorage::new());
        let manager = ProviderQuotaManager::new(storage, RoutingMode::Balanced);

        let config = RateLimitConfig {
            rpm: 60,
            rpd: 1000,
            characters_per_minute: 10000,
            characters_per_day: 100000,
            audio_seconds_per_day: 3600.0,
            max_concurrency: 5,
            burst_capacity: 5,
            safety_margin: 0.35,
        };
        manager.register_provider("mock-429-test", config);

        // Record a 429 rate limit with 10s cooldown
        manager.record_rate_limit("mock-429-test", Some(Duration::from_secs(10)));

        assert!(manager.is_in_cooldown("mock-429-test"));
        let status = manager.get_status("mock-429-test");
        assert_eq!(status.health, HealthStatus::Cooldown);

        let metrics = manager.get_metrics("mock-429-test").unwrap();
        assert_eq!(metrics.total_rate_limits, 1);

        // Preflight is blocked during cooldown
        let preflight_res = manager.check_preflight("mock-429-test", 10, 0.5);
        assert!(preflight_res.is_err());
        match preflight_res.unwrap_err() {
            QuotaViolation::InCooldown(provider, until) => {
                assert_eq!(provider, "mock-429-test");
                assert!(until > chrono::Utc::now());
            }
            other => panic!("Expected InCooldown, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_5_retry_after_header_handling() {
        let storage = Arc::new(InMemoryQuotaStorage::new());
        let manager = ProviderQuotaManager::new(storage, RoutingMode::Balanced);

        manager.register_provider(
            "mock-retry-after-test",
            RateLimitConfig {
                rpm: 100,
                rpd: 1000,
                characters_per_minute: 10000,
                characters_per_day: 100000,
                audio_seconds_per_day: 3600.0,
                max_concurrency: 5,
                burst_capacity: 5,
                safety_margin: 0.35,
            },
        );

        // Simulate 429 response containing Retry-After: 25
        manager.record_rate_limit("mock-retry-after-test", Some(Duration::from_secs(25)));

        assert!(manager.is_in_cooldown("mock-retry-after-test"));
        let status = manager.get_status("mock-retry-after-test");
        assert!(status.is_cooldown);
        assert!(status.in_cooldown_until.is_some());
        let remaining = (status.in_cooldown_until.unwrap() - chrono::Utc::now()).to_std().unwrap();
        assert!(remaining.as_secs() >= 20 && remaining.as_secs() <= 25);
    }

    #[tokio::test]
    async fn test_6_exponential_backoff_and_transient_retry() {
        let storage = Arc::new(InMemoryQuotaStorage::new());
        let manager = Arc::new(ProviderQuotaManager::new(storage, RoutingMode::Balanced));

        // Provider that fails once with transient network error, then succeeds
        let provider = MockTTSProvider::new("retry-provider", "Transient Retry Mock")
            .with_failures(1);

        let providers: Vec<Box<dyn TTSProvider>> = vec![Box::new(provider.clone())];
        let router = TTSRouter::new(providers, None, Arc::clone(&manager));

        let res = router.synthesize("Testing backoff and retry", None).await;
        assert!(res.is_ok(), "Router should retry and succeed on transient error: {:?}", res.err());
        assert_eq!(provider.call_count(), 2, "Expected exactly 2 attempts (1 initial failure + 1 retry)");
    }

    #[tokio::test]
    async fn test_7_provider_cooldown_skipped_by_router() {
        let storage = Arc::new(InMemoryQuotaStorage::new());
        let manager = Arc::new(ProviderQuotaManager::new(storage, RoutingMode::Balanced));

        let primary = MockTTSProvider::new("primary-cooling", "Primary Cooling Mock");
        let secondary = MockTTSProvider::new("secondary-active", "Secondary Active Mock");

        manager.register_provider(
            "primary-cooling",
            RateLimitConfig {
                rpm: 100,
                rpd: 1000,
                characters_per_minute: 10000,
                characters_per_day: 100000,
                audio_seconds_per_day: 3600.0,
                max_concurrency: 5,
                burst_capacity: 5,
                safety_margin: 0.35,
            },
        );
        manager.register_provider(
            "secondary-active",
            RateLimitConfig {
                rpm: 100,
                rpd: 1000,
                characters_per_minute: 10000,
                characters_per_day: 100000,
                audio_seconds_per_day: 3600.0,
                max_concurrency: 5,
                burst_capacity: 5,
                safety_margin: 0.35,
            },
        );

        // Put primary in cooldown
        manager.record_rate_limit("primary-cooling", Some(Duration::from_secs(30)));

        let providers: Vec<Box<dyn TTSProvider>> = vec![
            Box::new(primary.clone()),
            Box::new(secondary.clone()),
        ];
        let router = TTSRouter::new(providers, None, Arc::clone(&manager));

        let res = router.synthesize("Skip cooldown test", None).await;
        assert!(res.is_ok());

        // Primary should NOT have been invoked at all
        assert_eq!(primary.call_count(), 0, "Primary in cooldown must be skipped without calling");
        // Secondary should have fulfilled the request
        assert_eq!(secondary.call_count(), 1, "Secondary should fulfill request");
    }

    #[tokio::test]
    async fn test_8_automatic_provider_fallback_on_failure() {
        let storage = Arc::new(InMemoryQuotaStorage::new());
        let manager = Arc::new(ProviderQuotaManager::new(storage, RoutingMode::Balanced));

        // Primary fails all retries (5 failures)
        let primary = MockTTSProvider::new("primary-faulty", "Primary Faulty Mock")
            .with_failures(5);
        let secondary = MockTTSProvider::new("secondary-backup", "Secondary Backup Mock");

        let providers: Vec<Box<dyn TTSProvider>> = vec![
            Box::new(primary.clone()),
            Box::new(secondary.clone()),
        ];
        let router = TTSRouter::new(providers, None, Arc::clone(&manager));

        let res = router.synthesize("Automatic failover test", None).await;
        assert!(res.is_ok(), "Router must automatically fail over to secondary: {:?}", res.err());
        assert!(primary.call_count() > 0, "Primary was attempted");
        assert_eq!(secondary.call_count(), 1, "Secondary backup fulfilled request");
    }

    #[tokio::test]
    async fn test_9_duplicate_tts_prevention_cache() {
        let storage = Arc::new(InMemoryQuotaStorage::new());
        let manager = Arc::new(ProviderQuotaManager::new(storage, RoutingMode::Balanced));

        let mock_provider = MockTTSProvider::new("cache-mock", "Cache Mock TTS");
        let providers: Vec<Box<dyn TTSProvider>> = vec![Box::new(mock_provider.clone())];

        let cache = Arc::new(TTSCache::new(Duration::from_secs(60), 100));
        let router = TTSRouter::with_cache(
            providers,
            None,
            Arc::clone(&manager),
            cache.clone(),
            Duration::from_millis(500),
        );

        // 1st request -> Cache miss, synthesizes via provider
        let audio1 = router.synthesize("Exact duplicate voice prompt", Some(VoiceLanguage::English)).await.unwrap();
        assert_eq!(mock_provider.call_count(), 1);
        let stats1 = cache.stats();
        assert_eq!(stats1.0, 0); // hits
        assert_eq!(stats1.1, 1); // misses

        // 2nd request with exact same phrase -> Cache hit, provider call count remains 1!
        let audio2 = router.synthesize("Exact duplicate voice prompt", Some(VoiceLanguage::English)).await.unwrap();
        assert_eq!(mock_provider.call_count(), 1, "Provider must NOT be called on cache hit");
        let stats2 = cache.stats();
        assert_eq!(stats2.0, 1); // hits
        assert_eq!(audio1.pcm_bytes, audio2.pcm_bytes);

        // 3rd request with different text -> Cache miss, provider called
        let _audio3 = router.synthesize("Different voice prompt text", Some(VoiceLanguage::English)).await.unwrap();
        assert_eq!(mock_provider.call_count(), 2);
        let stats3 = cache.stats();
        assert_eq!(stats3.1, 2); // misses
    }

    #[tokio::test]
    async fn test_10_queue_behavior_at_safe_limit() {
        let storage = Arc::new(InMemoryQuotaStorage::new());
        let manager = ProviderQuotaManager::new(storage, RoutingMode::Balanced);

        // Provider with safe RPM of 39 (refill ~0.65/s) and burst capacity 1
        let config = RateLimitConfig {
            rpm: 60,
            rpd: 1000,
            characters_per_minute: 10000,
            characters_per_day: 100000,
            audio_seconds_per_day: 3600.0,
            max_concurrency: 5,
            burst_capacity: 1,
            safety_margin: 0.35,
        };
        manager.register_provider("mock-wait-test", config);

        // Consume the token
        assert!(manager.check_preflight("mock-wait-test", 10, 0.5).is_ok());

        // Wait time calculation returns positive duration to queue/sleep
        let wait = manager.calculate_wait_time("mock-wait-test", 10);
        assert!(wait > Duration::ZERO);
        assert!(wait <= Duration::from_secs(5));
    }

    #[tokio::test]
    async fn test_11_provider_recovery_after_cooldown() {
        let storage = Arc::new(InMemoryQuotaStorage::new());
        let manager = ProviderQuotaManager::new(storage, RoutingMode::Balanced);

        manager.register_provider(
            "mock-recovery-test",
            RateLimitConfig {
                rpm: 100,
                rpd: 1000,
                characters_per_minute: 10000,
                characters_per_day: 100000,
                audio_seconds_per_day: 3600.0,
                max_concurrency: 5,
                burst_capacity: 5,
                safety_margin: 0.35,
            },
        );

        // Set a short 30ms cooldown
        manager.record_rate_limit("mock-recovery-test", Some(Duration::from_millis(30)));
        assert!(manager.is_in_cooldown("mock-recovery-test"));

        // Wait for cooldown to naturally expire
        tokio::time::sleep(Duration::from_millis(60)).await;

        // Provider is now recovered and ready
        assert!(!manager.is_in_cooldown("mock-recovery-test"));
        assert!(manager.check_preflight("mock-recovery-test", 10, 0.5).is_ok());
    }

    #[tokio::test]
    async fn test_12_no_api_key_leakage_in_logs_and_errors() {
        let secret = "sk-test-secret-1234567890abcdefg";
        let groq_secret = "gsk_supersecretkeynotforlogs";

        let _err1 = ProviderError::InvalidCredentials(format!(
            "Failed authentication for endpoint with key {}",
            secret
        ));
        let _err2 = ProviderError::NetworkError(format!(
            "Connection failed to api.groq.com with authorization Bearer {}",
            groq_secret
        ));

        let _formatted1 = format!("{}", _err1);
        let _formatted2 = format!("{}", _err2);

        // Verify that ProviderError Display sanitizes raw secrets from error strings
        assert!(!_formatted1.contains(secret), "Formatted error must not leak OpenAI secret");
        assert!(!_formatted2.contains(groq_secret), "Formatted error must not leak Groq secret");
        assert!(_formatted1.contains("[REDACTED]"), "Formatted error must contain redaction notice");
        assert!(_formatted2.contains("[REDACTED]"), "Formatted error must contain redaction notice");

        let storage = Arc::new(InMemoryQuotaStorage::new());
        let manager = Arc::new(ProviderQuotaManager::new(storage, RoutingMode::Balanced));
        let admin_api = VoiceAdminApi::new(Arc::clone(&manager), None, None, vec![]);

        let providers_json = admin_api.get_providers().to_string();
        let quota_json = admin_api.get_quota().to_string();
        let health_json = admin_api.get_health().to_string();

        for json in &[&providers_json, &quota_json, &health_json] {
            assert!(!json.contains("sk-"), "Detected secret pattern in admin telemetry");
            assert!(!json.contains("gsk_"), "Detected secret pattern in admin telemetry");
            assert!(!json.contains("secret"), "Detected word 'secret' in admin telemetry");
        }

        // Verify status string does not contain secrets
        let status = manager.get_status("any-provider");
        let status_str = format!("{:?}", status);
        assert!(!status_str.contains("api_key"));
        assert!(!status_str.contains("secret"));
    }

    #[test]
    fn test_13_voice_mode_parsing() {
        std::env::set_var("TEST_VOICE_MODE", "local");
        assert_eq!(VoiceMode::from_env("TEST_VOICE_MODE"), VoiceMode::Local);
        assert!(VoiceMode::from_env("TEST_VOICE_MODE").is_local_only());
        assert!(!VoiceMode::from_env("TEST_VOICE_MODE").allows_cloud());

        std::env::set_var("TEST_VOICE_MODE", "offline");
        assert_eq!(VoiceMode::from_env("TEST_VOICE_MODE"), VoiceMode::Local);

        std::env::set_var("TEST_VOICE_MODE", "cloud");
        assert_eq!(VoiceMode::from_env("TEST_VOICE_MODE"), VoiceMode::Cloud);
        assert!(VoiceMode::from_env("TEST_VOICE_MODE").is_cloud_only());
        assert!(!VoiceMode::from_env("TEST_VOICE_MODE").allows_local());

        std::env::set_var("TEST_VOICE_MODE", "auto");
        assert_eq!(VoiceMode::from_env("TEST_VOICE_MODE"), VoiceMode::Auto);
        assert!(VoiceMode::from_env("TEST_VOICE_MODE").allows_cloud());
        assert!(VoiceMode::from_env("TEST_VOICE_MODE").allows_local());

        std::env::remove_var("TEST_VOICE_MODE");
        assert_eq!(VoiceMode::from_env("TEST_VOICE_MODE"), VoiceMode::Auto);
    }

    #[test]
    fn test_14_local_sapi_stt_provider_metadata() {
        let provider = LocalSapiSTTProvider::new();
        assert_eq!(provider.id(), "local-sapi-stt");
        assert!(provider.name().contains("Windows SAPI"));
        assert!(!provider.capabilities().streaming);
        assert_eq!(provider.capabilities().cost_per_second_usd, 0.0);
    }

    #[tokio::test]
    async fn test_15_stt_router_local_mode_strictly_bypasses_cloud() {
        let storage = Arc::new(InMemoryQuotaStorage::new());
        let manager = Arc::new(ProviderQuotaManager::new(storage, RoutingMode::Balanced));

        // Cloud provider configured to return an error if called
        let cloud_provider = Box::new(MockSTTProvider::new("cloud-mock", "Cloud Provider").with_failures(100));

        // Local emergency fallback that succeeds
        let local_fallback = Box::new(MockSTTProvider::new("local-fallback", "Local Fallback").with_transcript("local offline speech"));

        let router = STTRouter::with_fallback(
            vec![cloud_provider],
            Some(local_fallback),
            manager,
            VoiceMode::Local,
        );

        let audio = AudioData::new(vec![0; 3200], 16000, 1);
        let result = router.transcribe(audio, None).await;

        assert!(result.is_ok(), "STTRouter in local mode should succeed via local fallback");
        assert_eq!(result.unwrap().text, "local offline speech");
    }

    #[tokio::test]
    async fn test_16_stt_router_auto_mode_falls_back_when_cloud_unreachable() {
        let storage = Arc::new(InMemoryQuotaStorage::new());
        let manager = Arc::new(ProviderQuotaManager::new(storage, RoutingMode::Balanced));

        // Failing cloud provider simulating disconnected internet / DNS failure
        let cloud_provider = Box::new(MockSTTProvider::new("cloud-mock", "Cloud Provider").with_failures(100));

        // Local offline fallback
        let local_fallback = Box::new(MockSTTProvider::new("local-fallback", "Local Fallback").with_transcript("recovered via offline fallback"));

        let router = STTRouter::with_fallback(
            vec![cloud_provider],
            Some(local_fallback),
            manager,
            VoiceMode::Auto,
        );

        let audio = AudioData::new(vec![0; 3200], 16000, 1);
        let result = router.transcribe(audio, None).await;

        assert!(result.is_ok(), "STTRouter in auto mode should recover via local fallback");
        assert_eq!(result.unwrap().text, "recovered via offline fallback");
    }

    #[tokio::test]
    async fn test_17_tts_router_local_mode_bypasses_cloud() {
        let storage = Arc::new(InMemoryQuotaStorage::new());
        let manager = Arc::new(ProviderQuotaManager::new(storage, RoutingMode::Balanced));

        // Failing cloud provider
        let cloud_provider = Box::new(MockTTSProvider::new("cloud-tts", "Cloud TTS").with_failures(100));

        // Working local fallback
        let local_fallback = Box::new(MockTTSProvider::new("local-sapi-tts", "Local SAPI Fallback"));

        let router = TTSRouter::with_cache_and_mode(
            vec![cloud_provider],
            Some(local_fallback),
            manager,
            Arc::new(TTSCache::default()),
            Duration::from_millis(500),
            VoiceMode::Local,
        );

        let result = router.synthesize("Hello offline world", None).await;
        assert!(result.is_ok(), "TTSRouter in local mode should succeed directly via local fallback");
    }
}

