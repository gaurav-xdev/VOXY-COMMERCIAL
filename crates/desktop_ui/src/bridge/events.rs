use std::sync::Arc;

use voxy_event_bus::EventBus;
use voxy_shared::Event;

#[derive(Clone)]
pub struct EventBridge {
    bus: Arc<EventBus>,
}

impl EventBridge {
    pub fn new(bus: Arc<EventBus>) -> Self {
        Self { bus }
    }

    pub async fn subscribe(
        &self,
        topic: &str,
    ) -> Result<tokio::sync::broadcast::Receiver<Event>, String> {
        self.bus
            .subscribe(topic)
            .await
            .map_err(|e| format!("Subscribe failed: {}", e))
    }

    pub async fn publish(&self, topic: &str, event: Event) -> Result<(), String> {
        self.bus
            .publish(topic, event)
            .await
            .map_err(|e| format!("Publish failed: {}", e))
    }

    pub async fn topic_count(&self) -> usize {
        self.bus.topic_count().await
    }

    pub async fn topic_names(&self) -> Vec<String> {
        self.bus.topic_names().await
    }
}
