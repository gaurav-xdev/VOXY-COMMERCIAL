//! High-level factory building the cloud voice system from environment configuration.

use std::sync::Arc;
use std::time::Duration;
use tracing::{info, warn};

use crate::provider::admin::{ProviderDescriptor, VoiceAdminApi};
use crate::provider::cache::TTSCache;
use crate::provider::quota::budget::RoutingMode;
use crate::provider::quota::rate_limiter::RateLimitConfig;
use crate::provider::quota::storage::InMemoryQuotaStorage;
use crate::provider::quota::ProviderQuotaManager;
use crate::provider::router::{STTRouter, TTSRouter};
use crate::provider::stt::{
    AssemblyAISTTProvider, AzureSTTProvider, DeepgramSTTProvider, ElevenLabsSTTProvider,
    GoogleSTTProvider, GroqSTTProvider, LocalSapiSTTProvider, MockSTTProvider,
};
use crate::provider::stt_service::CloudSTTService;
use crate::provider::traits::{STTProvider, TTSProvider, VoiceMode};
use crate::provider::tts::{
    AzureTTSProvider, CartesiaTTSProvider, ChatterboxTTSProvider, DeepgramTTSProvider,
    ElevenLabsTTSProvider, GoogleTTSProvider, LocalSapiTTSProvider, OpenAITTSProvider,
};
use crate::provider::tts_service::CloudTTSService;

pub struct VoiceSystem {
    pub stt_service: Arc<CloudSTTService>,
    pub tts_service: Arc<CloudTTSService>,
    pub quota_manager: Arc<ProviderQuotaManager>,
    pub stt_router: Arc<STTRouter>,
    pub tts_router: Arc<TTSRouter>,
    pub admin_api: Arc<VoiceAdminApi>,
}

impl VoiceSystem {
    pub fn build_from_env() -> Self {
        let mode = RoutingMode::from_env("VOXY_VOICE_ROUTING_MODE");
        let voice_mode = VoiceMode::from_env("VOICE_MODE");
        let default_margin = mode.default_safety_margin();

        info!(
            routing_mode = ?mode,
            voice_mode = ?voice_mode,
            safety_margin = default_margin,
            "Initializing VOXY Enterprise Voice Provider System with Rate Limiting, Quota Engine & Local Fallback"
        );

        let storage = Arc::new(InMemoryQuotaStorage::new());
        let quota_manager = Arc::new(ProviderQuotaManager::new(storage, mode));

        let mut stt_providers: Vec<Box<dyn STTProvider>> = Vec::new();
        let mut tts_providers: Vec<Box<dyn TTSProvider>> = Vec::new();
        let mut descriptors: Vec<ProviderDescriptor> = Vec::new();

        // ── 1. STT Providers Discovery & Quota Configuration ──
        if let Some(groq) = GroqSTTProvider::from_env() {
            info!("Registered STT Provider: Groq Whisper Large V3 Turbo");
            quota_manager.register_provider(
                groq.id(),
                RateLimitConfig::from_env_with_defaults(
                    "GROQ_STT", 60, 2000, 5, 50_000, 500_000, default_margin,
                ),
            );
            descriptors.push(ProviderDescriptor {
                id: groq.id().to_string(),
                name: groq.name().to_string(),
                provider_type: "STT",
                streaming: false,
                supported_languages: vec!["en".into(), "hi".into(), "hinglish".into()],
                estimated_cost_info: "$0.000043 / sec ($0.04/hr)".into(),
            });
            stt_providers.push(Box::new(groq));
        }

        if let Some(deepgram) = DeepgramSTTProvider::from_env() {
            info!("Registered STT Provider: Deepgram Nova-2");
            quota_manager.register_provider(
                deepgram.id(),
                RateLimitConfig::from_env_with_defaults(
                    "DEEPGRAM_STT", 100, 3000, 5, 50_000, 500_000, default_margin,
                ),
            );
            descriptors.push(ProviderDescriptor {
                id: deepgram.id().to_string(),
                name: deepgram.name().to_string(),
                provider_type: "STT",
                streaming: true,
                supported_languages: vec!["en".into(), "hi".into(), "hinglish".into()],
                estimated_cost_info: "$0.0043 / min".into(),
            });
            stt_providers.push(Box::new(deepgram));
        }

        if let Some(google) = GoogleSTTProvider::from_env() {
            info!("Registered STT Provider: Google Cloud Speech / Chirp");
            quota_manager.register_provider(
                google.id(),
                RateLimitConfig::from_env_with_defaults(
                    "GOOGLE_STT", 300, 5000, 10, 100_000, 1_000_000, default_margin,
                ),
            );
            descriptors.push(ProviderDescriptor {
                id: google.id().to_string(),
                name: google.name().to_string(),
                provider_type: "STT",
                streaming: false,
                supported_languages: vec!["en".into(), "hi".into()],
                estimated_cost_info: "$0.016 / min".into(),
            });
            stt_providers.push(Box::new(google));
        }

        if let Some(azure) = AzureSTTProvider::from_env() {
            info!("Registered STT Provider: Microsoft Azure Speech");
            quota_manager.register_provider(
                azure.id(),
                RateLimitConfig::from_env_with_defaults(
                    "AZURE_STT", 100, 3000, 8, 50_000, 500_000, default_margin,
                ),
            );
            descriptors.push(ProviderDescriptor {
                id: azure.id().to_string(),
                name: azure.name().to_string(),
                provider_type: "STT",
                streaming: false,
                supported_languages: vec!["en".into(), "hi".into()],
                estimated_cost_info: "$1.00 / hr".into(),
            });
            stt_providers.push(Box::new(azure));
        }

        if let Some(assembly) = AssemblyAISTTProvider::from_env() {
            info!("Registered STT Provider: AssemblyAI Speech-to-Text");
            quota_manager.register_provider(
                assembly.id(),
                RateLimitConfig::from_env_with_defaults(
                    "ASSEMBLYAI_STT", 60, 2000, 5, 50_000, 500_000, default_margin,
                ),
            );
            descriptors.push(ProviderDescriptor {
                id: assembly.id().to_string(),
                name: assembly.name().to_string(),
                provider_type: "STT",
                streaming: false,
                supported_languages: vec!["en".into(), "hi".into()],
                estimated_cost_info: "$0.00025 / sec".into(),
            });
            stt_providers.push(Box::new(assembly));
        }

        if let Some(elevenlabs) = ElevenLabsSTTProvider::from_env() {
            info!("Registered STT Provider: ElevenLabs Scribe STT");
            quota_manager.register_provider(
                elevenlabs.id(),
                RateLimitConfig::from_env_with_defaults(
                    "ELEVENLABS_STT", 50, 1000, 3, 30_000, 250_000, default_margin,
                ),
            );
            descriptors.push(ProviderDescriptor {
                id: elevenlabs.id().to_string(),
                name: elevenlabs.name().to_string(),
                provider_type: "STT",
                streaming: false,
                supported_languages: vec!["en".into(), "hi".into(), "hinglish".into()],
                estimated_cost_info: "$0.0048 / min".into(),
            });
            stt_providers.push(Box::new(elevenlabs));
        }

        // ── 2. TTS Providers Discovery & Quota Configuration ──
        if let Some(cartesia) = CartesiaTTSProvider::from_env() {
            info!("Registered TTS Provider: Cartesia Sonic TTS");
            quota_manager.register_provider(
                cartesia.id(),
                RateLimitConfig::from_env_with_defaults(
                    "CARTESIA", 60, 2000, 4, 30_000, 500_000, default_margin,
                ),
            );
            descriptors.push(ProviderDescriptor {
                id: cartesia.id().to_string(),
                name: cartesia.name().to_string(),
                provider_type: "TTS",
                streaming: true,
                supported_languages: vec!["en".into(), "hi".into(), "hinglish".into()],
                estimated_cost_info: "$0.075 / 1k chars".into(),
            });
            tts_providers.push(Box::new(cartesia));
        }

        if let Some(elevenlabs) = ElevenLabsTTSProvider::from_env() {
            info!("Registered TTS Provider: ElevenLabs Neural TTS");
            quota_manager.register_provider(
                elevenlabs.id(),
                RateLimitConfig::from_env_with_defaults(
                    "ELEVENLABS", 50, 1000, 3, 30_000, 250_000, default_margin,
                ),
            );
            descriptors.push(ProviderDescriptor {
                id: elevenlabs.id().to_string(),
                name: elevenlabs.name().to_string(),
                provider_type: "TTS",
                streaming: true,
                supported_languages: vec!["en".into(), "hi".into(), "hinglish".into()],
                estimated_cost_info: "$0.15 / 1k chars".into(),
            });
            tts_providers.push(Box::new(elevenlabs));
        }

        if let Some(azure) = AzureTTSProvider::from_env() {
            info!("Registered TTS Provider: Microsoft Azure Neural TTS");
            quota_manager.register_provider(
                azure.id(),
                RateLimitConfig::from_env_with_defaults(
                    "AZURE_TTS", 100, 3000, 8, 50_000, 1_000_000, default_margin,
                ),
            );
            descriptors.push(ProviderDescriptor {
                id: azure.id().to_string(),
                name: azure.name().to_string(),
                provider_type: "TTS",
                streaming: true,
                supported_languages: vec!["en".into(), "hi".into(), "hinglish".into()],
                estimated_cost_info: "$16.00 / 1M chars".into(),
            });
            tts_providers.push(Box::new(azure));
        }

        if let Some(google) = GoogleTTSProvider::from_env() {
            info!("Registered TTS Provider: Google Cloud Text-to-Speech");
            quota_manager.register_provider(
                google.id(),
                RateLimitConfig::from_env_with_defaults(
                    "GOOGLE_TTS", 300, 5000, 10, 100_000, 2_000_000, default_margin,
                ),
            );
            descriptors.push(ProviderDescriptor {
                id: google.id().to_string(),
                name: google.name().to_string(),
                provider_type: "TTS",
                streaming: false,
                supported_languages: vec!["en".into(), "hi".into(), "hinglish".into()],
                estimated_cost_info: "$16.00 / 1M chars".into(),
            });
            tts_providers.push(Box::new(google));
        }

        if let Some(openai) = OpenAITTSProvider::from_env() {
            info!("Registered TTS Provider: OpenAI Text-to-Speech (tts-1)");
            quota_manager.register_provider(
                openai.id(),
                RateLimitConfig::from_env_with_defaults(
                    "OPENAI_TTS", 50, 1000, 3, 30_000, 250_000, default_margin,
                ),
            );
            descriptors.push(ProviderDescriptor {
                id: openai.id().to_string(),
                name: openai.name().to_string(),
                provider_type: "TTS",
                streaming: true,
                supported_languages: vec!["en".into(), "hi".into(), "hinglish".into()],
                estimated_cost_info: "$15.00 / 1M chars".into(),
            });
            tts_providers.push(Box::new(openai));
        }

        if let Some(deepgram) = DeepgramTTSProvider::from_env() {
            info!("Registered TTS Provider: Deepgram Aura TTS");
            quota_manager.register_provider(
                deepgram.id(),
                RateLimitConfig::from_env_with_defaults(
                    "DEEPGRAM_TTS", 100, 2000, 5, 50_000, 500_000, default_margin,
                ),
            );
            descriptors.push(ProviderDescriptor {
                id: deepgram.id().to_string(),
                name: deepgram.name().to_string(),
                provider_type: "TTS",
                streaming: true,
                supported_languages: vec!["en".into()],
                estimated_cost_info: "$15.00 / 1M chars".into(),
            });
            tts_providers.push(Box::new(deepgram));
        }

        if let Some(chatterbox) = ChatterboxTTSProvider::from_env() {
            info!("Registered TTS Provider: Chatterbox Turbo Hinglish");
            quota_manager.register_provider(
                chatterbox.id(),
                RateLimitConfig::from_env_with_defaults(
                    "CHATTERBOX", 60, 1000, 2, 20_000, 200_000, default_margin,
                ),
            );
            descriptors.push(ProviderDescriptor {
                id: chatterbox.id().to_string(),
                name: chatterbox.name().to_string(),
                provider_type: "TTS",
                streaming: false,
                supported_languages: vec!["en".into(), "hi".into(), "hinglish".into()],
                estimated_cost_info: "Self-hosted / Cloud".into(),
            });
            tts_providers.push(Box::new(chatterbox));
        }

        if stt_providers.is_empty() {
            warn!("No cloud STT API keys configured. Using Mock STT provider for graceful operation.");
            let mock_stt = MockSTTProvider::new("mock-stt", "Mock STT Provider (Fallback)");
            stt_providers.push(Box::new(mock_stt));
        }

        // Emergency local fallback STT (Windows SAPI)
        let emergency_stt_fallback = Box::new(LocalSapiSTTProvider::new());
        quota_manager.register_provider(
            emergency_stt_fallback.id(),
            RateLimitConfig::from_env_with_defaults(
                "LOCAL_SAPI_STT", 1000, 100_000, 1, 1_000_000, 10_000_000, 0.0,
            ),
        );
        descriptors.push(ProviderDescriptor {
            id: emergency_stt_fallback.id().to_string(),
            name: emergency_stt_fallback.name().to_string(),
            provider_type: "STT (Offline Fallback)",
            streaming: false,
            supported_languages: vec!["en".into()],
            estimated_cost_info: "$0.00 (Local Offline)".into(),
        });

        // Emergency local fallback TTS (Windows SAPI)
        let emergency_fallback = Box::new(LocalSapiTTSProvider::new());
        quota_manager.register_provider(
            emergency_fallback.id(),
            RateLimitConfig::from_env_with_defaults(
                "LOCAL_SAPI", 1000, 100_000, 1, 1_000_000, 10_000_000, 0.0,
            ),
        );
        descriptors.push(ProviderDescriptor {
            id: emergency_fallback.id().to_string(),
            name: emergency_fallback.name().to_string(),
            provider_type: "TTS (Offline Fallback)",
            streaming: false,
            supported_languages: vec!["en".into()],
            estimated_cost_info: "$0.00 (Local Offline)".into(),
        });

        let stt_router = Arc::new(STTRouter::with_fallback(
            stt_providers,
            Some(emergency_stt_fallback),
            Arc::clone(&quota_manager),
            voice_mode,
        ));

        let tts_cache = Arc::new(TTSCache::default());
        let tts_router = Arc::new(TTSRouter::with_cache_and_mode(
            tts_providers,
            Some(emergency_fallback),
            Arc::clone(&quota_manager),
            tts_cache,
            Duration::from_millis(1500),
            voice_mode,
        ));

        let stt_service = Arc::new(CloudSTTService::new(Arc::clone(&stt_router)));
        let tts_service = Arc::new(CloudTTSService::new(Arc::clone(&tts_router)));

        let admin_api = Arc::new(VoiceAdminApi::new(
            Arc::clone(&quota_manager),
            Some(Arc::clone(&stt_router)),
            Some(Arc::clone(&tts_router)),
            descriptors,
        ));

        Self {
            stt_service,
            tts_service,
            quota_manager,
            stt_router,
            tts_router,
            admin_api,
        }
    }
}
