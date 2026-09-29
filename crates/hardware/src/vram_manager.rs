use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::RwLock;
use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EngineRole {
    SpeechToText,
    LargeLanguageModel,
    TextToSpeech,
    Vision,
}

impl EngineRole {
    pub fn default_budget_ratio(&self) -> f64 {
        match self {
            Self::LargeLanguageModel => 0.70, // 70% of available AI budget
            Self::SpeechToText => 0.18,       // 18% (e.g. Whisper ~1GB)
            Self::TextToSpeech => 0.07,       // 7% (e.g. Piper/Kokoro ~400MB)
            Self::Vision => 0.05,             // 5%
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VramAllocationInfo {
    pub role: EngineRole,
    pub model_id: String,
    pub allocated_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VramBudgetStatus {
    pub total_dedicated_vram_bytes: u64,
    pub os_reserved_bytes: u64,
    pub usable_ai_vram_bytes: u64,
    pub currently_allocated_bytes: u64,
    pub free_ai_vram_bytes: u64,
    pub active_allocations: Vec<VramAllocationInfo>,
}

#[derive(Debug, thiserror::Error)]
pub enum VramAllocationError {
    #[error("Insufficient VRAM: requested {requested_gb:.2} GB, only {available_gb:.2} GB available")]
    InsufficientMemory { requested_gb: f64, available_gb: f64 },
    #[error("Engine slot {0:?} is already active with model '{1}'")]
    SlotBusy(EngineRole, String),
}

struct AllocatedSlot {
    model_id: String,
    allocated_bytes: u64,
    last_used: Instant,
}

pub struct VramManager {
    total_vram_bytes: u64,
    os_reserve_bytes: u64,
    slots: RwLock<HashMap<EngineRole, AllocatedSlot>>,
}

impl VramManager {
    /// Creates a new VRAM budget manager.
    /// Defaults to 768MB reserved for Windows desktop, DWM, and video display.
    pub fn new(total_vram_bytes: u64) -> Self {
        let os_reserve = 768 * 1024 * 1024; // 768MB
        Self {
            total_vram_bytes,
            os_reserve_bytes: os_reserve,
            slots: RwLock::new(HashMap::new()),
        }
    }

    pub fn with_os_reserve(total_vram_bytes: u64, os_reserve_bytes: u64) -> Self {
        Self {
            total_vram_bytes,
            os_reserve_bytes,
            slots: RwLock::new(HashMap::new()),
        }
    }

    /// Usable VRAM for AI models after subtracting OS reserve.
    pub fn usable_ai_vram(&self) -> u64 {
        self.total_vram_bytes.saturating_sub(self.os_reserve_bytes)
    }

    /// Currently allocated VRAM across all active AI engine slots.
    pub fn currently_allocated(&self) -> u64 {
        self.slots
            .read()
            .unwrap()
            .values()
            .map(|s| s.allocated_bytes)
            .sum()
    }

    /// Free AI VRAM available for allocation.
    pub fn free_ai_vram(&self) -> u64 {
        self.usable_ai_vram().saturating_sub(self.currently_allocated())
    }

    /// Requests memory for an engine role.
    pub fn request_slot(
        &self,
        role: EngineRole,
        model_id: &str,
        required_bytes: u64,
    ) -> Result<(), VramAllocationError> {
        let mut slots = self.slots.write().unwrap();

        // If the role already has an allocation, calculate difference
        let current_for_role = slots.get(&role).map(|s| s.allocated_bytes).unwrap_or(0);
        let total_allocated: u64 = slots.values().map(|s| s.allocated_bytes).sum();
        let projected = total_allocated.saturating_sub(current_for_role) + required_bytes;

        let usable = self.usable_ai_vram();
        if projected > usable {
            return Err(VramAllocationError::InsufficientMemory {
                requested_gb: required_bytes as f64 / (1024.0 * 1024.0 * 1024.0),
                available_gb: (usable.saturating_sub(total_allocated.saturating_sub(current_for_role)))
                    as f64
                    / (1024.0 * 1024.0 * 1024.0),
            });
        }

        slots.insert(
            role,
            AllocatedSlot {
                model_id: model_id.to_string(),
                allocated_bytes: required_bytes,
                last_used: Instant::now(),
            },
        );

        Ok(())
    }

    /// Releases an engine slot.
    pub fn release_slot(&self, role: EngineRole) {
        self.slots.write().unwrap().remove(&role);
    }

    /// Marks an engine slot as active (updates last used timestamp).
    pub fn touch_slot(&self, role: EngineRole) {
        if let Some(slot) = self.slots.write().unwrap().get_mut(&role) {
            slot.last_used = Instant::now();
        }
    }

    /// Returns current VRAM budget status.
    pub fn status(&self) -> VramBudgetStatus {
        let slots = self.slots.read().unwrap();
        let allocated: u64 = slots.values().map(|s| s.allocated_bytes).sum();
        let usable = self.usable_ai_vram();
        let free = usable.saturating_sub(allocated);

        let active_allocations = slots
            .iter()
            .map(|(role, slot)| VramAllocationInfo {
                role: *role,
                model_id: slot.model_id.clone(),
                allocated_bytes: slot.allocated_bytes,
            })
            .collect();

        VramBudgetStatus {
            total_dedicated_vram_bytes: self.total_vram_bytes,
            os_reserved_bytes: self.os_reserve_bytes,
            usable_ai_vram_bytes: usable,
            currently_allocated_bytes: allocated,
            free_ai_vram_bytes: free,
            active_allocations,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vram_manager_budgeting() {
        let mgr = VramManager::with_os_reserve(
            8 * 1024 * 1024 * 1024, // 8 GB VRAM
            1 * 1024 * 1024 * 1024, // 1 GB OS reserve -> 7 GB usable
        );

        assert_eq!(mgr.usable_ai_vram(), 7 * 1024 * 1024 * 1024);
        assert_eq!(mgr.free_ai_vram(), 7 * 1024 * 1024 * 1024);

        // Allocate 1 GB for STT (Whisper)
        let stt_res = mgr.request_slot(EngineRole::SpeechToText, "whisper-small", 1024 * 1024 * 1024);
        assert!(stt_res.is_ok());
        assert_eq!(mgr.free_ai_vram(), 6 * 1024 * 1024 * 1024);

        // Allocate 4.5 GB for LLM (Llama 3 8B Q4)
        let llm_res = mgr.request_slot(
            EngineRole::LargeLanguageModel,
            "llama-3-8b",
            (4.5 * 1024.0 * 1024.0 * 1024.0) as u64,
        );
        assert!(llm_res.is_ok());

        // Allocate 500 MB for TTS
        let tts_res = mgr.request_slot(EngineRole::TextToSpeech, "piper", 500 * 1024 * 1024);
        assert!(tts_res.is_ok());

        // Try to allocate another 3 GB (which would exceed 7 GB usable)
        let overcommit = mgr.request_slot(EngineRole::Vision, "vision-model", 3 * 1024 * 1024 * 1024);
        assert!(overcommit.is_err());

        // Release STT
        mgr.release_slot(EngineRole::SpeechToText);
        assert!(mgr.free_ai_vram() > 1024 * 1024 * 1024);
    }
}
