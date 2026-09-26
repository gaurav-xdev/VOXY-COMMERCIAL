//! Observability, structured logging, and metrics telemetry for voice providers.

use serde::Serialize;
use std::time::Duration;
use tracing::{error, info, warn};

use crate::provider::traits::{HealthStatus, ProviderError};

#[derive(Debug, Clone, Serialize)]
pub struct VoiceMetricEvent {
    pub timestamp_epoch_ms: u64,
    pub event_type: &'static str,
    pub provider_id: String,
    pub provider_type: &'static str, // "STT" or "TTS"
    pub duration_ms: u64,
    pub units_consumed: f64,
    pub unit_type: &'static str,
    pub estimated_cost_usd: f64,
    pub success: bool,
    pub error_kind: Option<String>,
}

pub struct VoiceLogger;

impl VoiceLogger {
    pub fn log_stt_success(
        provider: &str,
        latency: Duration,
        audio_duration_secs: f64,
        cost_usd: f64,
    ) {
        info!(
            target: "voxy::voice::stt",
            provider = provider,
            latency_ms = latency.as_millis(),
            audio_dur_secs = audio_duration_secs,
            cost_usd = cost_usd,
            "STT Transcription Succeeded"
        );
    }

    pub fn log_stt_failure(provider: &str, error: &ProviderError, latency: Duration) {
        warn!(
            target: "voxy::voice::stt",
            provider = provider,
            error = %error,
            latency_ms = latency.as_millis(),
            "STT Transcription Failed"
        );
    }

    pub fn log_tts_success(
        provider: &str,
        latency: Duration,
        characters: usize,
        cost_usd: f64,
    ) {
        info!(
            target: "voxy::voice::tts",
            provider = provider,
            latency_ms = latency.as_millis(),
            characters = characters,
            cost_usd = cost_usd,
            "TTS Synthesis Succeeded"
        );
    }

    pub fn log_tts_failure(provider: &str, error: &ProviderError, latency: Duration) {
        warn!(
            target: "voxy::voice::tts",
            provider = provider,
            error = %error,
            latency_ms = latency.as_millis(),
            "TTS Synthesis Failed"
        );
    }

    pub fn log_rate_limit(provider: &str, retry_after: Option<Duration>) {
        warn!(
            target: "voxy::voice::quota",
            provider = provider,
            retry_after_secs = retry_after.map(|d| d.as_secs()).unwrap_or(0),
            "Provider hit 429 rate limit, entering cooldown"
        );
    }

    pub fn log_health_transition(provider: &str, from: HealthStatus, to: HealthStatus) {
        if to == HealthStatus::Exhausted || to == HealthStatus::Cooldown {
            error!(
                target: "voxy::voice::health",
                provider = provider,
                from = ?from,
                to = ?to,
                "Provider health state degraded"
            );
        } else {
            info!(
                target: "voxy::voice::health",
                provider = provider,
                from = ?from,
                to = ?to,
                "Provider health state updated"
            );
        }
    }
}
