//! Smart TTS routing engine with multi-factor scoring, quota budgeting, streaming,
//! queueing, duplicate prevention caching, and emergency local fallback.

use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::{debug, error, info, warn};

use crate::provider::cache::TTSCache;
use crate::provider::quota::ProviderQuotaManager;
use crate::provider::traits::{
    AudioData, HealthStatus, ProviderError, TTSProvider, TTSStream, VoiceLanguage, VoiceMode,
};

pub struct TTSRouter {
    providers: Vec<Box<dyn TTSProvider>>,
    emergency_fallback: Option<Box<dyn TTSProvider>>,
    quota_manager: Arc<ProviderQuotaManager>,
    cache: Arc<TTSCache>,
    queue_timeout: Duration,
    mode: VoiceMode,
}

impl TTSRouter {
    pub fn new(
        providers: Vec<Box<dyn TTSProvider>>,
        emergency_fallback: Option<Box<dyn TTSProvider>>,
        quota_manager: Arc<ProviderQuotaManager>,
    ) -> Self {
        Self::with_cache_and_mode(
            providers,
            emergency_fallback,
            quota_manager,
            Arc::new(TTSCache::default()),
            Duration::from_millis(1500),
            VoiceMode::Auto,
        )
    }

    pub fn with_cache(
        providers: Vec<Box<dyn TTSProvider>>,
        emergency_fallback: Option<Box<dyn TTSProvider>>,
        quota_manager: Arc<ProviderQuotaManager>,
        cache: Arc<TTSCache>,
        queue_timeout: Duration,
    ) -> Self {
        Self::with_cache_and_mode(
            providers,
            emergency_fallback,
            quota_manager,
            cache,
            queue_timeout,
            VoiceMode::Auto,
        )
    }

    pub fn with_cache_and_mode(
        providers: Vec<Box<dyn TTSProvider>>,
        emergency_fallback: Option<Box<dyn TTSProvider>>,
        quota_manager: Arc<ProviderQuotaManager>,
        cache: Arc<TTSCache>,
        queue_timeout: Duration,
        mode: VoiceMode,
    ) -> Self {
        Self {
            providers,
            emergency_fallback,
            quota_manager,
            cache,
            queue_timeout,
            mode,
        }
    }

    pub fn mode(&self) -> VoiceMode {
        self.mode
    }

    pub fn set_mode(&mut self, mode: VoiceMode) {
        self.mode = mode;
    }

    pub fn cache(&self) -> &Arc<TTSCache> {
        &self.cache
    }

    /// Calculate real-time candidate score for a TTS provider.
    /// Range: 0.0 to 100.0 (higher is better).
    fn calculate_score(
        &self,
        provider: &dyn TTSProvider,
        language: Option<VoiceLanguage>,
        char_count: usize,
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
            provider.capabilities().typical_first_byte_ms as f64
        };
        let latency_pts = (100.0 - (latency_ms / 15.0)).clamp(5.0, 100.0);

        // 3. Quota Headroom Score (weight 0.25)
        let headroom_pts = quota.remaining_pct.clamp(0.0, 1.0) * 100.0;

        // 4. Cost Score (weight 0.15)
        let est = provider.estimate_cost(char_count);
        let cost_pts = (1.0 / (est.amount_usd + 0.0001) * 0.005).clamp(0.0, 100.0);

        let mut total =
            (health_pts * 0.35) + (latency_pts * 0.25) + (headroom_pts * 0.25) + (cost_pts * 0.15);

        // Language matching bonus (+20 pts)
        if let Some(ref lang) = language {
            if provider.capabilities().supports_language(lang) {
                total += 20.0;
            }
        }

        // Penalty for recent failure rate
        let failure_rate = self.quota_manager.recent_failure_rate(id);
        total = (total - (failure_rate * 50.0)).max(0.0);

        total
    }

    /// Synthesize speech using the highest ranked provider, with auto failover, duplicate caching,
    /// dynamic queueing, and local emergency fallback.
    pub async fn synthesize(
        &self,
        text: &str,
        language: Option<VoiceLanguage>,
    ) -> Result<AudioData, ProviderError> {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return Ok(AudioData::new(Vec::new(), 16000, 1));
        }

        // 1. Duplicate-request check: return cached audio if available
        if let Some(cached) = self.cache.get(trimmed, language.as_ref()) {
            info!(
                text_len = trimmed.len(),
                "Duplicate TTS request detected: serving audio directly from cache"
            );
            return Ok(cached);
        }

        // ── Strict Local Offline Mode ────────────────────────────────────
        if self.mode.is_local_only() {
            if let Some(ref fallback) = self.emergency_fallback {
                let pid = fallback.id();
                debug!(
                    fallback = pid,
                    "Local-only voice mode: using local offline TTS directly"
                );
                let start = Instant::now();
                match fallback.synthesize(trimmed, language).await {
                    Ok(audio) => {
                        info!(
                            fallback = pid,
                            elapsed_ms = start.elapsed().as_millis(),
                            "Local offline TTS succeeded"
                        );
                        self.cache.insert(trimmed, None, audio.clone());
                        return Ok(audio);
                    }
                    Err(e) => {
                        error!(fallback = pid, error = %e, "Local offline TTS failed");
                        return Err(e);
                    }
                }
            } else {
                return Err(ProviderError::ConfigError(
                    "Local voice mode active but no local offline TTS provider registered".into(),
                ));
            }
        }

        let char_count = trimmed.chars().count();
        let queue_deadline = Instant::now() + self.queue_timeout;

        loop {
            // Rank candidates
            let mut candidates: Vec<(usize, f64)> = self
                .providers
                .iter()
                .enumerate()
                .map(|(idx, p)| {
                    let score = self.calculate_score(p.as_ref(), language.clone(), char_count);
                    (idx, score)
                })
                .collect();

            candidates.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

            let mut attempted_any = false;
            let mut shortest_wait: Option<Duration> = None;
            let mut last_error = None;

            for (idx, score) in &candidates {
                if *score <= 0.0 {
                    continue;
                }

                let provider = &self.providers[*idx];
                let pid = provider.id();

                // Check preflight before pushing provider into its hard limit
                if let Err(violation) =
                    self.quota_manager
                        .check_preflight(pid, char_count as u32, 0.0)
                {
                    warn!(
                        provider = pid,
                        reason = ?violation,
                        "Preflight quota threshold reached; routing to next healthy provider"
                    );
                    let wait = self
                        .quota_manager
                        .calculate_wait_time(pid, char_count as u32);
                    if wait > Duration::ZERO {
                        shortest_wait = Some(shortest_wait.map_or(wait, |w| w.min(wait)));
                    }
                    continue;
                }

                // Acquire in-flight concurrency permit
                let permit = match self.quota_manager.acquire_concurrency(pid) {
                    Ok(p) => p,
                    Err(violation) => {
                        warn!(
                            provider = pid,
                            reason = ?violation,
                            "Provider at active concurrency limit; skipping to next candidate"
                        );
                        continue;
                    }
                };

                attempted_any = true;
                debug!(provider = pid, score = *score, "Attempting TTS synthesis");

                let start = Instant::now();

                // Attempt synthesis with transient backoff for 5xx/timeouts
                let mut synth_res = provider.synthesize(trimmed, language.clone()).await;

                if let Err(ref e) = synth_res {
                    if e.is_transient() {
                        // Single quick retry with backoff and jitter
                        warn!(
                            provider = pid,
                            error = %e,
                            "Transient error on TTS provider; retrying with backoff"
                        );
                        tokio::time::sleep(Duration::from_millis(50)).await;
                        synth_res = provider.synthesize(trimmed, language.clone()).await;
                    }
                }

                drop(permit); // Release concurrency slot immediately after request completes

                match synth_res {
                    Ok(audio) => {
                        let elapsed = start.elapsed();
                        let est_cost = provider.estimate_cost(char_count);

                        self.quota_manager.record_success(
                            pid,
                            elapsed,
                            char_count as u32,
                            audio.duration_secs(),
                            est_cost.amount_usd,
                        );

                        info!(
                            provider = pid,
                            elapsed_ms = elapsed.as_millis(),
                            cost_usd = est_cost.amount_usd,
                            "TTS synthesis successful"
                        );

                        // Cache synthesized result to prevent duplicate work
                        self.cache.insert(trimmed, language.as_ref(), audio.clone());

                        return Ok(audio);
                    }
                    Err(err) => {
                        let elapsed = start.elapsed();
                        warn!(
                            provider = pid,
                            error = %err,
                            elapsed_ms = elapsed.as_millis(),
                            "TTS provider failed, attempting failover"
                        );

                        match &err {
                            ProviderError::RateLimited { retry_after, .. } => {
                                self.quota_manager.record_rate_limit(pid, *retry_after);
                            }
                            ProviderError::Timeout(_) => {
                                self.quota_manager.record_timeout(pid);
                            }
                            ProviderError::InvalidCredentials(_) | ProviderError::AuthError(_) => {
                                self.quota_manager.record_invalid_credentials(pid);
                            }
                            _ => {
                                self.quota_manager.record_failure(pid);
                            }
                        }

                        last_error = Some(err);
                    }
                }
            }

            // If no provider could be attempted because all were temporarily saturated / at RPM limit
            if !attempted_any {
                if let Some(wait) = shortest_wait {
                    let now = Instant::now();
                    if now < queue_deadline && wait <= Duration::from_millis(1200) {
                        let sleep_duration = wait
                            .min(queue_deadline - now)
                            .max(Duration::from_millis(20));
                        info!(
                            wait_ms = sleep_duration.as_millis(),
                            "All TTS providers temporarily at safe threshold; queueing request"
                        );
                        tokio::time::sleep(sleep_duration).await;
                        continue;
                    }
                }
            }

            // Emergency local fallback if cloud providers are exhausted, failed, or offline
            if self.mode.allows_local() {
                if let Some(ref fallback) = self.emergency_fallback {
                    let pid = fallback.id();
                    warn!(
                        fallback = pid,
                        "Cloud TTS exhausted: using emergency local fallback"
                    );
                    let start = Instant::now();
                    match fallback.synthesize(trimmed, language).await {
                        Ok(audio) => {
                            let elapsed = start.elapsed();
                            info!(
                                fallback = pid,
                                elapsed_ms = elapsed.as_millis(),
                                "Emergency local TTS succeeded"
                            );
                            self.cache.insert(trimmed, None, audio.clone());
                            return Ok(audio);
                        }
                        Err(e) => {
                            error!(fallback = pid, error = %e, "Emergency local fallback failed");
                            last_error = Some(e);
                        }
                    }
                }
            }

            error!("All available TTS providers failed or exhausted");
            return Err(last_error.unwrap_or_else(|| {
                ProviderError::AllProvidersFailed(
                    "All TTS providers are exhausted, in cooldown, or failed".into(),
                )
            }));
        }
    }

    /// Synthesize speech stream if supported by top candidate.
    pub async fn synthesize_stream(
        &self,
        text: &str,
        language: Option<VoiceLanguage>,
    ) -> Result<TTSStream, ProviderError> {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            let empty_audio = AudioData::new(Vec::new(), 16000, 1);
            return Ok(Box::pin(futures::stream::iter(vec![Ok(empty_audio)])));
        }

        // If in cache, return single-chunk stream immediately
        if let Some(cached) = self.cache.get(trimmed, language.as_ref()) {
            info!("Duplicate streaming TTS request: returning cached audio stream");
            return Ok(Box::pin(futures::stream::iter(vec![Ok(cached)])));
        }

        // ── Strict Local Offline Mode ────────────────────────────────────
        if self.mode.is_local_only() {
            let audio = self.synthesize(trimmed, language).await?;
            return Ok(Box::pin(futures::stream::iter(vec![Ok(audio)])));
        }

        let char_count = trimmed.chars().count();

        let mut candidates: Vec<(usize, f64)> = self
            .providers
            .iter()
            .enumerate()
            .filter(|(_, p)| p.capabilities().streaming)
            .map(|(idx, p)| {
                let score = self.calculate_score(p.as_ref(), language.clone(), char_count);
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

            if self
                .quota_manager
                .check_preflight(pid, char_count as u32, 0.0)
                .is_err()
            {
                continue;
            }

            let permit = match self.quota_manager.acquire_concurrency(pid) {
                Ok(p) => p,
                Err(_) => continue,
            };

            match provider.synthesize_stream(trimmed, language.clone()).await {
                Ok(stream) => {
                    info!(provider = pid, "Started streaming TTS playback");
                    drop(permit);
                    return Ok(stream);
                }
                Err(err) => {
                    drop(permit);
                    warn!(provider = pid, error = %err, "Streaming TTS failed on provider");
                    match &err {
                        ProviderError::RateLimited { retry_after, .. } => {
                            self.quota_manager.record_rate_limit(pid, *retry_after);
                        }
                        ProviderError::Timeout(_) => {
                            self.quota_manager.record_timeout(pid);
                        }
                        _ => {
                            self.quota_manager.record_failure(pid);
                        }
                    }
                }
            }
        }

        // If streaming unavailable, synthesize full audio and return as 1-item stream
        let audio = self.synthesize(trimmed, language).await?;
        let stream = futures::stream::iter(vec![Ok(audio)]);
        Ok(Box::pin(stream))
    }
}
