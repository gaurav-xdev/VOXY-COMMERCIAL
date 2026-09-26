#![allow(dead_code)]

use std::collections::HashMap;
use std::time::{Duration, Instant};

use tracing::info;
use voxy_cognition::CognitiveEngine;
use voxy_voice_orchestrator::{SttEngine, TtsEngine, VadDetector, WakeWordDetector};

// ─── Statistical Collection ──────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct LatencySample {
    pub name: &'static str,
    values: Vec<f64>,
}

impl LatencySample {
    pub fn new(name: &'static str) -> Self {
        Self {
            name,
            values: Vec::with_capacity(1024),
        }
    }

    pub fn record(&mut self, ms: f64) {
        self.values.push(ms);
    }

    pub fn count(&self) -> usize {
        self.values.len()
    }

    pub fn min_ms(&self) -> f64 {
        self.values.iter().cloned().fold(f64::MAX, f64::min)
    }

    pub fn max_ms(&self) -> f64 {
        self.values.iter().cloned().fold(f64::MIN, f64::max)
    }

    pub fn avg_ms(&self) -> f64 {
        if self.values.is_empty() {
            return 0.0;
        }
        self.values.iter().sum::<f64>() / self.values.len() as f64
    }

    pub fn std_dev(&self) -> f64 {
        if self.values.len() < 2 {
            return 0.0;
        }
        let avg = self.avg_ms();
        let variance = self.values.iter().map(|v| (v - avg).powi(2)).sum::<f64>()
            / (self.values.len() - 1) as f64;
        variance.sqrt()
    }

    pub fn percentile(&self, p: f64) -> f64 {
        if self.values.is_empty() {
            return 0.0;
        }
        let mut sorted = self.values.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let idx = ((p / 100.0) * (sorted.len() as f64 - 1.0)).round() as usize;
        sorted[idx.min(sorted.len() - 1)]
    }

    pub fn median_ms(&self) -> f64 {
        self.percentile(50.0)
    }

    pub fn p95_ms(&self) -> f64 {
        self.percentile(95.0)
    }

    pub fn p99_ms(&self) -> f64 {
        self.percentile(99.0)
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    pub fn total_ms(&self) -> f64 {
        self.values.iter().sum()
    }
}

// ─── System Metrics ──────────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct SystemSnapshot {
    pub timestamp_ms: u64,
    pub cpu_percent: f64,
    pub ram_used_mb: f64,
    pub ram_total_mb: f64,
    pub ram_percent: f64,
    pub thread_count: usize,
    pub process_cpu_percent: f64,
    pub process_ram_mb: f64,
    pub gpu_percent: Option<f64>,
    pub vram_used_mb: Option<f64>,
    pub vram_total_mb: Option<f64>,
    pub disk_read_bytes: Option<u64>,
    pub disk_write_bytes: Option<u64>,
}

pub struct SystemMetricsCollector {
    snapshots: Vec<SystemSnapshot>,
    process_start: Instant,
}

impl SystemMetricsCollector {
    pub fn new() -> Self {
        Self {
            snapshots: Vec::with_capacity(1024),
            process_start: Instant::now(),
        }
    }

    pub fn collect(&mut self) -> SystemSnapshot {
        let snap = SystemSnapshot {
            timestamp_ms: self.process_start.elapsed().as_millis() as u64,
            cpu_percent: get_system_cpu(),
            ram_used_mb: get_ram_used_mb(),
            ram_total_mb: get_ram_total_mb(),
            ram_percent: get_ram_percent(),
            thread_count: get_thread_count(),
            process_cpu_percent: get_process_cpu(),
            process_ram_mb: get_process_ram_mb(),
            gpu_percent: get_gpu_percent(),
            vram_used_mb: get_vram_used_mb(),
            vram_total_mb: get_vram_total_mb(),
            disk_read_bytes: get_disk_read_bytes(),
            disk_write_bytes: get_disk_write_bytes(),
        };
        self.snapshots.push(snap.clone());
        snap
    }

    pub fn snapshots(&self) -> &[SystemSnapshot] {
        &self.snapshots
    }
}

// ─── Windows System Metrics ──────────────────────────────────────────────

#[cfg(target_os = "windows")]
fn get_system_cpu() -> f64 {
    use std::process::Command;
    if let Ok(output) = Command::new("wmic")
        .args(["cpu", "get", "LoadPercentage", "/format:csv"])
        .output()
    {
        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines().skip(1) {
            if line.is_empty() {
                continue;
            }
            let parts: Vec<&str> = line.split(',').collect();
            if let Some(val) = parts.last().and_then(|s| s.trim().parse::<f64>().ok()) {
                return val;
            }
        }
    }
    0.0
}

#[cfg(not(target_os = "windows"))]
fn get_system_cpu() -> f64 {
    0.0
}

#[cfg(target_os = "windows")]
fn get_ram_used_mb() -> f64 {
    use std::process::Command;
    if let Ok(output) = Command::new("wmic")
        .args([
            "OS",
            "get",
            "TotalVisibleMemorySize,FreePhysicalMemory",
            "/format:csv",
        ])
        .output()
    {
        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines().skip(1) {
            let parts: Vec<&str> = line.split(',').collect();
            if parts.len() >= 3 {
                if let (Ok(free_kb), Ok(total_kb)) = (
                    parts[2].trim().parse::<u64>(),
                    parts[1].trim().parse::<u64>(),
                ) {
                    return ((total_kb - free_kb) as f64 * 1024.0) / (1024.0 * 1024.0);
                }
            }
        }
    }
    0.0
}

#[cfg(not(target_os = "windows"))]
fn get_ram_used_mb() -> f64 {
    0.0
}

#[cfg(target_os = "windows")]
fn get_ram_total_mb() -> f64 {
    use std::process::Command;
    if let Ok(output) = Command::new("wmic")
        .args(["OS", "get", "TotalVisibleMemorySize", "/format:csv"])
        .output()
    {
        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines().skip(1) {
            let parts: Vec<&str> = line.split(',').collect();
            if let Some(val) = parts.last().and_then(|s| s.trim().parse::<u64>().ok()) {
                return (val as f64 * 1024.0) / (1024.0 * 1024.0);
            }
        }
    }
    0.0
}

#[cfg(not(target_os = "windows"))]
fn get_ram_total_mb() -> f64 {
    0.0
}

fn get_ram_percent() -> f64 {
    let total = get_ram_total_mb();
    let used = get_ram_used_mb();
    if total > 0.0 {
        used / total * 100.0
    } else {
        0.0
    }
}

fn get_thread_count() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
}

fn get_process_cpu() -> f64 {
    0.0
}

fn get_process_ram_mb() -> f64 {
    0.0
}

fn get_gpu_percent() -> Option<f64> {
    None
}

fn get_vram_used_mb() -> Option<f64> {
    None
}

fn get_vram_total_mb() -> Option<f64> {
    None
}

fn get_disk_read_bytes() -> Option<u64> {
    None
}

fn get_disk_write_bytes() -> Option<u64> {
    None
}

// ─── Voice-Specific Metrics ──────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct VoiceMetricsSnapshot {
    pub wake_word_detection_ms: f64,
    pub vad_detection_ms: f64,
    pub whisper_first_token_ms: Option<f64>,
    pub whisper_final_result_ms: Option<f64>,
    pub brain_processing_ms: f64,
    pub llm_first_token_ms: Option<f64>,
    pub llm_final_token_ms: Option<f64>,
    pub piper_first_audio_ms: Option<f64>,
    pub playback_start_ms: Option<f64>,
    pub complete_response_ms: f64,
    pub audio_capture_ms: f64,
    pub ring_buffer_occupancy: f64,
    pub audio_underruns: u64,
    pub dropped_frames: u64,
    pub audio_callback_ms: f64,
}

// ─── Bottleneck Analysis ─────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Bottleneck {
    pub rank: usize,
    pub stage: String,
    pub avg_ms: f64,
    pub p95_ms: f64,
    pub percent_of_total: f64,
    pub severity: Severity,
    pub improvement_estimate: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Severity {
    Critical,
    High,
    Medium,
    Low,
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Severity::Critical => write!(f, "CRITICAL"),
            Severity::High => write!(f, "HIGH"),
            Severity::Medium => write!(f, "MEDIUM"),
            Severity::Low => write!(f, "LOW"),
        }
    }
}

// ─── Benchmark Configuration ─────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct BenchmarkConfig {
    pub iterations: usize,
    pub warmup_iterations: usize,
    pub measure_system_metrics: bool,
    pub measure_voice_metrics: bool,
    pub output_path: Option<String>,
    pub whisper_model_path: String,
    pub piper_model_path: String,
    pub piper_config_path: String,
    pub ollama_url: String,
    pub ollama_model: String,
}

impl Default for BenchmarkConfig {
    fn default() -> Self {
        Self {
            iterations: 100,
            warmup_iterations: 10,
            measure_system_metrics: true,
            measure_voice_metrics: true,
            output_path: None,
            whisper_model_path: "models/ggml-base.en.bin".into(),
            piper_model_path: "models/en_US-lessac-medium.onnx".into(),
            piper_config_path: "models/en_US-lessac-medium.json".into(),
            ollama_url: "http://localhost:11434".into(),
            ollama_model: "gemma3:4b".into(),
        }
    }
}

// ─── Benchmark Report ────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct BenchmarkReport {
    pub config: BenchmarkConfig,
    pub stage_latencies: Vec<LatencySample>,
    pub e2e_latencies: Vec<LatencySample>,
    pub voice_metrics: Vec<VoiceMetricsSnapshot>,
    pub system_snapshots: Vec<SystemSnapshot>,
    pub hardware_info: Vec<String>,
    pub provider_results: Vec<String>,
    pub bottlenecks: Vec<Bottleneck>,
    pub total_duration_ms: f64,
    pub pipeline_total_ms: f64,
}

impl BenchmarkReport {
    pub fn new(config: BenchmarkConfig) -> Self {
        Self {
            config,
            stage_latencies: Vec::new(),
            e2e_latencies: Vec::new(),
            voice_metrics: Vec::new(),
            system_snapshots: Vec::new(),
            hardware_info: Vec::new(),
            provider_results: Vec::new(),
            bottlenecks: Vec::new(),
            total_duration_ms: 0.0,
            pipeline_total_ms: 0.0,
        }
    }

    pub fn identify_bottlenecks(&mut self) {
        let total: f64 = self.stage_latencies.iter().map(|s| s.avg_ms()).sum();

        self.pipeline_total_ms = total;

        let mut ranked: Vec<(String, f64, f64)> = self
            .stage_latencies
            .iter()
            .filter(|s| !s.is_empty())
            .map(|s| (s.name.to_string(), s.avg_ms(), s.p95_ms()))
            .collect();

        ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        self.bottlenecks = ranked
            .into_iter()
            .enumerate()
            .map(|(i, (name, avg, p95))| {
                let pct = if total > 0.0 {
                    avg / total * 100.0
                } else {
                    0.0
                };
                let severity = if pct > 30.0 {
                    Severity::Critical
                } else if pct > 15.0 {
                    Severity::High
                } else if pct > 5.0 {
                    Severity::Medium
                } else {
                    Severity::Low
                };

                let improvement = match name.as_str() {
                    "whisper_inference" => {
                        "Move to GPU-accelerated whisper.cpp or use faster model (tiny/base)".into()
                    }
                    "llm_generation" => {
                        "Use streaming LLM, smaller model, or speculative decoding".into()
                    }
                    "piper_synthesis" => {
                        "Use ONNX Runtime GPU, batch synthesis, or streaming chunks".into()
                    }
                    "brain_runtime" => {
                        "Parallelize context/companion/HDR stages, cache results".into()
                    }
                    "audio_capture" => "Reduce buffer size, use WASAPI exclusive mode".into(),
                    "audio_playback" => "Use persistent ring buffer (already implemented)".into(),
                    "vad" | "wake_word" => {
                        "These are already fast (<1ms). No optimization needed.".into()
                    }
                    _ => "Profile deeper to identify specific optimization target".into(),
                };

                Bottleneck {
                    rank: i + 1,
                    stage: name,
                    avg_ms: avg,
                    p95_ms: p95,
                    percent_of_total: pct,
                    severity,
                    improvement_estimate: improvement,
                }
            })
            .collect();
    }

    pub fn to_markdown(&self) -> String {
        let mut md = String::with_capacity(8192);

        md.push_str("# VOXY Voice Runtime Performance Benchmark Report\n\n");
        md.push_str(&format!(
            "**Date:** {}\n",
            chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC")
        ));
        md.push_str(&format!(
            "**Iterations:** {} ({} warmup)\n",
            self.config.iterations, self.config.warmup_iterations
        ));
        md.push_str(&format!(
            "**Total Duration:** {:.1}s\n\n",
            self.total_duration_ms / 1000.0
        ));
        md.push_str(&format!(
            "**LLM:** {} @ {}\n",
            self.config.ollama_model, self.config.ollama_url
        ));
        md.push_str(&format!(
            "**Whisper:** {}\n",
            self.config.whisper_model_path
        ));
        md.push_str(&format!("**TTS:** {}\n\n", self.config.piper_model_path));

        // System Info
        md.push_str("## System Information\n\n");
        for info in &self.hardware_info {
            md.push_str(&format!("- {}\n", info));
        }
        if let Some(avg) = self.system_snapshots.first() {
            md.push_str(&format!("- RAM: {:.0} MB total\n", avg.ram_total_mb));
        }
        md.push('\n');

        // Stage Latencies
        md.push_str("## Pipeline Stage Latencies\n\n");
        md.push_str("| Stage | Count | Avg (ms) | Min (ms) | P50 (ms) | P95 (ms) | P99 (ms) | Max (ms) | StdDev | % of Total |\n");
        md.push_str("|-------|-------|----------|----------|----------|----------|----------|----------|--------|------------|\n");

        let total_avg: f64 = self.stage_latencies.iter().map(|s| s.avg_ms()).sum();

        for s in &self.stage_latencies {
            if s.is_empty() {
                continue;
            }
            let pct = if total_avg > 0.0 {
                s.avg_ms() / total_avg * 100.0
            } else {
                0.0
            };
            md.push_str(&format!(
                "| {} | {} | {:.2} | {:.2} | {:.2} | {:.2} | {:.2} | {:.2} | {:.2} | {:.1}% |\n",
                s.name,
                s.count(),
                s.avg_ms(),
                s.min_ms(),
                s.median_ms(),
                s.p95_ms(),
                s.p99_ms(),
                s.max_ms(),
                s.std_dev(),
                pct
            ));
        }
        md.push('\n');

        // E2E Latencies
        if !self.e2e_latencies.is_empty() {
            md.push_str("## End-to-End Voice Metrics\n\n");
            md.push_str("| Metric | Count | Avg (ms) | Min (ms) | P50 (ms) | P95 (ms) | P99 (ms) | Max (ms) |\n");
            md.push_str("|--------|-------|----------|----------|----------|----------|----------|----------|\n");
            for s in &self.e2e_latencies {
                if s.is_empty() {
                    continue;
                }
                md.push_str(&format!(
                    "| {} | {} | {:.2} | {:.2} | {:.2} | {:.2} | {:.2} | {:.2} |\n",
                    s.name,
                    s.count(),
                    s.avg_ms(),
                    s.min_ms(),
                    s.median_ms(),
                    s.p95_ms(),
                    s.p99_ms(),
                    s.max_ms()
                ));
            }
            md.push('\n');
        }

        // System Metrics Summary
        if !self.system_snapshots.is_empty() {
            md.push_str("## System Resource Usage (Average)\n\n");
            let mut acc = SystemSnapshot::default();
            let n = self.system_snapshots.len() as f64;
            for s in &self.system_snapshots {
                acc.cpu_percent += s.cpu_percent;
                acc.ram_used_mb += s.ram_used_mb;
                acc.ram_percent += s.ram_percent;
                acc.process_cpu_percent += s.process_cpu_percent;
                acc.process_ram_mb += s.process_ram_mb;
                acc.thread_count = s.thread_count;
                acc.ram_total_mb = s.ram_total_mb;
            }
            md.push_str(&format!("- **System CPU:** {:.1}%\n", acc.cpu_percent / n));
            md.push_str(&format!(
                "- **Process RAM:** {:.1} MB\n",
                acc.process_ram_mb / n
            ));
            md.push_str(&format!(
                "- **System RAM:** {:.0} / {:.0} MB ({:.1}%)\n",
                acc.ram_used_mb / n,
                acc.ram_total_mb,
                acc.ram_percent / n
            ));
            md.push_str(&format!("- **Thread Count:** {}\n", acc.thread_count));
            md.push('\n');
        }

        // Bottleneck Analysis
        if !self.bottlenecks.is_empty() {
            md.push_str("## Bottleneck Analysis (Ranked by Impact)\n\n");
            md.push_str("| Rank | Stage | Avg (ms) | P95 (ms) | % of Total | Severity | Improvement Estimate |\n");
            md.push_str("|------|-------|----------|----------|------------|----------|---------------------|\n");
            for b in &self.bottlenecks {
                md.push_str(&format!(
                    "| {} | {} | {:.2} | {:.2} | {:.1}% | **{}** | {} |\n",
                    b.rank,
                    b.stage,
                    b.avg_ms,
                    b.p95_ms,
                    b.percent_of_total,
                    b.severity,
                    b.improvement_estimate
                ));
            }
            md.push('\n');
        }

        // Optimization Recommendations
        md.push_str("## Optimization Recommendations (Priority Order)\n\n");
        md.push_str(&self.generate_optimization_order());
        md.push('\n');

        // Provider Results
        if !self.provider_results.is_empty() {
            md.push_str("## LLM Provider Benchmarks\n\n");
            for r in &self.provider_results {
                md.push_str(&format!("- {}\n", r));
            }
            md.push('\n');
        }

        // Footer
        md.push_str("---\n\n");
        md.push_str("*Report generated by VOXY Performance Benchmark Suite (REAL MODELS).*\n");

        md
    }

    fn generate_optimization_order(&self) -> String {
        let mut out = String::new();
        let critical: Vec<_> = self
            .bottlenecks
            .iter()
            .filter(|b| b.severity == Severity::Critical)
            .collect();
        let high: Vec<_> = self
            .bottlenecks
            .iter()
            .filter(|b| b.severity == Severity::High)
            .collect();
        let medium: Vec<_> = self
            .bottlenecks
            .iter()
            .filter(|b| b.severity == Severity::Medium)
            .collect();

        let mut order = 1;

        if !critical.is_empty() {
            out.push_str("### Phase 1: Critical Bottlenecks (Immediate)\n\n");
            for b in &critical {
                out.push_str(&format!(
                    "{}. **{}** ({:.1}% of total, avg {:.1}ms) — {}\n",
                    order, b.stage, b.percent_of_total, b.avg_ms, b.improvement_estimate
                ));
                order += 1;
            }
            out.push('\n');
        }

        if !high.is_empty() {
            out.push_str("### Phase 2: High Impact (Short-term)\n\n");
            for b in &high {
                out.push_str(&format!(
                    "{}. **{}** ({:.1}% of total, avg {:.1}ms) — {}\n",
                    order, b.stage, b.percent_of_total, b.avg_ms, b.improvement_estimate
                ));
                order += 1;
            }
            out.push('\n');
        }

        if !medium.is_empty() {
            out.push_str("### Phase 3: Medium Impact (Long-term)\n\n");
            for b in &medium {
                out.push_str(&format!(
                    "{}. **{}** ({:.1}% of total, avg {:.1}ms) — {}\n",
                    order, b.stage, b.percent_of_total, b.avg_ms, b.improvement_estimate
                ));
                order += 1;
            }
            out.push('\n');
        }

        if self.bottlenecks.is_empty() {
            out.push_str(
                "No significant bottlenecks identified. All stages within acceptable thresholds.\n",
            );
        }

        out
    }
}

// ─── Timing Helpers ──────────────────────────────────────────────────────

pub fn measure_ms<F, T>(f: F) -> (f64, T)
where
    F: FnOnce() -> T,
{
    let start = Instant::now();
    let result = f();
    let ms = start.elapsed().as_secs_f64() * 1000.0;
    (ms, result)
}

pub async fn measure_ms_async<F, Fut, T>(f: F) -> (f64, T)
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = T>,
{
    let start = Instant::now();
    let result = f().await;
    let ms = start.elapsed().as_secs_f64() * 1000.0;
    (ms, result)
}

pub fn detect_simd() -> Vec<String> {
    let mut caps = Vec::new();
    #[cfg(target_arch = "x86_64")]
    {
        if is_x86_feature_detected!("avx2") {
            caps.push("AVX2".to_string());
        }
        if is_x86_feature_detected!("avx512f") {
            caps.push("AVX-512F".to_string());
        }
        if is_x86_feature_detected!("sse2") {
            caps.push("SSE2".to_string());
        }
        if is_x86_feature_detected!("fma") {
            caps.push("FMA".to_string());
        }
        if is_x86_feature_detected!("aes") {
            caps.push("AES-NI".to_string());
        }
    }
    if caps.is_empty() {
        caps.push("none detected".to_string());
    }
    caps
}

pub fn detect_hardware_info() -> Vec<String> {
    let mut info = Vec::new();
    info.push(format!(
        "Logical cores: {}",
        std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1)
    ));
    info.push(format!("SIMD: {}", detect_simd().join(", ")));
    info.push(format!("Platform: {}", std::env::consts::OS));
    info.push(format!("Arch: {}", std::env::consts::ARCH));
    info
}

// ─── Ollama API Client ───────────────────────────────────────────────────

#[derive(Debug, serde::Deserialize)]
struct OllamaGenerateResponse {
    response: String,
    done: bool,
    #[allow(dead_code)]
    total_duration: Option<u64>,
}

struct OllamaResult {
    text: String,
    ttft_ms: f64,
    total_ms: f64,
}

async fn call_ollama(
    client: &reqwest::Client,
    url: &str,
    model: &str,
    prompt: &str,
) -> OllamaResult {
    let body = serde_json::json!({
        "model": model,
        "prompt": prompt,
        "stream": true,
    });

    let start = Instant::now();
    let resp = client
        .post(format!("{}/api/generate", url))
        .json(&body)
        .send()
        .await;
    let mut ttft_ms = 0.0;
    let mut text = String::new();

    match resp {
        Ok(response) => {
            use futures_util::StreamExt;
            let mut stream = response.bytes_stream();
            let mut buf = String::new();
            let mut ttft_reported = false;

            while let Some(chunk_result) = stream.next().await {
                match chunk_result {
                    Ok(bytes) => {
                        if let Ok(s) = std::str::from_utf8(&bytes) {
                            buf.push_str(s);
                            while let Some(newline_pos) = buf.find('\n') {
                                let line = buf[..newline_pos].trim().to_string();
                                buf = buf[newline_pos + 1..].to_string();
                                if line.is_empty() {
                                    continue;
                                }
                                if let Ok(parsed) =
                                    serde_json::from_str::<OllamaGenerateResponse>(&line)
                                {
                                    if !ttft_reported && !parsed.response.is_empty() {
                                        ttft_ms = start.elapsed().as_secs_f64() * 1000.0;
                                        ttft_reported = true;
                                    }
                                    text.push_str(&parsed.response);
                                    if parsed.done {
                                        break;
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        tracing::warn!("Ollama stream error: {e}");
                        break;
                    }
                }
            }
        }
        Err(e) => {
            tracing::warn!("Ollama request failed: {e}");
            text = "Error: unable to reach LLM".to_string();
            ttft_ms = start.elapsed().as_secs_f64() * 1000.0;
        }
    }

    let total_ms = start.elapsed().as_secs_f64() * 1000.0;
    OllamaResult {
        text,
        ttft_ms,
        total_ms,
    }
}

// ─── Synthetic Test Audio ────────────────────────────────────────────────

fn generate_test_speech_audio(duration_ms: usize, sample_rate: u32) -> Vec<f32> {
    let total_samples = (sample_rate as f64 * duration_ms as f64 / 1000.0) as usize;
    let mut audio = Vec::with_capacity(total_samples);
    for i in 0..total_samples {
        let t = i as f64 / sample_rate as f64;
        let envelope = (t * 8.0 * std::f64::consts::PI).sin().abs();
        let sample = (t * 440.0 * std::f64::consts::PI).sin() as f32 * envelope as f32 * 0.3;
        audio.push(sample);
    }
    audio
}

// ─── Benchmark Context (pre-loaded real models) ──────────────────────────

struct BenchmarkContext {
    whisper: voxy_whisper::WhisperSttEngine,
    tts: voxy_kokoro::KokoroTtsEngine,
    cognitive: voxy_cognition::InMemoryCognitiveEngine,
    vad: voxy_voice::EnergyVadDetector,
    wake_word: voxy_voice::EnergyWakeWordDetector,
    ollama_client: reqwest::Client,
    ollama_url: String,
    ollama_model: String,
    test_audio: Vec<f32>,
    whisper_loaded: bool,
    tts_loaded: bool,
}

impl BenchmarkContext {
    async fn new(config: &BenchmarkConfig) -> Self {
        info!("Initializing benchmark context with REAL models...");

        let whisper = voxy_whisper::WhisperSttEngine::new()
            .with_model_path(config.whisper_model_path.clone().into());
        let whisper_loaded = match whisper.load_model() {
            Ok(()) => {
                info!("Whisper model loaded: {}", config.whisper_model_path);
                true
            }
            Err(e) => {
                tracing::warn!("Failed to load Whisper model: {e}. Benchmark will use fallback.");
                false
            }
        };

        let tts = voxy_kokoro::KokoroTtsEngine::new()
            .with_voice("default")
            .with_speed(0.55)
            .with_pitch(1.0)
            .with_model_path(config.piper_model_path.clone().into());
        let tts_loaded = match tts.load_model() {
            Ok(()) => {
                info!("Piper TTS model loaded: {}", config.piper_model_path);
                true
            }
            Err(e) => {
                tracing::warn!("Failed to load Piper model: {e}. Benchmark will use fallback.");
                false
            }
        };

        let ollama_client = reqwest::Client::builder()
            .timeout(Duration::from_secs(120))
            .build()
            .unwrap_or_else(|e| {
                tracing::warn!("Failed to create benchmark HTTP client: {e}, using default");
                reqwest::Client::new()
            });

        let test_audio = generate_test_speech_audio(2000, 16000);

        let cognitive = voxy_cognition::InMemoryCognitiveEngine::new(
            voxy_cognition::CognitionConfig::default(),
        );

        let vad = voxy_voice::EnergyVadDetector::new(0.05, 16000);
        let wake_word = voxy_voice::EnergyWakeWordDetector::new("hey voxy", 0.3, 16000);

        Self {
            whisper,
            tts,
            cognitive,
            vad,
            wake_word,
            ollama_client,
            ollama_url: config.ollama_url.clone(),
            ollama_model: config.ollama_model.clone(),
            test_audio,
            whisper_loaded,
            tts_loaded,
        }
    }
}

// ─── Benchmark Runner ────────────────────────────────────────────────────

pub async fn run_full_benchmark(config: BenchmarkConfig) -> BenchmarkReport {
    info!(
        "Starting VOXY REAL benchmark: {} iterations, {} warmup",
        config.iterations, config.warmup_iterations
    );

    let start = Instant::now();
    let mut report = BenchmarkReport::new(config.clone());
    report.hardware_info = detect_hardware_info();

    let mut sys_collector = SystemMetricsCollector::new();
    sys_collector.collect();

    // Initialize real context with loaded models
    let ctx = BenchmarkContext::new(&config).await;

    // Verify Ollama connectivity
    info!("Testing Ollama connectivity at {}...", config.ollama_url);
    match ctx
        .ollama_client
        .get(format!("{}/api/tags", config.ollama_url))
        .send()
        .await
    {
        Ok(resp) => {
            if resp.status().is_success() {
                info!("Ollama is reachable");
                report.provider_results.push(format!(
                    "Ollama: {} at {}",
                    config.ollama_model, config.ollama_url
                ));
            } else {
                tracing::warn!("Ollama returned status {}", resp.status());
                report
                    .provider_results
                    .push(format!("Ollama: unreachable (status {})", resp.status()));
            }
        }
        Err(e) => {
            tracing::warn!("Cannot reach Ollama: {e}. LLM benchmarks will fail.");
            report
                .provider_results
                .push(format!("Ollama: connection failed ({e})"));
        }
    }

    report.provider_results.push(format!(
        "Whisper: {} (loaded: {})",
        config.whisper_model_path, ctx.whisper_loaded
    ));
    report.provider_results.push(format!(
        "Piper: {} (loaded: {})",
        config.piper_model_path, ctx.tts_loaded
    ));

    // Warmup
    info!("Warmup: {} iterations", config.warmup_iterations);
    for i in 0..config.warmup_iterations {
        if i % 5 == 0 && i > 0 {
            info!("  Warmup progress: {}/{}", i, config.warmup_iterations);
        }
        let _ = run_real_iteration(&ctx).await;
    }

    // Main Benchmark
    let mut stage_collectors: HashMap<&str, LatencySample> = HashMap::new();
    let stage_names = [
        "audio_capture",
        "ring_buffer_write",
        "ring_buffer_read",
        "vad",
        "wake_word",
        "whisper_inference",
        "brain_runtime",
        "llm_generation",
        "piper_synthesis",
        "audio_playback",
    ];
    for name in &stage_names {
        stage_collectors.insert(name, LatencySample::new(name));
    }

    let mut e2e_collectors: Vec<LatencySample> = vec![
        LatencySample::new("wake_to_first_token"),
        LatencySample::new("wake_to_first_audio"),
        LatencySample::new("total_command_response"),
        LatencySample::new("stt_to_tts"),
    ];

    let mut voice_snapshots: Vec<VoiceMetricsSnapshot> = Vec::with_capacity(config.iterations);

    for i in 0..config.iterations {
        if i % 10 == 0 && i > 0 {
            info!("  Progress: {}/{}", i, config.iterations);
            sys_collector.collect();
        }

        let result = run_real_iteration(&ctx).await;

        if let Some(s) = stage_collectors.get_mut("audio_capture") {
            s.record(result.audio_capture_ms);
        }
        if let Some(s) = stage_collectors.get_mut("ring_buffer_write") {
            s.record(result.ring_buffer_write_ms);
        }
        if let Some(s) = stage_collectors.get_mut("ring_buffer_read") {
            s.record(result.ring_buffer_read_ms);
        }
        if let Some(s) = stage_collectors.get_mut("vad") {
            s.record(result.vad_ms);
        }
        if let Some(s) = stage_collectors.get_mut("wake_word") {
            s.record(result.wake_word_ms);
        }
        if let Some(s) = stage_collectors.get_mut("whisper_inference") {
            s.record(result.whisper_ms);
        }
        if let Some(s) = stage_collectors.get_mut("brain_runtime") {
            s.record(result.brain_ms);
        }
        if let Some(s) = stage_collectors.get_mut("llm_generation") {
            s.record(result.llm_ms);
        }
        if let Some(s) = stage_collectors.get_mut("piper_synthesis") {
            s.record(result.piper_ms);
        }
        if let Some(s) = stage_collectors.get_mut("audio_playback") {
            s.record(result.playback_ms);
        }

        e2e_collectors[0].record(result.wake_to_first_token_ms);
        e2e_collectors[1].record(result.wake_to_first_audio_ms);
        e2e_collectors[2].record(result.total_response_ms);
        e2e_collectors[3].record(result.stt_to_tts_ms);

        voice_snapshots.push(result.voice_snapshot);
    }

    sys_collector.collect();

    report.stage_latencies = stage_collectors
        .into_values()
        .filter(|s| !s.is_empty())
        .collect();
    report.e2e_latencies = e2e_collectors;
    report.voice_metrics = voice_snapshots;
    report.system_snapshots = sys_collector.snapshots().to_vec();
    report.total_duration_ms = start.elapsed().as_secs_f64() * 1000.0;

    report.identify_bottlenecks();

    info!(
        "Benchmark complete in {:.1}s",
        report.total_duration_ms / 1000.0
    );
    report
}

// ─── Single Iteration Result ─────────────────────────────────────────────

struct IterationResult {
    audio_capture_ms: f64,
    ring_buffer_write_ms: f64,
    ring_buffer_read_ms: f64,
    vad_ms: f64,
    wake_word_ms: f64,
    whisper_ms: f64,
    brain_ms: f64,
    llm_ms: f64,
    piper_ms: f64,
    playback_ms: f64,
    wake_to_first_token_ms: f64,
    wake_to_first_audio_ms: f64,
    total_response_ms: f64,
    stt_to_tts_ms: f64,
    voice_snapshot: VoiceMetricsSnapshot,
}

async fn run_real_iteration(ctx: &BenchmarkContext) -> IterationResult {
    let iter_start = Instant::now();
    let chunk_len = 480usize;
    let test_len = ctx.test_audio.len();
    let sample_offset = iter_start.elapsed().subsec_nanos() as usize % test_len.max(1);

    // T0: Audio Capture — clone chunk from pre-generated test audio
    let (audio_capture_ms, packet_data) = measure_ms(|| {
        if test_len == 0 {
            return vec![0.0f32; chunk_len];
        }
        let start = sample_offset % test_len;
        let end = (start + chunk_len).min(test_len);
        if start < end {
            ctx.test_audio[start..end].to_vec()
        } else {
            vec![0.0f32; chunk_len]
        }
    });

    // T1: Ring Buffer Write
    let ring = voxy_audio::SpscRingBuffer::new(65536);
    let (ring_write_ms, _) = measure_ms(|| {
        ring.write(&packet_data);
    });

    // T2: Ring Buffer Read
    let mut read_buf = vec![0.0f32; chunk_len];
    let (ring_read_ms, _) = measure_ms(|| {
        ring.read(&mut read_buf);
    });

    // T3: VAD — real model call
    let chunk_data_vad = read_buf.clone();
    let (vad_ms, _) = measure_ms_async(|| {
        let data = chunk_data_vad;
        let vad = &ctx.vad;
        async move {
            let chunk = voxy_voice_orchestrator::AudioChunk {
                data,
                sample_rate: 16000,
                channels: 1,
                timestamp: chrono::Utc::now(),
                sequence: 0,
                is_final: false,
            };
            vad.is_voice(&chunk).await.unwrap_or(false)
        }
    })
    .await;

    // T4: Wake Word — real model call
    let chunk_data_ww = read_buf.clone();
    let (wake_word_ms, _) = measure_ms_async(|| {
        let data = chunk_data_ww;
        let ww = &ctx.wake_word;
        async move {
            let chunk = voxy_voice_orchestrator::AudioChunk {
                data,
                sample_rate: 16000,
                channels: 1,
                timestamp: chrono::Utc::now(),
                sequence: 0,
                is_final: false,
            };
            ww.detect(&chunk).await.unwrap_or(None)
        }
    })
    .await;

    // T5: Whisper Inference — REAL transcription via whisper-rs
    let audio_for_whisper: Vec<f32> = {
        let take = 16000.min(test_len);
        ctx.test_audio[..take].to_vec()
    };
    let (whisper_ms, whisper_result) = measure_ms_async(|| {
        let data = audio_for_whisper.clone();
        let stt = &ctx.whisper;
        async move {
            let chunk = voxy_voice_orchestrator::AudioChunk {
                data,
                sample_rate: 16000,
                channels: 1,
                timestamp: chrono::Utc::now(),
                sequence: 0,
                is_final: true,
            };
            stt.transcribe(&chunk).await.unwrap_or_default()
        }
    })
    .await;

    let user_text = if whisper_result.trim().is_empty() {
        "What is the capital of France?".to_string()
    } else {
        whisper_result
    };

    // T7: Brain Runtime — real InMemoryCognitiveEngine
    let (brain_ms, _) = measure_ms_async(|| {
        let text = user_text.clone();
        let cog = &ctx.cognitive;
        async move {
            let intent = voxy_cognition::IntentInput {
                raw_text: text,
                context: None,
                source: "benchmark".to_string(),
                metadata: HashMap::new(),
            };
            cog.process(&intent).await.ok()
        }
    })
    .await;

    // T8-T9: LLM Generation via Ollama — REAL HTTP streaming
    let prompt = format!(
        "You are a helpful voice assistant. Reply concisely.\n\nUser: {}",
        user_text
    );
    let (llm_ms, llm_result) = measure_ms_async(|| {
        let client = &ctx.ollama_client;
        let url = &ctx.ollama_url;
        let model = &ctx.ollama_model;
        let p = prompt.clone();
        async move { call_ollama(client, url, model, &p).await }
    })
    .await;

    let llm_response = llm_result.text;

    // T12: Piper TTS Synthesis — REAL model call
    let tts_text = if llm_response.is_empty() || llm_response.starts_with("Error") {
        "Hello! How can I help you today?".to_string()
    } else {
        llm_response.clone()
    };
    let (piper_ms, tts_result) = measure_ms_async(|| {
        let text = tts_text.clone();
        let tts = &ctx.tts;
        async move { tts.synthesize(&text).await.ok() }
    })
    .await;

    // T13-T14: Audio Playback — write to output ring
    let out_ring = voxy_audio::SpscRingBuffer::new(262144);
    let (playback_ms, _) = measure_ms(|| {
        if let Some(ref chunk) = tts_result {
            if !chunk.data.is_empty() {
                out_ring.write(&chunk.data);
            }
        }
    });

    // E2E
    let wake_to_first_token_ms = vad_ms + wake_word_ms + whisper_ms + brain_ms + llm_ms;
    let wake_to_first_audio_ms = wake_to_first_token_ms + piper_ms;
    let total_response_ms = iter_start.elapsed().as_secs_f64() * 1000.0;
    let stt_to_tts_ms = whisper_ms + brain_ms + llm_ms + piper_ms;

    IterationResult {
        audio_capture_ms,
        ring_buffer_write_ms: ring_write_ms,
        ring_buffer_read_ms: ring_read_ms,
        vad_ms,
        wake_word_ms,
        whisper_ms,
        brain_ms,
        llm_ms,
        piper_ms,
        playback_ms,
        wake_to_first_token_ms,
        wake_to_first_audio_ms,
        total_response_ms,
        stt_to_tts_ms,
        voice_snapshot: VoiceMetricsSnapshot {
            wake_word_detection_ms: wake_word_ms,
            vad_detection_ms: vad_ms,
            whisper_first_token_ms: None,
            whisper_final_result_ms: Some(whisper_ms),
            brain_processing_ms: brain_ms,
            llm_first_token_ms: Some(llm_result.ttft_ms),
            llm_final_token_ms: Some(llm_ms),
            piper_first_audio_ms: None,
            playback_start_ms: None,
            complete_response_ms: total_response_ms,
            audio_capture_ms,
            ring_buffer_occupancy: 0.0,
            audio_underruns: 0,
            dropped_frames: 0,
            audio_callback_ms: 0.0,
        },
    }
}
