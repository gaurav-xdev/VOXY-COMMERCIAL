use std::sync::atomic::{AtomicU64, Ordering};
use serde::{Deserialize, Serialize};

static ID_COUNTER: AtomicU64 = AtomicU64::new(1);

pub fn next_frame_id() -> u64 {
    ID_COUNTER.fetch_add(1, Ordering::Relaxed)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameDirection {
    Downstream,
    Upstream,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrameBase {
    pub id: u64,
    pub pts: Option<i64>,
}

impl Default for FrameBase {
    fn default() -> Self {
        Self {
            id: next_frame_id(),
            pts: None,
        }
    }
}

impl FrameBase {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_id(id: u64) -> Self {
        Self { id, pts: None }
    }
}

/// Core pipeline frame types matching upstream Voxray specifications.
#[derive(Debug, Clone)]
pub enum Frame {
    // System Frames
    Start(StartFrame),
    Cancel(CancelFrame),
    End(EndFrame),
    Stop(StopFrame),
    Error(ErrorFrame),

    // Data Frames
    AudioRaw(AudioRawFrame),
    Transcription(TranscriptionFrame),
    LLMText(LLMTextFrame),
    TTSSpeak(TTSSpeakFrame),
    TTSAudioRaw(TTSAudioRawFrame),

    // Control & Event Frames
    UserStartedSpeaking(UserStartedSpeakingFrame),
    UserStoppedSpeaking(UserStoppedSpeakingFrame),
    BotStartedSpeaking(BotStartedSpeakingFrame),
    BotStoppedSpeaking(BotStoppedSpeakingFrame),
    Interruption(InterruptionFrame),
    VADParamsUpdate(VADParamsUpdateFrame),
}

impl Frame {
    pub fn frame_type(&self) -> &'static str {
        match self {
            Frame::Start(_) => "StartFrame",
            Frame::Cancel(_) => "CancelFrame",
            Frame::End(_) => "EndFrame",
            Frame::Stop(_) => "StopFrame",
            Frame::Error(_) => "ErrorFrame",
            Frame::AudioRaw(_) => "AudioRawFrame",
            Frame::Transcription(_) => "TranscriptionFrame",
            Frame::LLMText(_) => "LLMTextFrame",
            Frame::TTSSpeak(_) => "TTSSpeakFrame",
            Frame::TTSAudioRaw(_) => "TTSAudioRawFrame",
            Frame::UserStartedSpeaking(_) => "UserStartedSpeakingFrame",
            Frame::UserStoppedSpeaking(_) => "UserStoppedSpeakingFrame",
            Frame::BotStartedSpeaking(_) => "BotStartedSpeakingFrame",
            Frame::BotStoppedSpeaking(_) => "BotStoppedSpeakingFrame",
            Frame::Interruption(_) => "InterruptionFrame",
            Frame::VADParamsUpdate(_) => "VADParamsUpdateFrame",
        }
    }

    pub fn id(&self) -> u64 {
        match self {
            Frame::Start(f) => f.base.id,
            Frame::Cancel(f) => f.base.id,
            Frame::End(f) => f.base.id,
            Frame::Stop(f) => f.base.id,
            Frame::Error(f) => f.base.id,
            Frame::AudioRaw(f) => f.base.id,
            Frame::Transcription(f) => f.base.id,
            Frame::LLMText(f) => f.base.id,
            Frame::TTSSpeak(f) => f.base.id,
            Frame::TTSAudioRaw(f) => f.base.id,
            Frame::UserStartedSpeaking(f) => f.base.id,
            Frame::UserStoppedSpeaking(f) => f.base.id,
            Frame::BotStartedSpeaking(f) => f.base.id,
            Frame::BotStoppedSpeaking(f) => f.base.id,
            Frame::Interruption(f) => f.base.id,
            Frame::VADParamsUpdate(f) => f.base.id,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StartFrame {
    pub base: FrameBase,
    pub audio_in_sample_rate: u32,
    pub audio_out_sample_rate: u32,
    pub allow_interruptions: bool,
    pub enable_metrics: bool,
}

impl Default for StartFrame {
    fn default() -> Self {
        Self {
            base: FrameBase::new(),
            audio_in_sample_rate: 16000,
            audio_out_sample_rate: 24000,
            allow_interruptions: true,
            enable_metrics: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CancelFrame {
    pub base: FrameBase,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EndFrame {
    pub base: FrameBase,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StopFrame {
    pub base: FrameBase,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorFrame {
    pub base: FrameBase,
    pub error: String,
    pub fatal: bool,
    pub processor: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AudioRawFrame {
    pub base: FrameBase,
    pub audio: Vec<u8>,
    pub sample_rate: u32,
    pub num_channels: u16,
    pub num_frames: usize,
}

impl AudioRawFrame {
    pub fn new(audio: Vec<u8>, sample_rate: u32, num_channels: u16) -> Self {
        let num_frames = if num_channels > 0 {
            audio.len() / (num_channels as usize * 2)
        } else {
            0
        };
        Self {
            base: FrameBase::new(),
            audio,
            sample_rate,
            num_channels,
            num_frames,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptionFrame {
    pub base: FrameBase,
    pub text: String,
    pub user_id: String,
    pub timestamp: String,
    pub language: Option<String>,
    pub finalized: bool,
}

impl TranscriptionFrame {
    pub fn new(text: impl Into<String>, finalized: bool) -> Self {
        Self {
            base: FrameBase::new(),
            text: text.into(),
            user_id: "user".to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            language: None,
            finalized,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LLMTextFrame {
    pub base: FrameBase,
    pub text: String,
    pub append_to_context: bool,
}

impl LLMTextFrame {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            base: FrameBase::new(),
            text: text.into(),
            append_to_context: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TTSSpeakFrame {
    pub base: FrameBase,
    pub text: String,
}

impl TTSSpeakFrame {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            base: FrameBase::new(),
            text: text.into(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct TTSAudioRawFrame {
    pub base: FrameBase,
    pub audio: Vec<u8>,
    pub sample_rate: u32,
    pub num_channels: u16,
}

impl TTSAudioRawFrame {
    pub fn new(audio: Vec<u8>, sample_rate: u32, num_channels: u16) -> Self {
        Self {
            base: FrameBase::new(),
            audio,
            sample_rate,
            num_channels,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserStartedSpeakingFrame {
    pub base: FrameBase,
}

impl Default for UserStartedSpeakingFrame {
    fn default() -> Self {
        Self {
            base: FrameBase::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserStoppedSpeakingFrame {
    pub base: FrameBase,
}

impl Default for UserStoppedSpeakingFrame {
    fn default() -> Self {
        Self {
            base: FrameBase::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BotStartedSpeakingFrame {
    pub base: FrameBase,
}

impl Default for BotStartedSpeakingFrame {
    fn default() -> Self {
        Self {
            base: FrameBase::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BotStoppedSpeakingFrame {
    pub base: FrameBase,
}

impl Default for BotStoppedSpeakingFrame {
    fn default() -> Self {
        Self {
            base: FrameBase::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterruptionFrame {
    pub base: FrameBase,
}

impl Default for InterruptionFrame {
    fn default() -> Self {
        Self {
            base: FrameBase::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VADParamsUpdateFrame {
    pub base: FrameBase,
    pub threshold: Option<f32>,
    pub stop_secs: Option<f64>,
}
