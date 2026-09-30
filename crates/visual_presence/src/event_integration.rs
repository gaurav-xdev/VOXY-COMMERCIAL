use crate::audio_reactive::AudioReactive;
use crate::config::VisualPresenceConfig;
use crate::presence_renderer::{PresenceRenderer, PresenceState};
use crate::spatial_motion::SpatialMotion;
use glam::Vec3;
use tokio::sync::broadcast;

#[derive(Clone, Debug)]
pub enum ExperienceOutput {
    PresenceChanged(String),
    MoodChanged(String),
    VoiceParamsUpdated { speed: f32, pitch: f32, volume: f32 },
    DesktopFocusChanged(String),
    DesktopActivityChanged(String),
    DesktopIdle,
    Shutdown,
}

pub struct EventIntegration {
    renderer: PresenceRenderer,
    spatial_motion: SpatialMotion,
    audio_reactive: AudioReactive,
    config: VisualPresenceConfig,
    rx: broadcast::Receiver<ExperienceOutput>,
    mouse_pos: Vec3,
    is_speaking: bool,
    is_thinking: bool,
}

impl EventIntegration {
    pub fn new(
        renderer: PresenceRenderer,
        spatial_motion: SpatialMotion,
        audio_reactive: AudioReactive,
        config: VisualPresenceConfig,
        rx: broadcast::Receiver<ExperienceOutput>,
    ) -> Self {
        Self {
            renderer,
            spatial_motion,
            audio_reactive,
            config,
            rx,
            mouse_pos: Vec3::ZERO,
            is_speaking: false,
            is_thinking: false,
        }
    }

    pub fn update(&mut self, delta_time: f32, rms: f32) {
        while let Ok(msg) = self.rx.try_recv() {
            match msg {
                ExperienceOutput::PresenceChanged(presence) => {
                    let state = match presence.as_str() {
                        "idle" => PresenceState::Idle,
                        "listening" => PresenceState::Listening,
                        "thinking" => PresenceState::Thinking,
                        "speaking" => PresenceState::Speaking,
                        "error" => PresenceState::Error,
                        _ => PresenceState::Idle,
                    };
                    self.renderer.set_state(state);
                }
                ExperienceOutput::MoodChanged(mood) => {
                    tracing::debug!("Mood changed: {}", mood);
                }
                ExperienceOutput::VoiceParamsUpdated { speed, .. } => {
                    self.is_speaking = speed > 0.0;
                }
                ExperienceOutput::DesktopFocusChanged(app) => {
                    tracing::debug!("Focus changed: {}", app);
                }
                ExperienceOutput::DesktopActivityChanged(activity) => {
                    tracing::debug!("Activity changed: {}", activity);
                }
                ExperienceOutput::DesktopIdle => {
                    self.renderer.set_state(PresenceState::Idle);
                }
                ExperienceOutput::Shutdown => {
                    tracing::info!("Shutdown received");
                }
            }
        }

        self.audio_reactive.update_rms(rms);
        self.audio_reactive.update(delta_time);
        self.spatial_motion
            .update(delta_time, self.is_speaking, self.is_thinking);
        self.renderer
            .update(delta_time, Some(self.spatial_motion.position()));
    }

    pub fn set_mouse_pos(&mut self, pos: Vec3) {
        self.mouse_pos = pos;
        self.spatial_motion.update_cursor(pos);
    }

    pub fn set_audio_rms(&mut self, rms: f32) {
        self.audio_reactive.update_rms(rms);
    }

    pub fn renderer(&self) -> &PresenceRenderer {
        &self.renderer
    }

    pub fn spatial_motion(&self) -> &SpatialMotion {
        &self.spatial_motion
    }

    pub fn audio_reactive(&self) -> &AudioReactive {
        &self.audio_reactive
    }

    pub fn config(&self) -> &VisualPresenceConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_experience_output_variants() {
        let outputs = [
            ExperienceOutput::PresenceChanged("idle".to_string()),
            ExperienceOutput::MoodChanged("happy".to_string()),
            ExperienceOutput::VoiceParamsUpdated {
                speed: 1.0,
                pitch: 1.0,
                volume: 0.8,
            },
            ExperienceOutput::DesktopFocusChanged("notepad".to_string()),
            ExperienceOutput::DesktopActivityChanged("typing".to_string()),
            ExperienceOutput::DesktopIdle,
            ExperienceOutput::Shutdown,
        ];
        assert_eq!(outputs.len(), 7);
    }
}
