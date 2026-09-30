use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum VoiceLanguage {
    English,
    Hindi,
    Hinglish,
    AutoDetect,
    Auto,
    Other(String),
    Custom(String),
}

impl VoiceLanguage {
    pub fn as_code(&self) -> &str {
        match self {
            Self::English => "en",
            Self::Hindi => "hi",
            Self::Hinglish => "hi-Latn",
            Self::AutoDetect | Self::Auto => "auto",
            Self::Other(c) | Self::Custom(c) => c.as_str(),
        }
    }
}

impl fmt::Display for VoiceLanguage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_code())
    }
}

/// Voice pipeline operation mode controlling cloud vs offline execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum VoiceMode {
    /// Cloud first, automatic local fallback on failure, timeout, or quota exhaustion.
    #[default]
    Auto,
    /// Cloud providers only; errors out if cloud is unavailable.
    Cloud,
    /// Strict local offline mode; zero outbound network calls, zero cloud dependencies.
    Local,
}

impl VoiceMode {
    pub fn from_env(var_name: &str) -> Self {
        match std::env::var(var_name).ok().as_deref() {
            Some(v) if v.eq_ignore_ascii_case("local") || v.eq_ignore_ascii_case("offline") => {
                Self::Local
            }
            Some(v) if v.eq_ignore_ascii_case("cloud") => Self::Cloud,
            _ => Self::Auto,
        }
    }

    pub fn is_local_only(&self) -> bool {
        matches!(self, Self::Local)
    }

    pub fn is_cloud_only(&self) -> bool {
        matches!(self, Self::Cloud)
    }

    pub fn allows_cloud(&self) -> bool {
        !matches!(self, Self::Local)
    }

    pub fn allows_local(&self) -> bool {
        !matches!(self, Self::Cloud)
    }
}

impl fmt::Display for VoiceMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Auto => write!(f, "auto"),
            Self::Cloud => write!(f, "cloud"),
            Self::Local => write!(f, "local"),
        }
    }
}

/// Normalized raw audio buffer used for STT input or TTS output.
#[derive(Debug, Clone)]
pub struct AudioData {
    pub pcm_bytes: Vec<u8>,
    pub sample_rate: u32,
    pub channels: u16,
    pub duration_secs: f64,
}

impl AudioData {
    pub fn new(pcm_bytes: Vec<u8>, sample_rate: u32, channels: u16) -> Self {
        let bytes_per_sample = 2usize; // 16-bit PCM
        let total_samples = pcm_bytes.len() / (bytes_per_sample * channels.max(1) as usize);
        let duration_secs = if sample_rate > 0 {
            total_samples as f64 / sample_rate as f64
        } else {
            0.0
        };
        Self {
            pcm_bytes,
            sample_rate,
            channels,
            duration_secs,
        }
    }

    pub fn from_pcm(samples: Vec<f32>, sample_rate: u32, channels: u32) -> Self {
        let mut pcm_bytes = Vec::with_capacity(samples.len() * 2);
        for s in samples {
            let sample_i16 = (s.clamp(-1.0, 1.0) * 32767.0) as i16;
            pcm_bytes.extend_from_slice(&sample_i16.to_le_bytes());
        }
        Self::new(pcm_bytes, sample_rate, channels as u16)
    }

    pub fn to_samples_f32(&self) -> Vec<f32> {
        self.pcm_bytes
            .chunks_exact(2)
            .map(|c| i16::from_le_bytes([c[0], c[1]]) as f32 / 32768.0)
            .collect()
    }

    pub fn duration_secs(&self) -> f64 {
        self.duration_secs
    }

    pub fn to_wav(&self) -> Vec<u8> {
        let byte_rate = self.sample_rate * self.channels as u32 * 2;
        let block_align = self.channels * 2;
        let data_size = self.pcm_bytes.len() as u32;
        let header_size = 44u32;

        let mut wav = Vec::with_capacity((header_size + data_size) as usize);
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&(header_size + data_size - 8).to_le_bytes());
        wav.extend_from_slice(b"WAVE");
        wav.extend_from_slice(b"fmt ");
        wav.extend_from_slice(&16u32.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes()); // PCM
        wav.extend_from_slice(&self.channels.to_le_bytes());
        wav.extend_from_slice(&self.sample_rate.to_le_bytes());
        wav.extend_from_slice(&byte_rate.to_le_bytes());
        wav.extend_from_slice(&block_align.to_le_bytes());
        wav.extend_from_slice(&16u16.to_le_bytes()); // 16-bit
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&data_size.to_le_bytes());
        wav.extend_from_slice(&self.pcm_bytes);
        wav
    }
}

/// Normalized transcription result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transcript {
    pub text: String,
    pub confidence: Option<f32>,
    pub language: Option<VoiceLanguage>,
    pub latency_ms: u64,
    pub is_final: bool,
    pub provider: String,
    #[serde(default)]
    pub duration_secs: f64,
    #[serde(default)]
    pub words: Option<Vec<String>>,
}

/// Quota estimation certainty level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum QuotaConfidence {
    DocumentedLimit,
    EstimatedRemaining,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuotaStatus {
    pub confidence: QuotaConfidence,
    pub rpm_limit: u32,
    pub rpd_limit: u32,
    pub requests_today: u32,
    pub estimated_remaining_today: Option<u32>,
    pub concurrency_limit: u32,
    pub active_requests: u32,
    pub in_cooldown_until: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(default)]
    pub health: HealthStatus,
    #[serde(default)]
    pub is_cooldown: bool,
    #[serde(default)]
    pub latency_p50_ms: f64,
    #[serde(default)]
    pub remaining_pct: f64,
}

impl Default for QuotaStatus {
    fn default() -> Self {
        Self {
            confidence: QuotaConfidence::Unknown,
            rpm_limit: 100,
            rpd_limit: 1000,
            requests_today: 0,
            estimated_remaining_today: Some(1000),
            concurrency_limit: 5,
            active_requests: 0,
            in_cooldown_until: None,
            health: HealthStatus::Healthy,
            is_cooldown: false,
            latency_p50_ms: 120.0,
            remaining_pct: 1.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EstimatedCost {
    pub amount_usd: f64,
    pub currency: String,
    #[serde(default)]
    pub estimated_cost_usd: f64,
    #[serde(default)]
    pub billable_units: f64,
    #[serde(default)]
    pub unit_type: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum HealthStatus {
    #[default]
    Healthy,
    Degraded(String),
    CoolingDown {
        retry_after_secs: u64,
    },
    InvalidCredentials(String),
    Unavailable(String),
    Exhausted,
    Cooldown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct STTCapabilities {
    pub streaming: bool,
    pub word_timestamps: bool,
    pub supported_languages: Vec<VoiceLanguage>,
    pub multilingual: bool,
    pub custom_vocab: bool,
    pub max_audio_duration_secs: u32,
    pub typical_latency_ms: u32,
    pub cost_per_second_usd: f64,
}

impl STTCapabilities {
    pub fn supports_language(&self, lang: &VoiceLanguage) -> bool {
        self.supported_languages.contains(lang) || self.multilingual
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TTSCapabilities {
    pub streaming: bool,
    pub supported_languages: Vec<VoiceLanguage>,
    pub emotional_styles: bool,
    pub speed_control: bool,
    pub pitch_control: bool,
    pub typical_first_byte_ms: u32,
    pub cost_per_char_usd: f64,
}

impl TTSCapabilities {
    pub fn supports_language(&self, lang: &VoiceLanguage) -> bool {
        self.supported_languages.contains(lang)
    }
}

pub fn redact_sensitive_str(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let words = input.split_inclusive(|c: char| {
        c.is_whitespace() || c == '"' || c == '\'' || c == ',' || c == ';' || c == '}'
    });

    let mut prev_was_bearer = false;
    for word in words {
        let (token, delimiter) = match word.find(|c: char| {
            c.is_whitespace() || c == '"' || c == '\'' || c == ',' || c == ';' || c == '}'
        }) {
            Some(idx) => (&word[..idx], &word[idx..]),
            None => (word, ""),
        };

        if prev_was_bearer && !token.is_empty() {
            out.push_str("***[REDACTED]");
            out.push_str(delimiter);
            prev_was_bearer = false;
            continue;
        }

        if token.eq_ignore_ascii_case("bearer") {
            prev_was_bearer = true;
            out.push_str(token);
            out.push_str(delimiter);
            continue;
        }

        if token.starts_with("sk-") || token.starts_with("gsk_") || token.starts_with("AIza") {
            out.push_str(&token[..4.min(token.len())]);
            out.push_str("***[REDACTED]");
            out.push_str(delimiter);
            prev_was_bearer = false;
            continue;
        }

        if let Some(pos) = token.find("key=") {
            let prefix_part = &token[..pos + 4];
            out.push_str(prefix_part);
            out.push_str("***[REDACTED]");
            out.push_str(delimiter);
            prev_was_bearer = false;
            continue;
        }

        out.push_str(token);
        out.push_str(delimiter);
        prev_was_bearer = false;
    }
    out
}

#[derive(Debug, Clone)]
pub enum ProviderError {
    RateLimited {
        retry_after: Option<Duration>,
        reason: String,
    },
    AuthError(String),
    InvalidCredentials(String),
    ServerError {
        status: u16,
        message: String,
    },
    Timeout(Duration),
    NetworkError(String),
    Network(String),
    TranscriptionFailed(String),
    SynthesisFailed(String),
    ConfigError(String),
    Unsupported(String),
    AudioConversion(String),
    AllProvidersFailed(String),
}

impl std::fmt::Display for ProviderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProviderError::RateLimited {
                retry_after: _,
                reason,
            } => {
                write!(f, "Rate limited (429): {}", redact_sensitive_str(reason))
            }
            ProviderError::AuthError(s) => {
                write!(f, "Authentication failed: {}", redact_sensitive_str(s))
            }
            ProviderError::InvalidCredentials(s) => {
                write!(f, "Invalid credentials: {}", redact_sensitive_str(s))
            }
            ProviderError::ServerError { status, message } => {
                write!(
                    f,
                    "Server error ({}): {}",
                    status,
                    redact_sensitive_str(message)
                )
            }
            ProviderError::Timeout(d) => write!(f, "Request timed out after {:?}", d),
            ProviderError::NetworkError(s) => {
                write!(f, "Network error: {}", redact_sensitive_str(s))
            }
            ProviderError::Network(s) => {
                write!(f, "Network error: {}", redact_sensitive_str(s))
            }
            ProviderError::TranscriptionFailed(s) => {
                write!(f, "Transcription failed: {}", redact_sensitive_str(s))
            }
            ProviderError::SynthesisFailed(s) => {
                write!(f, "Synthesis failed: {}", redact_sensitive_str(s))
            }
            ProviderError::ConfigError(s) => {
                write!(f, "Config error: {}", redact_sensitive_str(s))
            }
            ProviderError::Unsupported(s) => {
                write!(f, "Unsupported capability: {}", redact_sensitive_str(s))
            }
            ProviderError::AudioConversion(s) => {
                write!(f, "Audio conversion error: {}", redact_sensitive_str(s))
            }
            ProviderError::AllProvidersFailed(s) => {
                write!(
                    f,
                    "All voice providers in fallback chain failed: {}",
                    redact_sensitive_str(s)
                )
            }
        }
    }
}

impl std::error::Error for ProviderError {}

impl ProviderError {
    pub fn is_transient(&self) -> bool {
        match self {
            ProviderError::ServerError { status, .. } => *status >= 500 && *status < 600,
            ProviderError::Timeout(_) => true,
            ProviderError::NetworkError(_) | ProviderError::Network(_) => true,
            _ => false,
        }
    }
}

pub type TTSStream =
    std::pin::Pin<Box<dyn futures::Stream<Item = Result<AudioData, ProviderError>> + Send>>;

/// Unified provider-agnostic STT interface.
#[async_trait]
pub trait STTProvider: Send + Sync {
    fn id(&self) -> &'static str;
    fn name(&self) -> &'static str;
    fn capabilities(&self) -> &STTCapabilities;
    async fn transcribe(
        &self,
        audio: AudioData,
        language_hint: Option<VoiceLanguage>,
    ) -> Result<Transcript, ProviderError>;
    fn estimate_cost(&self, audio_duration_secs: f64) -> EstimatedCost;
}

/// Unified provider-agnostic TTS interface.
#[async_trait]
pub trait TTSProvider: Send + Sync {
    fn id(&self) -> &'static str;
    fn name(&self) -> &'static str;
    fn capabilities(&self) -> &TTSCapabilities;
    async fn synthesize(
        &self,
        text: &str,
        language: Option<VoiceLanguage>,
    ) -> Result<AudioData, ProviderError>;
    async fn synthesize_stream(
        &self,
        text: &str,
        language: Option<VoiceLanguage>,
    ) -> Result<TTSStream, ProviderError> {
        let audio = self.synthesize(text, language).await?;
        let stream = futures::stream::iter(vec![Ok(audio)]);
        Ok(Box::pin(stream))
    }
    fn estimate_cost(&self, character_count: usize) -> EstimatedCost;
}
