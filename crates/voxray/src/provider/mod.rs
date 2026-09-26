//! VOXY Cloud-First Enterprise STT/TTS Provider Architecture.
//!
//! Features:
//! - 6 STT Adapters: Groq Whisper, Google Speech/Chirp, Azure Speech, AssemblyAI, ElevenLabs Scribe, Deepgram Nova-2.
//! - 6 TTS Adapters: ElevenLabs, Azure Neural, Google TTS, OpenAI TTS, Deepgram Aura, Chatterbox Turbo Hinglish.
//! - Windows SAPI Emergency Offline Local Fallback (0 VRAM, 0 GPU).
//! - Multi-tier Quota & Rate Limit Management (RPM, RPH, RPD, Characters, Audio Seconds, Cooldowns).
//! - Dynamic Multi-factor Scoring with failover and cost optimization.
//! - Real-time sentence-level streaming.
//! - Admin & Debug HTTP API (/voice/providers, /voice/quota, /voice/health, /voice/test/...).

pub mod admin;
pub mod cache;
pub mod factory;
pub mod observability;
pub mod quota;
pub mod router;
pub mod stt;
pub mod stt_service;
pub mod traits;
pub mod tts;
pub mod tts_service;

pub use cache::TTSCache;
pub use factory::VoiceSystem;
pub use quota::ProviderQuotaManager;
pub use router::{STTRouter, TTSRouter};
pub use stt_service::CloudSTTService;
pub use traits::{AudioData, EstimatedCost, HealthStatus, ProviderError, STTProvider, TTSProvider, VoiceLanguage};
pub use tts_service::CloudTTSService;

#[cfg(test)]
mod tests;
