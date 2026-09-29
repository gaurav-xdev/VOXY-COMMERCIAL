use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModelFormat {
    Gguf,
    SafeTensors,
    OllamaBlob,
    Onnx,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModelFamily {
    Llama,
    Qwen,
    Mistral,
    Gemma,
    Phi,
    Whisper,
    Piper,
    DeepSeek,
    Other(String),
}

#[allow(non_camel_case_types)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Quantization {
    Q4_0,
    Q4_K_M,
    Q4_K_S,
    Q5_0,
    Q5_K_M,
    Q8_0,
    F16,
    F32,
    Unknown(String),
}

impl Quantization {
    pub fn bits_per_weight(&self) -> f32 {
        match self {
            Self::Q4_0 | Self::Q4_K_S => 4.5,
            Self::Q4_K_M => 4.8,
            Self::Q5_0 | Self::Q5_K_M => 5.5,
            Self::Q8_0 => 8.5,
            Self::F16 => 16.0,
            Self::F32 => 32.0,
            Self::Unknown(_) => 5.0, // default estimate
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalModelMetadata {
    pub id: String,
    pub name: String,
    pub file_path: PathBuf,
    pub format: ModelFormat,
    pub family: ModelFamily,
    pub parameters_billions: Option<f32>,
    pub quantization: Quantization,
    pub file_size_bytes: u64,
    pub context_length: Option<u32>,
}

pub struct ModelScanner;

impl ModelScanner {
    /// Scans standard model directories (Ollama, HuggingFace, VOXY local) for models.
    pub fn scan_all() -> Vec<LocalModelMetadata> {
        let mut results = Vec::new();

        // 1. VOXY local models directory (%LOCALAPPDATA%\VOXY\models)
        if let Some(local_appdata) = dirs::data_local_dir() {
            let voxy_models = local_appdata.join("VOXY").join("models");
            if voxy_models.exists() {
                results.extend(Self::scan_directory(&voxy_models));
            }
        }

        // 2. Ollama models manifests
        results.extend(Self::scan_ollama_manifests());

        // 3. Hugging Face hub cache
        if let Some(home) = dirs::home_dir() {
            let hf_hub = home.join(".cache").join("huggingface").join("hub");
            if hf_hub.exists() {
                results.extend(Self::scan_directory(&hf_hub));
            }
        }

        results
    }

    /// Recursively scans a directory for supported model files (max depth 5).
    pub fn scan_directory(dir: &Path) -> Vec<LocalModelMetadata> {
        Self::scan_directory_bounded(dir, 0, 5)
    }

    fn scan_directory_bounded(dir: &Path, current_depth: usize, max_depth: usize) -> Vec<LocalModelMetadata> {
        let mut models = Vec::new();
        if current_depth > max_depth || !dir.is_dir() {
            return models;
        }

        let walker = match std::fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(_) => return models,
        };

        for entry in walker.flatten() {
            let path = entry.path();
            if path.is_dir() {
                models.extend(Self::scan_directory_bounded(&path, current_depth + 1, max_depth));
            } else if path.is_file() {
                if let Some(model) = Self::parse_model_file(&path) {
                    models.push(model);
                }
            }
        }

        models
    }

    /// Parses a model file by path and filename patterns.
    pub fn parse_model_file(path: &Path) -> Option<LocalModelMetadata> {
        let file_name = path.file_name()?.to_string_lossy().to_string();
        let ext = path
            .extension()
            .map(|e| e.to_string_lossy().to_lowercase())
            .unwrap_or_default();

        let format = match ext.as_str() {
            "gguf" => ModelFormat::Gguf,
            "safetensors" => ModelFormat::SafeTensors,
            "onnx" => ModelFormat::Onnx,
            _ => return None,
        };

        let file_size_bytes = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
        let name_lower = file_name.to_lowercase();

        let family = if name_lower.contains("llama") {
            ModelFamily::Llama
        } else if name_lower.contains("qwen") {
            ModelFamily::Qwen
        } else if name_lower.contains("mistral") {
            ModelFamily::Mistral
        } else if name_lower.contains("gemma") {
            ModelFamily::Gemma
        } else if name_lower.contains("phi") {
            ModelFamily::Phi
        } else if name_lower.contains("deepseek") {
            ModelFamily::DeepSeek
        } else if name_lower.contains("whisper") {
            ModelFamily::Whisper
        } else if name_lower.contains("piper") {
            ModelFamily::Piper
        } else {
            ModelFamily::Other(file_name.clone())
        };

        let parameters_billions = Self::extract_parameter_size(&name_lower);
        let quantization = Self::extract_quantization(&name_lower);
        let context_length = Self::estimate_context_length(&family);

        Some(LocalModelMetadata {
            id: file_name.clone(),
            name: file_name,
            file_path: path.to_path_buf(),
            format,
            family,
            parameters_billions,
            quantization,
            file_size_bytes,
            context_length,
        })
    }

    /// Scans Ollama manifest directories (e.g. ~/.ollama/models/manifests).
    pub fn scan_ollama_manifests() -> Vec<LocalModelMetadata> {
        let mut models = Vec::new();
        let ollama_dir = std::env::var("OLLAMA_MODELS")
            .map(PathBuf::from)
            .ok()
            .or_else(|| dirs::home_dir().map(|h| h.join(".ollama").join("models")));

        if let Some(base) = ollama_dir {
            let manifests_dir = base.join("manifests");
            if manifests_dir.exists() {
                models.extend(Self::scan_manifest_dir(&manifests_dir, &base));
            }
        }

        models
    }

    fn scan_manifest_dir(dir: &Path, base_models_dir: &Path) -> Vec<LocalModelMetadata> {
        Self::scan_manifest_dir_bounded(dir, base_models_dir, 0, 3)
    }

    fn scan_manifest_dir_bounded(dir: &Path, base_models_dir: &Path, current_depth: usize, max_depth: usize) -> Vec<LocalModelMetadata> {
        let mut models = Vec::new();
        if current_depth > max_depth || !dir.is_dir() {
            return models;
        }

        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    models.extend(Self::scan_manifest_dir_bounded(&path, base_models_dir, current_depth + 1, max_depth));
                } else if path.is_file() {
                    let model_tag = path.file_name().unwrap_or_default().to_string_lossy().to_string();
                    let file_size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
                    let tag_lower = model_tag.to_lowercase();
                    let family = if tag_lower.contains("llama") {
                        ModelFamily::Llama
                    } else if tag_lower.contains("qwen") {
                        ModelFamily::Qwen
                    } else if tag_lower.contains("mistral") {
                        ModelFamily::Mistral
                    } else if tag_lower.contains("deepseek") {
                        ModelFamily::DeepSeek
                    } else {
                        ModelFamily::Other(model_tag.clone())
                    };

                    models.push(LocalModelMetadata {
                        id: format!("ollama:{}", model_tag),
                        name: model_tag,
                        file_path: path,
                        format: ModelFormat::OllamaBlob,
                        family: family.clone(),
                        parameters_billions: Self::extract_parameter_size(&tag_lower),
                        quantization: Self::extract_quantization(&tag_lower),
                        file_size_bytes: file_size,
                        context_length: Self::estimate_context_length(&family),
                    });
                }
            }
        }
        models
    }

    fn extract_parameter_size(name: &str) -> Option<f32> {
        // Look for patterns like "1.5b", "3b", "7b", "8b", "14b", "32b", "70b"
        let parts: Vec<&str> = name.split(|c: char| !c.is_alphanumeric() && c != '.').collect();
        for p in parts {
            if let Some(stripped) = p.strip_suffix('b') {
                if let Ok(val) = stripped.parse::<f32>() {
                    return Some(val);
                }
            }
        }
        None
    }

    fn extract_quantization(name: &str) -> Quantization {
        if name.contains("q4_k_m") {
            Quantization::Q4_K_M
        } else if name.contains("q4_k_s") {
            Quantization::Q4_K_S
        } else if name.contains("q4_0") {
            Quantization::Q4_0
        } else if name.contains("q5_k_m") {
            Quantization::Q5_K_M
        } else if name.contains("q5_0") {
            Quantization::Q5_0
        } else if name.contains("q8_0") {
            Quantization::Q8_0
        } else if name.contains("fp16") || name.contains("f16") {
            Quantization::F16
        } else if name.contains("fp32") || name.contains("f32") {
            Quantization::F32
        } else {
            Quantization::Unknown("default".into())
        }
    }

    fn estimate_context_length(family: &ModelFamily) -> Option<u32> {
        match family {
            ModelFamily::Llama => Some(8192),
            ModelFamily::Qwen => Some(32768),
            ModelFamily::Mistral => Some(32768),
            ModelFamily::DeepSeek => Some(65536),
            ModelFamily::Gemma => Some(8192),
            ModelFamily::Phi => Some(4096),
            _ => Some(4096),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_parameter_size() {
        assert_eq!(ModelScanner::extract_parameter_size("llama-3.2-3b-instruct"), Some(3.0));
        assert_eq!(ModelScanner::extract_parameter_size("qwen2.5-7b-q4_k_m.gguf"), Some(7.0));
        assert_eq!(ModelScanner::extract_parameter_size("deepseek-r1-14b"), Some(14.0));
        assert_eq!(ModelScanner::extract_parameter_size("no-param-size"), None);
    }

    #[test]
    fn test_extract_quantization() {
        assert_eq!(ModelScanner::extract_quantization("model-q4_k_m.gguf"), Quantization::Q4_K_M);
        assert_eq!(ModelScanner::extract_quantization("model-q8_0.gguf"), Quantization::Q8_0);
        assert_eq!(ModelScanner::extract_quantization("model-fp16.safetensors"), Quantization::F16);
    }
}
