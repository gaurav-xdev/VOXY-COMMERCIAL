//! Smart STT routing engine with multi-factor scoring, quota budgeting, and automatic failover.

use std::sync::Arc;
use std::time::Instant;
use tracing::{debug, error, info, warn};

use crate::provider::quota::ProviderQuotaManager;
use crate::provider::traits::{
    AudioData, HealthStatus, ProviderError, STTProvider, Transcript, VoiceLanguage, VoiceMode,
};

pub struct STTRouter {
    providers: Vec<Box<dyn STTProvider>>,
    emergency_fallback: Option<Box<dyn STTProvider>>,
    quota_manager: Arc<ProviderQuotaManager>,
    mode: VoiceMode,
}

impl STTRouter {
    pub fn new(
        providers: Vec<Box<dyn STTProvider>>,
        quota_manager: Arc<ProviderQuotaManager>,
    ) -> Self {
        Self::with_fallback(providers, None, quota_manager, VoiceMode::Auto)
    }

    pub fn with_fallback(
        providers: Vec<Box<dyn STTProvider>>,
        emergency_fallback: Option<Box<dyn STTProvider>>,
        quota_manager: Arc<ProviderQuotaManager>,
        mode: VoiceMode,
    ) -> Self {
        Self {
            providers,
            emergency_fallback,
            quota_manager,
            mode,
        }
    }

    pub fn mode(&self) -> VoiceMode {
        self.mode
    }

    pub fn set_mode(&mut self, mode: VoiceMode) {
        self.mode = mode;
    }

    /// Calculate real-time candidate score for an STT provider.
    /// Range: 0.0 to 100.0 (higher is better).
    fn calculate_score(
        &self,
        provider: &dyn STTProvider,
        language_hint: Option<VoiceLanguage>,
        audio_duration_secs: f64,
    ) -> f64 {
        let id = provider.id();
        let quota = self.quota_manager.get_status(id);

        if self.quota_manager.is_in_cooldown(id) {
            return 0.0;
        }

        // 1. Health Score (weight 0.35)
        let health_pts = match quota.health {
            HealthStatus::Healthy => 100.0,
            HealthStatus::Degraded(_) => 50.0,
            HealthStatus::Exhausted | HealthStatus::Cooldown => 0.0,
            HealthStatus::CoolingDown { .. }
            | HealthStatus::InvalidCredentials(_)
            | HealthStatus::Unavailable(_) => 0.0,
        };

        // 2. Latency Score (weight 0.25)
        let latency_ms = if quota.latency_p50_ms > 0.0 {
            quota.latency_p50_ms
        } else {
            provider.capabilities().typical_latency_ms as f64
        };
        let latency_pts = (100.0 - (latency_ms / 15.0)).clamp(5.0, 100.0);

        // 3. Quota Headroom Score (weight 0.25)
        let headroom_pts = quota.remaining_pct.clamp(0.0, 1.0) * 100.0;

        // 4. Cost Score (weight 0.15)
        let est = provider.estimate_cost(audio_duration_secs);
        let cost_pts = (1.0 / (est.amount_usd + 0.0001) * 0.01).clamp(0.0, 100.0);

        let mut total =
            (health_pts * 0.35) + (latency_pts * 0.25) + (headroom_pts * 0.25) + (cost_pts * 0.15);

        // Language matching bonus
        if let Some(ref lang) = language_hint {
            if provider.capabilities().supports_language(lang) {
                total += 15.0;
            }
        }

        total
    }

    /// Transcribe audio using the highest scored provider, with automatic failover and local offline fallback.
    pub async fn transcribe(
        &self,
        audio: AudioData,
        language_hint: Option<VoiceLanguage>,
    ) -> Result<Transcript, ProviderError> {
        // ── 1. Strict Local Offline Mode ─────────────────────────────────
        if self.mode.is_local_only() {
            if let Some(ref fallback) = self.emergency_fallback {
                let pid = fallback.id();
                debug!(
                    fallback = pid,
                    "Local-only voice mode: using local offline STT directly"
                );
                let start = Instant::now();
                let res = fallback.transcribe(audio, language_hint).await;
                if res.is_ok() {
                    info!(
                        fallback = pid,
                        elapsed_ms = start.elapsed().as_millis(),
                        "Local offline STT transcription succeeded"
                    );
                }
                return res;
            } else {
                return Err(ProviderError::ConfigError(
                    "Local voice mode active but no local offline STT provider registered".into(),
                ));
            }
        }

        // ── 2. Cloud Providers Evaluation ────────────────────────────────
        let mut last_error = None;

        if !self.providers.is_empty() {
            let audio_dur = audio.duration_secs();

            // Rank candidates
            let mut candidates: Vec<(usize, f64)> = self
                .providers
                .iter()
                .enumerate()
                .map(|(idx, p)| {
                    let score = self.calculate_score(p.as_ref(), language_hint.clone(), audio_dur);
                    (idx, score)
                })
                .collect();

            candidates.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

            for (idx, score) in candidates {
                if score <= 0.0 {
                    continue;
                }

                let provider = &self.providers[idx];
                let pid = provider.id();

                if let Err(violation) = self.quota_manager.check_preflight(pid, 1, audio_dur) {
                    warn!(provider = pid, reason = ?violation, "Preflight quota rejected provider");
                    continue;
                }

                debug!(
                    provider = pid,
                    score = score,
                    "Attempting STT transcription"
                );

                let start = Instant::now();
                match provider
                    .transcribe(audio.clone(), language_hint.clone())
                    .await
                {
                    Ok(transcript) => {
                        let elapsed = start.elapsed();
                        let est_cost = provider.estimate_cost(audio_dur);

                        self.quota_manager.record_success(
                            pid,
                            elapsed,
                            1,
                            audio_dur,
                            est_cost.amount_usd,
                        );

                        info!(
                            provider = pid,
                            elapsed_ms = elapsed.as_millis(),
                            cost_usd = est_cost.amount_usd,
                            "STT transcription successful"
                        );

                        return Ok(transcript);
                    }
                    Err(err) => {
                        let elapsed = start.elapsed();
                        warn!(
                            provider = pid,
                            error = %err,
                            elapsed_ms = elapsed.as_millis(),
                            "STT provider failed, attempting failover"
                        );

                        match &err {
                            ProviderError::RateLimited { retry_after, .. } => {
                                self.quota_manager.record_rate_limit(pid, *retry_after);
                            }
                            _ => {
                                self.quota_manager.record_failure(pid);
                            }
                        }

                        last_error = Some(err);
                    }
                }
            }
        }

        // ── 3. Emergency Local Fallback (Auto Mode) ───────────────────────
        if self.mode.allows_local() {
            if let Some(ref fallback) = self.emergency_fallback {
                let pid = fallback.id();
                warn!(
                    fallback = pid,
                    "Cloud STT exhausted or failed: using emergency local offline fallback"
                );
                let start = Instant::now();
                match fallback.transcribe(audio, language_hint).await {
                    Ok(transcript) => {
                        info!(
                            fallback = pid,
                            elapsed_ms = start.elapsed().as_millis(),
                            "Emergency local offline STT succeeded"
                        );
                        return Ok(transcript);
                    }
                    Err(err) => {
                        error!(fallback = pid, error = %err, "Emergency local offline STT fallback failed");
                        last_error = Some(err);
                    }
                }
            }
        }

        error!("All available STT providers failed or exhausted");
        Err(last_error.unwrap_or_else(|| {
            ProviderError::AllProvidersFailed(
                "All STT providers are exhausted, in cooldown, or failed".into(),
            )
        }))
    }
}
