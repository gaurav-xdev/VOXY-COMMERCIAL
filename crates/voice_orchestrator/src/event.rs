use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct InterruptionSignal {
    pub reason: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub cancelled_tokens: bool,
    pub cleared_audio_frames: usize,
}

impl InterruptionSignal {
    pub fn new(reason: impl Into<String>) -> Self {
        Self {
            reason: reason.into(),
            timestamp: chrono::Utc::now(),
            cancelled_tokens: true,
            cleared_audio_frames: 0,
        }
    }

    pub fn with_cancelled_tokens(mut self, cancelled: bool) -> Self {
        self.cancelled_tokens = cancelled;
        self
    }

    pub fn with_cleared_frames(mut self, frames: usize) -> Self {
        self.cleared_audio_frames = frames;
        self
    }

    pub fn with_cleared_audio_frames(mut self, frames: usize) -> Self {
        self.cleared_audio_frames = frames;
        self
    }
}

#[derive(Clone)]
pub enum VoiceEvent {
    WakeWordDetected {
        confidence: f32,
    },
    VoiceActivityStarted,
    VoiceActivityEnded {
        duration_ms: u64,
    },
    TranscriptionResult {
        text: String,
        is_final: bool,
        confidence: f32,
    },
    TranscriptionError {
        error: String,
    },
    SynthesisStarted {
        text: String,
    },
    SynthesisCompleted {
        duration_ms: u64,
    },
    SynthesisError {
        error: String,
    },
    PipelineStateChanged {
        state: String,
    },
    Interrupted {
        signal: InterruptionSignal,
    },
}

impl fmt::Display for VoiceEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WakeWordDetected { confidence } => {
                write!(f, "Wake word detected (confidence: {:.2})", confidence)
            }
            Self::VoiceActivityStarted => write!(f, "Voice activity started"),
            Self::VoiceActivityEnded { duration_ms } => {
                write!(f, "Voice activity ended (duration: {}ms)", duration_ms)
            }
            Self::TranscriptionResult {
                text,
                is_final,
                confidence,
            } => {
                write!(
                    f,
                    "Transcription: \"{}\" (final: {}, confidence: {:.2})",
                    text, is_final, confidence
                )
            }
            Self::TranscriptionError { error } => {
                write!(f, "Transcription error: {}", error)
            }
            Self::SynthesisStarted { text } => {
                write!(f, "Synthesis started: \"{}\"", text)
            }
            Self::SynthesisCompleted { duration_ms } => {
                write!(f, "Synthesis completed ({}ms)", duration_ms)
            }
            Self::SynthesisError { error } => {
                write!(f, "Synthesis error: {}", error)
            }
            Self::PipelineStateChanged { state } => {
                write!(f, "Pipeline state changed: {}", state)
            }
            Self::Interrupted { signal } => {
                write!(f, "Interrupted: {} (cleared {} frames)", signal.reason, signal.cleared_audio_frames)
            }
        }
    }
}

impl std::fmt::Debug for VoiceEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::WakeWordDetected { confidence } => f
                .debug_struct("WakeWordDetected")
                .field("confidence", confidence)
                .finish(),
            Self::VoiceActivityStarted => f.debug_struct("VoiceActivityStarted").finish(),
            Self::VoiceActivityEnded { duration_ms } => f
                .debug_struct("VoiceActivityEnded")
                .field("duration_ms", duration_ms)
                .finish(),
            Self::TranscriptionResult {
                text,
                is_final,
                confidence,
            } => f
                .debug_struct("TranscriptionResult")
                .field("text", text)
                .field("is_final", is_final)
                .field("confidence", confidence)
                .finish(),
            Self::TranscriptionError { error } => f
                .debug_struct("TranscriptionError")
                .field("error", error)
                .finish(),
            Self::SynthesisStarted { text } => f
                .debug_struct("SynthesisStarted")
                .field("text", text)
                .finish(),
            Self::SynthesisCompleted { duration_ms } => f
                .debug_struct("SynthesisCompleted")
                .field("duration_ms", duration_ms)
                .finish(),
            Self::SynthesisError { error } => f
                .debug_struct("SynthesisError")
                .field("error", error)
                .finish(),
            Self::PipelineStateChanged { state } => f
                .debug_struct("PipelineStateChanged")
                .field("state", state)
                .finish(),
            Self::Interrupted { signal } => f
                .debug_struct("Interrupted")
                .field("signal", signal)
                .finish(),
        }
    }
}
