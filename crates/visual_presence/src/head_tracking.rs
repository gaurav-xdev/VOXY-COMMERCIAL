use crate::config::HeadTrackingConfig;

pub struct HeadTracker {
    config: HeadTrackingConfig,
    head_position: [f32; 3],
    head_rotation: [f32; 3],
    confidence: f32,
    is_tracking: bool,
}

impl HeadTracker {
    pub fn new(config: HeadTrackingConfig) -> Self {
        Self {
            config,
            head_position: [0.0; 3],
            head_rotation: [0.0; 3],
            confidence: 0.0,
            is_tracking: false,
        }
    }

    pub fn start(&mut self) -> Result<(), String> {
        if !self.config.enabled {
            return Err("Head tracking is disabled".to_string());
        }
        self.is_tracking = true;
        Ok(())
    }

    pub fn stop(&mut self) {
        self.is_tracking = false;
    }

    pub fn update(&mut self, position: [f32; 3], rotation: [f32; 3], confidence: f32) {
        if !self.is_tracking {
            return;
        }

        if confidence >= self.config.tracking_confidence {
            let smoothing = self.config.smoothing;
            for i in 0..3 {
                self.head_position[i] =
                    self.head_position[i] * smoothing + position[i] * (1.0 - smoothing);
                self.head_rotation[i] =
                    self.head_rotation[i] * smoothing + rotation[i] * (1.0 - smoothing);
            }
            self.confidence = confidence;
        }
    }

    pub fn head_position(&self) -> [f32; 3] {
        self.head_position
    }

    pub fn head_rotation(&self) -> [f32; 3] {
        self.head_rotation
    }

    pub fn confidence(&self) -> f32 {
        self.confidence
    }

    pub fn is_tracking(&self) -> bool {
        self.is_tracking
    }

    pub fn config(&self) -> &HeadTrackingConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_head_tracker_creation() {
        let config = HeadTrackingConfig::default();
        let tracker = HeadTracker::new(config);
        assert!(!tracker.is_tracking());
        assert_eq!(tracker.confidence(), 0.0);
    }

    #[test]
    fn test_head_tracker_start_stop() {
        let config = HeadTrackingConfig {
            enabled: true,
            ..Default::default()
        };
        let mut tracker = HeadTracker::new(config);
        assert!(tracker.start().is_ok());
        assert!(tracker.is_tracking());
        tracker.stop();
        assert!(!tracker.is_tracking());
    }

    #[test]
    fn test_head_tracker_disabled() {
        let config = HeadTrackingConfig {
            enabled: false,
            ..Default::default()
        };
        let mut tracker = HeadTracker::new(config);
        assert!(tracker.start().is_err());
    }

    #[test]
    fn test_head_tracker_update() {
        let config = HeadTrackingConfig {
            enabled: true,
            tracking_confidence: 0.5,
            smoothing: 0.5,
            ..Default::default()
        };
        let mut tracker = HeadTracker::new(config);
        let _ = tracker.start();
        tracker.update([1.0, 2.0, 3.0], [0.1, 0.2, 0.3], 0.8);
        assert!(tracker.confidence() > 0.0);
        assert!(tracker.head_position()[0] > 0.0);
    }
}
