//! Voice Provider Admin & Debug API and embedded HTTP server.

use serde_json::{json, Value};
use std::sync::Arc;
use std::time::Instant;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tracing::{info, warn};

use crate::provider::quota::ProviderQuotaManager;
use crate::provider::router::{STTRouter, TTSRouter};
use crate::provider::traits::AudioData;

pub struct VoiceAdminApi {
    quota_manager: Arc<ProviderQuotaManager>,
    stt_router: Option<Arc<STTRouter>>,
    tts_router: Option<Arc<TTSRouter>>,
    registered_providers: Vec<ProviderDescriptor>,
}

#[derive(Clone, serde::Serialize)]
pub struct ProviderDescriptor {
    pub id: String,
    pub name: String,
    pub provider_type: &'static str,
    pub streaming: bool,
    pub supported_languages: Vec<String>,
    pub estimated_cost_info: String,
}

impl VoiceAdminApi {
    pub fn new(
        quota_manager: Arc<ProviderQuotaManager>,
        stt_router: Option<Arc<STTRouter>>,
        tts_router: Option<Arc<TTSRouter>>,
        registered_providers: Vec<ProviderDescriptor>,
    ) -> Self {
        Self {
            quota_manager,
            stt_router,
            tts_router,
            registered_providers,
        }
    }

    /// GET /voice/providers
    pub fn get_providers(&self) -> Value {
        let providers: Vec<Value> = self
            .registered_providers
            .iter()
            .map(|p| {
                let status = self.quota_manager.get_status(&p.id);
                json!({
                    "id": p.id,
                    "name": p.name,
                    "type": p.provider_type,
                    "streaming": p.streaming,
                    "languages": p.supported_languages,
                    "cost_info": p.estimated_cost_info,
                    "health": format!("{:?}", status.health),
                    "cooldown": status.is_cooldown,
                    "latency_p50_ms": status.latency_p50_ms,
                    "remaining_pct": (status.remaining_pct * 100.0).round() / 100.0,
                })
            })
            .collect();

        json!({
            "status": "ok",
            "provider_count": providers.len(),
            "providers": providers,
        })
    }

    /// GET /voice/quota
    pub fn get_quota(&self) -> Value {
        let all_metrics = self.quota_manager.get_all_metrics();
        let total_cost: f64 = all_metrics.values().map(|m| m.total_cost_usd).sum();
        let total_requests: u64 = all_metrics.values().map(|m| m.total_requests).sum();

        let providers_quota: Vec<Value> = all_metrics
            .into_iter()
            .map(|(id, m)| {
                json!({
                    "id": id,
                    "health": format!("{:?}", m.health),
                    "rpm_current": m.current_rpm,
                    "rpm_limit": m.limit_rpm,
                    "rph_current": m.current_rph,
                    "rph_limit": m.limit_rph,
                    "rpd_current": m.current_rpd,
                    "rpd_limit": m.limit_rpd,
                    "total_requests": m.total_requests,
                    "total_failures": m.total_failures,
                    "total_rate_limits_429": m.total_rate_limits,
                    "total_cost_usd": (m.total_cost_usd * 10000.0).round() / 10000.0,
                    "latency_p50_ms": m.latency_p50_ms,
                    "latency_p95_ms": m.latency_p95_ms,
                    "in_cooldown": m.is_cooldown,
                })
            })
            .collect();

        json!({
            "status": "ok",
            "routing_mode": format!("{:?}", self.quota_manager.routing_mode()),
            "total_estimated_cost_usd": (total_cost * 10000.0).round() / 10000.0,
            "total_requests": total_requests,
            "providers": providers_quota,
        })
    }

    /// GET /voice/health
    pub fn get_health(&self) -> Value {
        let all_metrics = self.quota_manager.get_all_metrics();
        let mut overall_healthy = true;
        let mut degraded_count = 0;
        let mut exhausted_count = 0;

        for m in all_metrics.values() {
            match m.health {
                crate::provider::traits::HealthStatus::Healthy => {}
                crate::provider::traits::HealthStatus::Degraded(_) => degraded_count += 1,
                crate::provider::traits::HealthStatus::Exhausted
                | crate::provider::traits::HealthStatus::Cooldown => {
                    exhausted_count += 1;
                    overall_healthy = false;
                }
                _ => {}
            }
        }

        json!({
            "status": if overall_healthy { "healthy" } else { "degraded" },
            "degraded_providers": degraded_count,
            "exhausted_providers": exhausted_count,
            "total_registered": self.registered_providers.len(),
            "uptime_active": true,
        })
    }

    /// POST /voice/test/stt
    pub async fn test_stt(&self, test_pcm: Option<Vec<u8>>) -> Value {
        let router = match self.stt_router {
            Some(ref r) => r,
            None => {
                return json!({ "status": "error", "message": "STT router not initialized" });
            }
        };

        let audio = match test_pcm {
            Some(bytes) => {
                let samples: Vec<f32> = bytes
                    .chunks_exact(2)
                    .map(|c| i16::from_le_bytes([c[0], c[1]]) as f32 / 32768.0)
                    .collect();
                AudioData::from_pcm(samples, 16000, 1)
            }
            None => {
                // Generate 1 second of dummy silence/sample audio
                let samples = vec![0.0f32; 16000];
                AudioData::from_pcm(samples, 16000, 1)
            }
        };

        let start = Instant::now();
        match router.transcribe(audio, None).await {
            Ok(transcript) => json!({
                "status": "ok",
                "latency_ms": start.elapsed().as_millis(),
                "transcript": transcript.text,
                "confidence": transcript.confidence,
            }),
            Err(e) => json!({
                "status": "error",
                "latency_ms": start.elapsed().as_millis(),
                "error": e.to_string(),
            }),
        }
    }

    /// POST /voice/test/tts
    pub async fn test_tts(&self, text: &str) -> Value {
        let router = match self.tts_router {
            Some(ref r) => r,
            None => {
                return json!({ "status": "error", "message": "TTS router not initialized" });
            }
        };

        let start = Instant::now();
        match router.synthesize(text, None).await {
            Ok(audio) => json!({
                "status": "ok",
                "latency_ms": start.elapsed().as_millis(),
                "audio_samples": audio.pcm_bytes.len() / 2,
                "duration_secs": audio.duration_secs(),
                "sample_rate": audio.sample_rate,
            }),
            Err(e) => json!({
                "status": "error",
                "latency_ms": start.elapsed().as_millis(),
                "error": e.to_string(),
            }),
        }
    }

    /// POST /voice/cooldown/reset
    pub fn reset_cooldown(&self, provider_id: Option<&str>) -> Value {
        match provider_id {
            Some(id) => {
                self.quota_manager.reset_cooldown(id);
                json!({ "status": "ok", "message": format!("Reset cooldown for provider {}", id) })
            }
            None => {
                self.quota_manager.reset_all_cooldowns();
                json!({ "status": "ok", "message": "Reset cooldown for all providers" })
            }
        }
    }
}

/// Start background lightweight HTTP administration listener.
pub async fn start_admin_server(port: u16, api: Arc<VoiceAdminApi>) -> Result<(), std::io::Error> {
    let addr = format!("127.0.0.1:{}", port);
    let listener = TcpListener::bind(&addr).await?;
    info!(addr = %addr, "Voice Admin & Debug HTTP server listening");

    tokio::spawn(async move {
        loop {
            let (mut socket, _) = match listener.accept().await {
                Ok(conn) => conn,
                Err(e) => {
                    warn!("Admin server accept error: {}", e);
                    continue;
                }
            };

            let api = Arc::clone(&api);
            tokio::spawn(async move {
                let mut buffer = [0u8; 4096];
                let n = match socket.read(&mut buffer).await {
                    Ok(n) if n > 0 => n,
                    _ => return,
                };

                let request = String::from_utf8_lossy(&buffer[..n]);
                let mut lines = request.lines();
                let request_line = match lines.next() {
                    Some(l) => l,
                    None => return,
                };

                let parts: Vec<&str> = request_line.split_whitespace().collect();
                if parts.len() < 2 {
                    return;
                }

                let method = parts[0];
                let path = parts[1];

                let (status_code, body) = match (method, path) {
                    ("GET", "/voice/providers") => (200, api.get_providers()),
                    ("GET", "/voice/quota") => (200, api.get_quota()),
                    ("GET", "/voice/health") => (200, api.get_health()),
                    ("POST", "/voice/test/stt") => {
                        let res = api.test_stt(None).await;
                        (200, res)
                    }
                    ("POST", "/voice/test/tts") => {
                        let res = api.test_tts("VOXY cloud voice pipeline operational.").await;
                        (200, res)
                    }
                    ("POST", "/voice/cooldown/reset") => {
                        let res = api.reset_cooldown(None);
                        (200, res)
                    }
                    _ => (404, json!({ "error": "Not Found", "path": path })),
                };

                let body_str = body.to_string();
                let response = format!(
                    "HTTP/1.1 {} OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\nConnection: close\r\n\r\n{}",
                    status_code,
                    body_str.len(),
                    body_str
                );

                let _ = socket.write_all(response.as_bytes()).await;
                let _ = socket.flush().await;
            });
        }
    });

    Ok(())
}
