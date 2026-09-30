//! Windows SAPI Emergency Local Fallback STT provider.
//! Operates completely offline with 0 VRAM and 0 GPU overhead.
//! Used when cloud STT providers are unreachable, exhausted, or in strict local offline mode.

use async_trait::async_trait;
use base64::Engine;
use std::process::Command;
use tracing::{debug, warn};

use crate::provider::traits::{
    AudioData, EstimatedCost, ProviderError, STTCapabilities, STTProvider, Transcript,
    VoiceLanguage,
};

pub struct LocalSapiSTTProvider {
    capabilities: STTCapabilities,
}

impl LocalSapiSTTProvider {
    pub fn new() -> Self {
        Self {
            capabilities: STTCapabilities {
                streaming: false,
                word_timestamps: false,
                supported_languages: vec![VoiceLanguage::English, VoiceLanguage::AutoDetect],
                multilingual: false,
                custom_vocab: false,
                max_audio_duration_secs: 60,
                typical_latency_ms: 250,
                cost_per_second_usd: 0.0,
            },
        }
    }
}

impl Default for LocalSapiSTTProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl STTProvider for LocalSapiSTTProvider {
    fn id(&self) -> &'static str {
        "local-sapi-stt"
    }

    fn name(&self) -> &'static str {
        "Windows SAPI Emergency Local Fallback STT"
    }

    fn capabilities(&self) -> &STTCapabilities {
        &self.capabilities
    }

    async fn transcribe(
        &self,
        audio: AudioData,
        _language_hint: Option<VoiceLanguage>,
    ) -> Result<Transcript, ProviderError> {
        if audio.pcm_bytes.is_empty() {
            return Ok(Transcript {
                text: String::new(),
                confidence: Some(1.0),
                language: Some(VoiceLanguage::English),
                latency_ms: 0,
                is_final: true,
                provider: self.id().to_string(),
                duration_secs: 0.0,
                words: None,
            });
        }

        let temp_dir = std::env::temp_dir();
        let file_name = format!(
            "voxy_sapi_stt_{}_{}.wav",
            std::process::id(),
            uuid::Uuid::new_v4()
        );
        let wav_path = temp_dir.join(file_name);
        let wav_path_str = wav_path.to_string_lossy().to_string();

        let wav_bytes = audio.to_wav();
        std::fs::write(&wav_path, &wav_bytes).map_err(|e| {
            ProviderError::TranscriptionFailed(format!("Failed to write temp WAV file: {}", e))
        })?;

        let b64_path = base64::engine::general_purpose::STANDARD.encode(wav_path_str.as_bytes());

        debug!(
            provider = self.id(),
            "Transcribing speech via Windows SAPI local fallback"
        );
        let start = std::time::Instant::now();

        let script = format!(
            "Add-Type -AssemblyName System.Speech; \
             $p = [System.Text.Encoding]::UTF8.GetString([System.Convert]::FromBase64String('{}')); \
             $sre = New-Object System.Speech.Recognition.SpeechRecognitionEngine; \
             $g = New-Object System.Speech.Recognition.DictationGrammar; \
             $sre.LoadGrammar($g); \
             $sre.SetInputToWaveFile($p); \
             $res = $sre.Recognize(); \
             if ($res) {{ Write-Output $res.Text }}; \
             $sre.Dispose();",
            b64_path
        );

        let output_res = tokio::task::spawn_blocking(move || {
            Command::new("powershell")
                .args(["-NoProfile", "-NonInteractive", "-Command", &script])
                .output()
        })
        .await;

        // Cleanup temporary WAV file regardless of success or failure
        let _ = std::fs::remove_file(&wav_path);

        let output = output_res
            .map_err(|e| {
                ProviderError::TranscriptionFailed(format!(
                    "Failed to spawn PowerShell task: {}",
                    e
                ))
            })?
            .map_err(|e| {
                ProviderError::TranscriptionFailed(format!(
                    "Failed to execute Windows SAPI STT: {}",
                    e
                ))
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            warn!(
                provider = self.id(),
                stderr = %stderr,
                "Windows SAPI STT execution returned non-zero exit code"
            );
            return Err(ProviderError::TranscriptionFailed(format!(
                "Windows SAPI STT failed: {}",
                stderr.trim()
            )));
        }

        let text = String::from_utf8_lossy(&output.stdout).trim().to_string();

        Ok(Transcript {
            text,
            confidence: Some(0.9),
            language: Some(VoiceLanguage::English),
            latency_ms: start.elapsed().as_millis() as u64,
            is_final: true,
            provider: self.id().to_string(),
            duration_secs: audio.duration_secs,
            words: None,
        })
    }

    fn estimate_cost(&self, audio_duration_secs: f64) -> EstimatedCost {
        EstimatedCost {
            amount_usd: 0.0,
            currency: "USD".into(),
            estimated_cost_usd: 0.0,
            billable_units: audio_duration_secs,
            unit_type: "seconds".into(),
        }
    }
}
