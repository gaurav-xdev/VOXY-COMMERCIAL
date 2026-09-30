use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use parking_lot::RwLock;
use tracing::{debug, warn};
use voxy_voice_orchestrator::{AudioChunk, AudioStream, SttEngine};

#[derive(Debug, Clone)]
pub struct EngineCapability {
    pub name: String,
    pub has_gpu: bool,
    pub has_cloud: bool,
}

pub struct SttRouter {
    engines: RwLock<Vec<Arc<dyn SttEngine>>>,
    capabilities: RwLock<Vec<EngineCapability>>,
    active_index: AtomicUsize,
}

impl SttRouter {
    pub fn new() -> Self {
        Self {
            engines: RwLock::new(Vec::new()),
            capabilities: RwLock::new(Vec::new()),
            active_index: AtomicUsize::new(0),
        }
    }

    pub fn add_engine(&self, engine: Box<dyn SttEngine>, capability: EngineCapability) {
        self.engines.write().push(Arc::from(engine));
        self.capabilities.write().push(capability);
    }

    pub fn set_engines(&self, engines: Vec<(Box<dyn SttEngine>, EngineCapability)>) {
        let mut eng = self.engines.write();
        let mut caps = self.capabilities.write();
        eng.clear();
        caps.clear();
        for (engine, cap) in engines {
            eng.push(Arc::from(engine));
            caps.push(cap);
        }
        self.active_index.store(0, Ordering::SeqCst);
    }

    pub fn active_engine_name(&self) -> Option<String> {
        let engines = self.engines.read();
        let idx = self.active_index.load(Ordering::SeqCst);
        engines.get(idx).map(|e| e.name().to_string())
    }

    pub fn capabilities(&self) -> Vec<EngineCapability> {
        self.capabilities.read().clone()
    }

    pub fn set_active_index(&self, index: usize) -> bool {
        let len = self.engines.read().len();
        if index < len {
            self.active_index.store(index, Ordering::SeqCst);
            true
        } else {
            false
        }
    }

    pub fn engine_count(&self) -> usize {
        self.engines.read().len()
    }
}

impl Default for SttRouter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl SttEngine for SttRouter {
    fn name(&self) -> &str {
        "stt-router"
    }

    async fn transcribe(
        &self,
        audio: &AudioChunk,
    ) -> Result<String, voxy_voice_orchestrator::VoiceOrchestratorError> {
        let snapshot: Vec<(Arc<dyn SttEngine>, bool)> = {
            let engines = self.engines.read();
            if engines.is_empty() {
                return Err(voxy_voice_orchestrator::VoiceOrchestratorError::NoSttEngine);
            }
            engines
                .iter()
                .map(|e| (Arc::clone(e), e.is_available()))
                .collect()
        };

        let last_idx = snapshot.len() - 1;

        for (i, (engine, available)) in snapshot.iter().enumerate() {
            if !available {
                debug!("Engine '{}' not available, trying next", engine.name());
                continue;
            }

            let result = engine.transcribe(audio).await;

            match result {
                Ok(text) if !text.is_empty() => {
                    self.active_index.store(i, Ordering::SeqCst);
                    debug!("Transcription succeeded with '{}'", engine.name());
                    return Ok(text);
                }
                Ok(_) => {
                    debug!("Engine '{}' returned empty, trying next", engine.name());
                }
                Err(e) => {
                    warn!("Engine '{}' failed: {}, trying next", engine.name(), e);
                }
            }

            if i == last_idx {
                break;
            }
        }

        let active_name = self
            .engines
            .read()
            .get(self.active_index.load(Ordering::SeqCst))
            .map(|e| e.name().to_string())
            .unwrap_or_default();

        Err(
            voxy_voice_orchestrator::VoiceOrchestratorError::TranscriptionFailed(format!(
                "All {} engines failed for '{}'",
                snapshot.len(),
                active_name
            )),
        )
    }

    async fn transcribe_stream(
        &self,
        mut stream: Box<dyn AudioStream>,
    ) -> Result<String, voxy_voice_orchestrator::VoiceOrchestratorError> {
        let mut accumulated = Vec::new();
        let sr = stream.sample_rate();

        while let Some(chunk) = stream.next_chunk().await {
            accumulated.extend_from_slice(&chunk.data);
        }

        if accumulated.is_empty() {
            return Ok(String::new());
        }

        let fake_chunk = AudioChunk {
            data: accumulated,
            sample_rate: sr,
            channels: 1,
            timestamp: chrono::Utc::now(),
            sequence: 0,
            is_final: true,
        };

        self.transcribe(&fake_chunk).await
    }

    fn supported_languages(&self) -> Vec<String> {
        let engines = self.engines.read();
        let mut all_langs = Vec::new();
        for engine in engines.iter() {
            for lang in engine.supported_languages() {
                if !all_langs.contains(&lang) {
                    all_langs.push(lang);
                }
            }
        }
        all_langs
    }

    fn is_available(&self) -> bool {
        let engines = self.engines.read();
        engines.iter().any(|e| e.is_available())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    struct MockStt {
        name: String,
        available: bool,
        ok_text: Option<String>,
        err_msg: Option<String>,
    }

    impl MockStt {
        fn ok(name: &str, text: &str) -> Self {
            Self {
                name: name.into(),
                available: true,
                ok_text: Some(text.into()),
                err_msg: None,
            }
        }

        fn err(name: &str, msg: &str) -> Self {
            Self {
                name: name.into(),
                available: true,
                ok_text: None,
                err_msg: Some(msg.into()),
            }
        }

        fn with_available(mut self, available: bool) -> Self {
            self.available = available;
            self
        }
    }

    #[async_trait]
    impl SttEngine for MockStt {
        fn name(&self) -> &str {
            &self.name
        }

        async fn transcribe(
            &self,
            _audio: &AudioChunk,
        ) -> Result<String, voxy_voice_orchestrator::VoiceOrchestratorError> {
            if let Some(ref text) = self.ok_text {
                Ok(text.clone())
            } else {
                Err(
                    voxy_voice_orchestrator::VoiceOrchestratorError::TranscriptionFailed(
                        self.err_msg.clone().unwrap_or_default(),
                    ),
                )
            }
        }

        async fn transcribe_stream(
            &self,
            _stream: Box<dyn AudioStream>,
        ) -> Result<String, voxy_voice_orchestrator::VoiceOrchestratorError> {
            if let Some(ref text) = self.ok_text {
                Ok(text.clone())
            } else {
                Err(
                    voxy_voice_orchestrator::VoiceOrchestratorError::TranscriptionFailed(
                        self.err_msg.clone().unwrap_or_default(),
                    ),
                )
            }
        }

        fn supported_languages(&self) -> Vec<String> {
            vec!["en".into()]
        }

        fn is_available(&self) -> bool {
            self.available
        }
    }

    fn make_chunk() -> AudioChunk {
        AudioChunk {
            data: vec![0.1; 160],
            sample_rate: 16000,
            channels: 1,
            timestamp: Utc::now(),
            sequence: 0,
            is_final: true,
        }
    }

    #[tokio::test]
    async fn test_router_empty_returns_no_engine() {
        let router = SttRouter::new();
        let chunk = make_chunk();
        let result = router.transcribe(&chunk).await;
        assert!(matches!(
            result,
            Err(voxy_voice_orchestrator::VoiceOrchestratorError::NoSttEngine)
        ));
    }

    #[tokio::test]
    async fn test_router_single_engine_success() {
        let router = SttRouter::new();
        router.add_engine(
            Box::new(MockStt::ok("a", "hello")),
            EngineCapability {
                name: "a".into(),
                has_gpu: false,
                has_cloud: false,
            },
        );

        let chunk = make_chunk();
        let result = router.transcribe(&chunk).await.unwrap();
        assert_eq!(result, "hello");
        assert_eq!(router.active_engine_name(), Some("a".into()));
    }

    #[tokio::test]
    async fn test_router_fallback_to_next() {
        let router = SttRouter::new();
        router.add_engine(
            Box::new(MockStt::err("failing", "err")),
            EngineCapability {
                name: "failing".into(),
                has_gpu: false,
                has_cloud: false,
            },
        );
        router.add_engine(
            Box::new(MockStt::ok("working", "result")),
            EngineCapability {
                name: "working".into(),
                has_gpu: false,
                has_cloud: true,
            },
        );

        let chunk = make_chunk();
        let result = router.transcribe(&chunk).await.unwrap();
        assert_eq!(result, "result");
        assert_eq!(router.active_engine_name(), Some("working".into()));
    }

    #[tokio::test]
    async fn test_router_all_fail() {
        let router = SttRouter::new();
        for i in 0..3 {
            router.add_engine(
                Box::new(MockStt::err(&format!("e{}", i), "fail")),
                EngineCapability {
                    name: format!("e{}", i),
                    has_gpu: false,
                    has_cloud: false,
                },
            );
        }

        let chunk = make_chunk();
        let result = router.transcribe(&chunk).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_router_skips_unavailable() {
        let router = SttRouter::new();
        router.add_engine(
            Box::new(MockStt::ok("unavail", "nope").with_available(false)),
            EngineCapability {
                name: "unavail".into(),
                has_gpu: false,
                has_cloud: false,
            },
        );
        router.add_engine(
            Box::new(MockStt::ok("ok", "yes")),
            EngineCapability {
                name: "ok".into(),
                has_gpu: false,
                has_cloud: false,
            },
        );

        let chunk = make_chunk();
        let result = router.transcribe(&chunk).await.unwrap();
        assert_eq!(result, "yes");
    }

    #[tokio::test]
    async fn test_router_empty_response_triggers_fallback() {
        let router = SttRouter::new();
        router.add_engine(
            Box::new(MockStt::ok("empty", "")),
            EngineCapability {
                name: "empty".into(),
                has_gpu: false,
                has_cloud: false,
            },
        );
        router.add_engine(
            Box::new(MockStt::ok("real", "text")),
            EngineCapability {
                name: "real".into(),
                has_gpu: false,
                has_cloud: false,
            },
        );

        let chunk = make_chunk();
        let result = router.transcribe(&chunk).await.unwrap();
        assert_eq!(result, "text");
    }

    #[tokio::test]
    async fn test_router_set_engines() {
        let router = SttRouter::new();
        router.set_engines(vec![
            (
                Box::new(MockStt::ok("a", "a")),
                EngineCapability {
                    name: "a".into(),
                    has_gpu: false,
                    has_cloud: false,
                },
            ),
            (
                Box::new(MockStt::ok("b", "b")),
                EngineCapability {
                    name: "b".into(),
                    has_gpu: true,
                    has_cloud: false,
                },
            ),
        ]);

        assert_eq!(router.engine_count(), 2);
        assert_eq!(router.capabilities().len(), 2);
        assert!(!router.capabilities()[1].has_cloud);
        assert!(router.capabilities()[1].has_gpu);
    }

    #[tokio::test]
    async fn test_router_set_active_index() {
        let router = SttRouter::new();
        router.add_engine(
            Box::new(MockStt::ok("a", "a")),
            EngineCapability {
                name: "a".into(),
                has_gpu: false,
                has_cloud: false,
            },
        );
        router.add_engine(
            Box::new(MockStt::ok("b", "b")),
            EngineCapability {
                name: "b".into(),
                has_gpu: false,
                has_cloud: false,
            },
        );

        assert!(router.set_active_index(1));
        assert!(!router.set_active_index(5));
        assert_eq!(router.active_engine_name(), Some("b".into()));
    }

    #[tokio::test]
    async fn test_router_is_available() {
        let router = SttRouter::new();
        assert!(!router.is_available());
        router.add_engine(
            Box::new(MockStt::ok("a", "").with_available(false)),
            EngineCapability {
                name: "a".into(),
                has_gpu: false,
                has_cloud: false,
            },
        );
        assert!(!router.is_available());
        router.add_engine(
            Box::new(MockStt::ok("b", "")),
            EngineCapability {
                name: "b".into(),
                has_gpu: false,
                has_cloud: false,
            },
        );
        assert!(router.is_available());
    }

    #[tokio::test]
    async fn test_router_supported_languages_union() {
        let router = SttRouter::new();
        router.add_engine(
            Box::new(MockSttWithLangs::new("a", vec!["en", "es"])),
            EngineCapability {
                name: "a".into(),
                has_gpu: false,
                has_cloud: false,
            },
        );
        router.add_engine(
            Box::new(MockSttWithLangs::new("b", vec!["fr", "de"])),
            EngineCapability {
                name: "b".into(),
                has_gpu: false,
                has_cloud: false,
            },
        );

        let langs = router.supported_languages();
        assert!(langs.contains(&"en".into()));
        assert!(langs.contains(&"fr".into()));
    }

    struct MockSttWithLangs {
        name: String,
        langs: Vec<String>,
    }

    impl MockSttWithLangs {
        fn new(name: &str, langs: Vec<&str>) -> Self {
            Self {
                name: name.into(),
                langs: langs.into_iter().map(String::from).collect(),
            }
        }
    }

    #[async_trait]
    impl SttEngine for MockSttWithLangs {
        fn name(&self) -> &str {
            &self.name
        }

        async fn transcribe(
            &self,
            _audio: &AudioChunk,
        ) -> Result<String, voxy_voice_orchestrator::VoiceOrchestratorError> {
            Ok(String::new())
        }

        async fn transcribe_stream(
            &self,
            _stream: Box<dyn AudioStream>,
        ) -> Result<String, voxy_voice_orchestrator::VoiceOrchestratorError> {
            Ok(String::new())
        }

        fn supported_languages(&self) -> Vec<String> {
            self.langs.clone()
        }

        fn is_available(&self) -> bool {
            true
        }
    }
}
