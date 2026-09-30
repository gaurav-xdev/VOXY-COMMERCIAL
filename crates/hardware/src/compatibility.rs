use serde::{Deserialize, Serialize};

use crate::hardware_detector::SystemHardwareSummary;
use crate::model_scanner::{LocalModelMetadata, ModelFamily, Quantization};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CompatibilityTier {
    /// The entire model and its KV cache fit comfortably in dedicated GPU VRAM with headroom.
    FullGpu,
    /// Model can run with partial GPU layer offloading (percentage of layers offloaded to VRAM).
    PartialOffload { vram_layers_pct: u8 },
    /// Model exceeds dedicated VRAM but fits in system RAM (slower CPU inference).
    CpuOnly,
    /// Model cannot run because it exceeds total available system memory (RAM + VRAM).
    Incompatible { reason: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelResourceEstimate {
    pub weights_memory_bytes: u64,
    pub kv_cache_memory_bytes: u64,
    pub runtime_overhead_bytes: u64,
    pub total_required_memory_bytes: u64,
    pub recommended_vram_bytes: u64,
}

impl ModelResourceEstimate {
    pub fn weights_gb(&self) -> f64 {
        self.weights_memory_bytes as f64 / (1024.0 * 1024.0 * 1024.0)
    }

    pub fn kv_cache_gb(&self) -> f64 {
        self.kv_cache_memory_bytes as f64 / (1024.0 * 1024.0 * 1024.0)
    }

    pub fn total_required_gb(&self) -> f64 {
        self.total_required_memory_bytes as f64 / (1024.0 * 1024.0 * 1024.0)
    }
}

pub struct CompatibilityEngine;

impl CompatibilityEngine {
    /// Estimates memory requirements for weights, KV cache, and runtime overhead.
    pub fn estimate_requirements(
        params_billions: f32,
        quantization: &Quantization,
        context_length: u32,
        family: &ModelFamily,
    ) -> ModelResourceEstimate {
        // 1. Weights memory = params * bits_per_weight / 8
        let bits_per_weight = quantization.bits_per_weight();
        let weights_bytes = (params_billions * 1_000_000_000.0 * (bits_per_weight / 8.0)) as u64;

        // 2. KV cache architecture estimation based on family and parameter count
        let (layers, kv_heads, head_dim) = match family {
            ModelFamily::Whisper => (6, 8, 64),
            ModelFamily::Piper => (4, 4, 32),
            _ => {
                if params_billions <= 3.5 {
                    (24, 8, 64)
                } else if params_billions <= 9.0 {
                    (32, 8, 128)
                } else if params_billions <= 16.0 {
                    (48, 8, 128)
                } else if params_billions <= 35.0 {
                    (64, 8, 128)
                } else {
                    (80, 8, 128)
                }
            }
        };

        // KV cache = 2 * layers * kv_heads * head_dim * context_len * 2 bytes (FP16)
        let kv_cache_bytes =
            2u64 * layers as u64 * kv_heads as u64 * head_dim as u64 * context_length as u64 * 2;

        // Runtime activation buffers & CUDA context overhead (typically ~512MB - 1GB)
        let runtime_overhead_bytes = 512 * 1024 * 1024;

        let total_required = weights_bytes + kv_cache_bytes + runtime_overhead_bytes;
        // Headroom buffer of 15%
        let recommended_vram = (total_required as f64 * 1.15) as u64;

        ModelResourceEstimate {
            weights_memory_bytes: weights_bytes,
            kv_cache_memory_bytes: kv_cache_bytes,
            runtime_overhead_bytes,
            total_required_memory_bytes: total_required,
            recommended_vram_bytes: recommended_vram,
        }
    }

    /// Evaluates compatibility of a local model against detected system hardware.
    pub fn evaluate(
        model: &LocalModelMetadata,
        hardware: &SystemHardwareSummary,
        target_context: Option<u32>,
    ) -> CompatibilityTier {
        let params = model.parameters_billions.unwrap_or_else(|| {
            // Estimate parameter count from file size if unknown
            let gb = model.file_size_bytes as f64 / (1024.0 * 1024.0 * 1024.0);
            (gb / 0.6) as f32 // approx 0.6GB per billion for Q4
        });

        let ctx = target_context
            .or(model.context_length)
            .unwrap_or(4096)
            .min(16384); // clamp default for sane desktop footprint

        let estimate = Self::estimate_requirements(params, &model.quantization, ctx, &model.family);

        let dedicated_vram = hardware.total_dedicated_vram_bytes;
        let total_ram = hardware.total_ram_bytes;
        let available_ram = hardware.available_ram_bytes;

        // VRAM headroom safety buffer (512MB for Windows DWM and system apps)
        let usable_vram = dedicated_vram.saturating_sub(512 * 1024 * 1024);

        if hardware.has_discrete_gpu() && estimate.total_required_memory_bytes <= usable_vram {
            CompatibilityTier::FullGpu
        } else if hardware.has_discrete_gpu()
            && usable_vram > estimate.kv_cache_memory_bytes + (1024 * 1024 * 1024)
        {
            // Can fit KV cache and a portion of the layers in VRAM
            let remaining_vram_for_weights = usable_vram
                .saturating_sub(estimate.kv_cache_memory_bytes + estimate.runtime_overhead_bytes);
            let pct = ((remaining_vram_for_weights as f64 / estimate.weights_memory_bytes as f64)
                * 100.0)
                .clamp(10.0, 95.0) as u8;

            if estimate.total_required_memory_bytes <= available_ram + usable_vram {
                CompatibilityTier::PartialOffload {
                    vram_layers_pct: pct,
                }
            } else {
                CompatibilityTier::Incompatible {
                    reason: format!(
                        "Requires {:.1} GB total memory, but available RAM ({:.1} GB) and VRAM ({:.1} GB) are insufficient",
                        estimate.total_required_gb(),
                        hardware.available_ram_gb(),
                        hardware.total_vram_gb()
                    ),
                }
            }
        } else if estimate.total_required_memory_bytes <= total_ram {
            CompatibilityTier::CpuOnly
        } else {
            CompatibilityTier::Incompatible {
                reason: format!(
                    "Requires {:.1} GB memory, exceeding total system RAM ({:.1} GB)",
                    estimate.total_required_gb(),
                    hardware.total_ram_gb()
                ),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hardware_detector::GpuInfo;
    use std::path::PathBuf;

    #[test]
    fn test_estimate_7b_q4_requirements() {
        let est = CompatibilityEngine::estimate_requirements(
            7.0,
            &Quantization::Q4_K_M,
            4096,
            &ModelFamily::Llama,
        );
        // 7B Q4 weights should be roughly 3.8 - 4.5 GB (binary gigabytes)
        assert!(est.weights_gb() >= 3.8 && est.weights_gb() <= 4.6);
        // Total required should be ~4.5 - 5.5 GB
        assert!(est.total_required_gb() >= 4.5 && est.total_required_gb() <= 5.8);
    }

    #[test]
    fn test_evaluate_full_gpu() {
        let hardware = SystemHardwareSummary {
            gpus: vec![GpuInfo {
                name: "Test GPU".into(),
                vendor: crate::hardware_detector::GpuVendor::Nvidia,
                vendor_id: 0x10DE,
                device_id: 0x1234,
                dedicated_vram_bytes: 16 * 1024 * 1024 * 1024, // 16 GB VRAM
                shared_system_memory_bytes: 8 * 1024 * 1024 * 1024,
                is_discrete: true,
                is_software: false,
            }],
            primary_gpu: None,
            total_dedicated_vram_bytes: 16 * 1024 * 1024 * 1024,
            total_ram_bytes: 32 * 1024 * 1024 * 1024,
            available_ram_bytes: 20 * 1024 * 1024 * 1024,
            cpu_cores: 16,
            cpu_brand: "AMD Ryzen".into(),
            os_arch: "x86_64".into(),
            os_platform: "windows".into(),
        };

        let model = LocalModelMetadata {
            id: "llama3-8b.gguf".into(),
            name: "Llama 3 8B".into(),
            file_path: PathBuf::from("llama3-8b.gguf"),
            format: crate::model_scanner::ModelFormat::Gguf,
            family: ModelFamily::Llama,
            parameters_billions: Some(8.0),
            quantization: Quantization::Q4_K_M,
            file_size_bytes: 4_500_000_000,
            context_length: Some(4096),
        };

        let tier = CompatibilityEngine::evaluate(&model, &hardware, Some(4096));
        assert_eq!(tier, CompatibilityTier::FullGpu);
    }
}
