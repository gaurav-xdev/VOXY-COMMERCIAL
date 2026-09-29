use async_trait::async_trait;

use crate::error::Result;
use crate::registry::ProviderHealth;

#[async_trait]
pub trait ProviderHealthChecker: Send + Sync {
    async fn check_health(&self, provider_id: &str) -> Result<ProviderHealth>;
    async fn is_healthy(&self, provider_id: &str) -> Result<bool>;
    fn interval_seconds(&self) -> u64;
}

pub struct NoopHealthChecker;

#[async_trait]
impl ProviderHealthChecker for NoopHealthChecker {
    async fn check_health(&self, provider_id: &str) -> Result<ProviderHealth> {
        let mut h = ProviderHealth::new_healthy(None);
        h.details = Some(format!("No-op health check for {provider_id}"));
        Ok(h)
    }

    async fn is_healthy(&self, _provider_id: &str) -> Result<bool> {
        Ok(true)
    }

    fn interval_seconds(&self) -> u64 {
        60
    }
}

#[derive(Debug, Clone)]
pub struct ProviderBenchmarkResult {
    pub provider_name: String,
    pub model_id: String,
    pub time_to_first_token_ms: f64,
    pub tokens_per_second: f64,
    pub average_latency_ms: f64,
    pub failure_rate: f64,
    pub context_size: Option<u32>,
    pub supports_tool_calling: bool,
    pub supports_vision: bool,
    pub total_attempts: u32,
    pub successful_attempts: u32,
}

pub struct LlmBenchmarkRunner;

impl Default for LlmBenchmarkRunner {
    fn default() -> Self {
        Self
    }
}

impl LlmBenchmarkRunner {
    pub fn new() -> Self {
        Self
    }

    pub async fn benchmark_provider(
        &self,
        base_url: &str,
        model: &str,
        prompt: &str,
        num_runs: u32,
    ) -> ProviderBenchmarkResult {
        let mut ttft_samples = Vec::with_capacity(num_runs as usize);
        let mut latency_samples = Vec::with_capacity(num_runs as usize);
        let mut token_counts = Vec::with_capacity(num_runs as usize);
        let mut successes = 0u32;

        for _ in 0..num_runs {
            if let Ok((ttft, total_latency, tokens)) =
                self.run_completion(base_url, model, prompt).await
            {
                ttft_samples.push(ttft);
                latency_samples.push(total_latency);
                token_counts.push(tokens);
                successes += 1;
            }
        }

        let total_attempts = num_runs;
        let failure_rate = if total_attempts > 0 {
            (total_attempts - successes) as f64 / total_attempts as f64
        } else {
            1.0
        };

        let avg_latency = if !latency_samples.is_empty() {
            latency_samples.iter().sum::<f64>() / latency_samples.len() as f64
        } else {
            0.0
        };

        let avg_ttft = if !ttft_samples.is_empty() {
            ttft_samples.iter().sum::<f64>() / ttft_samples.len() as f64
        } else {
            0.0
        };

        let avg_tps = if !token_counts.is_empty() && !latency_samples.is_empty() {
            let total_tokens: u32 = token_counts.iter().sum();
            let total_lat: f64 = latency_samples.iter().sum();
            if total_lat > 0.0 {
                total_tokens as f64 / (total_lat / 1000.0)
            } else {
                0.0
            }
        } else {
            0.0
        };

        let context_size = self.detect_context_size(base_url, model).await;
        let supports_tool_calling = self.detect_tool_calling(base_url, model).await;
        let supports_vision = self.detect_vision(base_url, model).await;

        ProviderBenchmarkResult {
            provider_name: base_url.to_string(),
            model_id: model.to_string(),
            time_to_first_token_ms: avg_ttft,
            tokens_per_second: avg_tps,
            average_latency_ms: avg_latency,
            failure_rate,
            context_size,
            supports_tool_calling,
            supports_vision,
            total_attempts,
            successful_attempts: successes,
        }
    }

    async fn run_completion(
        &self,
        base_url: &str,
        model: &str,
        prompt: &str,
    ) -> std::result::Result<(f64, f64, u32), String> {
        let url = format!("{}/v1/chat/completions", base_url);
        let body = serde_json::json!({
            "model": model,
            "messages": [{"role": "user", "content": prompt}],
            "stream": false,
            "max_tokens": 50,
        });

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| e.to_string())?;

        let start = std::time::Instant::now();
        let resp = client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        let ttft = start.elapsed().as_secs_f64() * 1000.0;

        let json: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
        let total_latency = start.elapsed().as_secs_f64() * 1000.0;

        let content = json["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or("");
        let tokens = content.split_whitespace().count() as u32;

        Ok((ttft, total_latency, tokens.max(1)))
    }

    async fn detect_context_size(&self, base_url: &str, model: &str) -> Option<u32> {
        let url = format!("{}/v1/models", base_url);
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()
            .ok()?;

        let resp = client.get(&url).send().await.ok()?;
        let json: serde_json::Value = resp.json().await.ok()?;

        if let Some(data) = json["data"].as_array() {
            for m in data {
                if m["id"].as_str() == Some(model) {
                    if let Some(ctx) = m["context_size"].as_u64() {
                        return Some(ctx as u32);
                    }
                    if let Some(ctx) = m["max_context_length"].as_u64() {
                        return Some(ctx as u32);
                    }
                }
            }
        }

        if base_url.contains("11434") {
            let url = format!("{}/api/show", base_url);
            let body = serde_json::json!({"name": model});
            if let Ok(resp) = client.post(&url).json(&body).send().await {
                if let Ok(json) = resp.json::<serde_json::Value>().await {
                    if let Some(ctx) = json["context_length"].as_u64() {
                        return Some(ctx as u32);
                    }
                    if let Some(details) = json["details"].as_object() {
                        if let Some(ctx) = details.get("context_length").and_then(|v| v.as_u64()) {
                            return Some(ctx as u32);
                        }
                    }
                }
            }
        }

        None
    }

    async fn detect_tool_calling(&self, base_url: &str, model: &str) -> bool {
        let url = format!("{}/v1/chat/completions", base_url);
        let body = serde_json::json!({
            "model": model,
            "messages": [{"role": "user", "content": "hello"}],
            "tools": [{
                "type": "function",
                "function": {
                    "name": "test",
                    "description": "test",
                    "parameters": {"type": "object", "properties": {}}
                }
            }],
            "max_tokens": 1,
        });

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .ok();

        match client {
            Some(c) => c.post(&url).json(&body).send().await.is_ok(),
            None => false,
        }
    }

    async fn detect_vision(&self, base_url: &str, model: &str) -> bool {
        let url = format!("{}/v1/chat/completions", base_url);
        let body = serde_json::json!({
            "model": model,
            "messages": [{
                "role": "user",
                "content": [
                    {"type": "text", "text": "describe"},
                    {"type": "image_url", "image_url": {"url": "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg=="}}
                ]
            }],
            "max_tokens": 10,
        });

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .ok();

        match client {
            Some(c) => c.post(&url).json(&body).send().await.is_ok(),
            None => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_noop_health_checker() {
        let checker = NoopHealthChecker;
        let health = checker.check_health("test-provider").await.unwrap();
        assert!(health.is_healthy);
        assert!(health.details.is_some());
    }

    #[tokio::test]
    async fn test_noop_is_healthy() {
        let checker = NoopHealthChecker;
        let healthy = checker.is_healthy("test-provider").await.unwrap();
        assert!(healthy);
    }

    #[test]
    fn test_interval_seconds() {
        let checker = NoopHealthChecker;
        assert_eq!(checker.interval_seconds(), 60);
    }

    #[tokio::test]
    async fn test_benchmark_runner_creation() {
        let runner = LlmBenchmarkRunner::new();
        let result = runner
            .run_completion("http://127.0.0.1:9999", "test", "hello")
            .await;
        assert!(result.is_err());
    }

    #[test]
    fn test_provider_benchmark_result_defaults() {
        let r = ProviderBenchmarkResult {
            provider_name: "test".into(),
            model_id: "m".into(),
            time_to_first_token_ms: 100.0,
            tokens_per_second: 10.0,
            average_latency_ms: 200.0,
            failure_rate: 0.0,
            context_size: Some(4096),
            supports_tool_calling: true,
            supports_vision: false,
            total_attempts: 5,
            successful_attempts: 5,
        };
        assert_eq!(r.provider_name, "test");
        assert_eq!(r.model_id, "m");
        assert!(r.supports_tool_calling);
        assert!(!r.supports_vision);
    }
}
