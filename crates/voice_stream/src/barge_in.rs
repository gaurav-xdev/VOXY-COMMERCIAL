use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BargeInAction {
    None,
    CancelGeneration,
    CancelPlayback,
    RewindToChunk,
    HardReset,
}

pub struct BargeInState {
    pub llm_streaming: bool,
    pub pending_generates: usize,
    pub playback_active: bool,
    pub playback_elapsed_ms: u64,
}

impl BargeInState {
    pub fn idle() -> Self {
        Self {
            llm_streaming: false,
            pending_generates: 0,
            playback_active: false,
            playback_elapsed_ms: 0,
        }
    }
}

pub struct BargeInCoordinator {
    enabled: bool,
    min_tts_playback_ms: u64,
    interrupt_cooldown_ms: u64,
    last_interrupt_at: Option<Instant>,
}

impl BargeInCoordinator {
    pub fn new(enabled: bool, min_tts_playback_ms: u64, interrupt_cooldown_ms: u64) -> Self {
        Self {
            enabled,
            min_tts_playback_ms,
            interrupt_cooldown_ms,
            last_interrupt_at: None,
        }
    }

    pub fn should_barge_in(&mut self, state: &BargeInState, now: Instant) -> bool {
        if !self.enabled {
            return false;
        }
        // Nothing has been heard by the user until TTS audio actually
        // starts playing. Barge-in before first audio would cancel the
        // in-flight LLM generation and silently discard the entire
        // response (observed on real hardware: ambient noise retriggers
        // VAD during the pre-roll window, before any playback begins).
        if !state.playback_active {
            return false;
        }
        if let Some(last) = self.last_interrupt_at {
            if (now.duration_since(last).as_millis() as u64) < self.interrupt_cooldown_ms {
                return false;
            }
        }
        if state.llm_streaming || state.pending_generates > 0 {
            return true;
        }
        state.playback_elapsed_ms >= self.min_tts_playback_ms
    }

    pub fn mark_interrupt(&mut self, now: Instant) {
        self.last_interrupt_at = Some(now);
    }

    /// Reconfigure thresholds at runtime without resetting cooldown state.
    pub fn configure(
        &mut self,
        enabled: bool,
        min_tts_playback_ms: u64,
        interrupt_cooldown_ms: u64,
    ) {
        self.enabled = enabled;
        self.min_tts_playback_ms = min_tts_playback_ms;
        self.interrupt_cooldown_ms = interrupt_cooldown_ms;
    }
}

impl Default for BargeInCoordinator {
    fn default() -> Self {
        Self::new(true, 500, 200)
    }
}

pub fn rewind_target(chunk_boundaries: &[usize], fallback: usize) -> usize {
    chunk_boundaries.last().copied().unwrap_or(fallback)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn now() -> Instant {
        Instant::now()
    }

    #[test]
    fn disabled_never_barges_in() {
        let mut coord = BargeInCoordinator::new(false, 500, 200);
        let state = BargeInState {
            llm_streaming: true,
            pending_generates: 3,
            playback_active: true,
            playback_elapsed_ms: 5000,
        };
        assert!(!coord.should_barge_in(&state, now()));
    }

    #[test]
    fn llm_streaming_triggers_barge_in() {
        let mut coord = BargeInCoordinator::default();
        let state = BargeInState {
            llm_streaming: true,
            pending_generates: 0,
            playback_active: true,
            playback_elapsed_ms: 0,
        };
        assert!(coord.should_barge_in(&state, now()));
    }

    #[test]
    fn pending_generates_trigger_barge_in() {
        let mut coord = BargeInCoordinator::default();
        let state = BargeInState {
            llm_streaming: false,
            pending_generates: 2,
            playback_active: true,
            playback_elapsed_ms: 0,
        };
        assert!(coord.should_barge_in(&state, now()));
    }

    #[test]
    fn pre_playback_never_barges_in() {
        let mut coord = BargeInCoordinator::default();
        let state = BargeInState {
            llm_streaming: true,
            pending_generates: 2,
            playback_active: false,
            playback_elapsed_ms: 0,
        };
        assert!(!coord.should_barge_in(&state, now()));
    }

    #[test]
    fn playback_before_min_elapsed_does_not_barge_in() {
        let mut coord = BargeInCoordinator::default();
        let state = BargeInState {
            llm_streaming: false,
            pending_generates: 0,
            playback_active: true,
            playback_elapsed_ms: 100,
        };
        assert!(!coord.should_barge_in(&state, now()));
    }

    #[test]
    fn playback_after_min_elapsed_barges_in() {
        let mut coord = BargeInCoordinator::default();
        let state = BargeInState {
            llm_streaming: false,
            pending_generates: 0,
            playback_active: true,
            playback_elapsed_ms: 600,
        };
        assert!(coord.should_barge_in(&state, now()));
    }

    #[test]
    fn cooldown_blocks_repeated_interrupts() {
        let mut coord = BargeInCoordinator::default();
        let t0 = now();
        let state = BargeInState {
            llm_streaming: true,
            pending_generates: 0,
            playback_active: true,
            playback_elapsed_ms: 0,
        };
        assert!(coord.should_barge_in(&state, t0));
        coord.mark_interrupt(t0);
        assert!(!coord.should_barge_in(&state, t0 + Duration::from_millis(100)));
        assert!(coord.should_barge_in(&state, t0 + Duration::from_millis(250)));
    }

    #[test]
    fn rewind_target_uses_last_chunk_boundary() {
        assert_eq!(rewind_target(&[5, 10, 15], 0), 15);
        assert_eq!(rewind_target(&[7], 0), 7);
        assert_eq!(rewind_target(&[], 3), 3);
    }

    #[test]
    fn configure_updates_thresholds_without_losing_cooldown() {
        let mut coord = BargeInCoordinator::default();
        let t0 = now();
        let state = BargeInState {
            llm_streaming: true,
            pending_generates: 0,
            playback_active: true,
            playback_elapsed_ms: 0,
        };
        assert!(coord.should_barge_in(&state, t0));
        coord.mark_interrupt(t0);
        coord.configure(true, 500, 5000);
        assert!(!coord.should_barge_in(&state, t0 + Duration::from_millis(1000)));
        assert!(coord.should_barge_in(&state, t0 + Duration::from_millis(6000)));
    }
}
