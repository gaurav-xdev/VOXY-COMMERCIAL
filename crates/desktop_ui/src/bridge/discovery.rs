pub struct LocalModelDiscovery;

impl LocalModelDiscovery {
    /// Parses the JSON models array from Ollama tags response.
    pub fn parse_model_names(json_val: &serde_json::Value) -> Vec<String> {
        let mut models = Vec::new();
        if let Some(arr) = json_val.get("models").and_then(|m| m.as_array()) {
            for item in arr {
                if let Some(name) = item.get("name").and_then(|n| n.as_str()) {
                    models.push(name.to_string());
                }
            }
        }
        models
    }

    /// Queries the real Ollama HTTP daemon running locally on the system.
    /// Returns the list of installed model tags (e.g. ["llama3.2:3b", "mistral:latest", ...])
    pub async fn fetch_ollama_models() -> Result<Vec<String>, String> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_millis(1500))
            .build()
            .map_err(|e| format!("HTTP client init failed: {e}"))?;

        let resp = client
            .get("http://127.0.0.1:11434/api/tags")
            .send()
            .await
            .map_err(|e| format!("Ollama unreachable: {e}"))?;

        if !resp.status().is_success() {
            return Err(format!("Ollama API returned HTTP {}", resp.status()));
        }

        let body: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| format!("Failed to parse Ollama JSON: {e}"))?;

        Ok(Self::parse_model_names(&body))
    }

    /// Checks whether local Ollama server is alive.
    pub async fn check_ollama_alive() -> bool {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_millis(800))
            .build();
        if let Ok(c) = client {
            if let Ok(resp) = c.get("http://127.0.0.1:11434/api/tags").send().await {
                return resp.status().is_success();
            }
        }
        false
    }

    /// Discovers configured NVIDIA NIM models if API key is present in environment.
    pub async fn fetch_nim_models() -> Result<Vec<String>, String> {
        let api_key = std::env::var("VOXY_API_KEYS_NVIDIA")
            .or_else(|_| std::env::var("NVIDIA_API_KEY"))
            .map_err(|_| "NVIDIA NIM API key not configured".to_string())?;

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()
            .map_err(|e| format!("HTTP client init failed: {e}"))?;

        let resp = client
            .get("https://integrate.api.nvidia.com/v1/models")
            .bearer_auth(api_key)
            .send()
            .await
            .map_err(|e| format!("NVIDIA NIM unreachable: {e}"))?;

        if !resp.status().is_success() {
            return Err(format!("NVIDIA NIM API returned HTTP {}", resp.status()));
        }

        let body: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| format!("Failed to parse NVIDIA NIM JSON: {e}"))?;

        let mut models = Vec::new();
        if let Some(arr) = body.get("data").and_then(|d| d.as_array()) {
            for item in arr {
                if let Some(id) = item.get("id").and_then(|id| id.as_str()) {
                    models.push(id.to_string());
                }
            }
        }
        Ok(models)
    }

    /// Checks whether NVIDIA NIM is configured.
    pub fn is_nim_configured() -> bool {
        std::env::var("VOXY_API_KEYS_NVIDIA")
            .or_else(|_| std::env::var("NVIDIA_API_KEY"))
            .map(|k| !k.trim().is_empty())
            .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_ollama_model_tags() {
        let raw = serde_json::json!({
            "models": [
                { "name": "llama3.2:3b", "modified_at": "2026-01-01T00:00:00Z", "size": 2000000 },
                { "name": "mistral:latest", "modified_at": "2026-01-02T00:00:00Z", "size": 4000000 }
            ]
        });
        let parsed = LocalModelDiscovery::parse_model_names(&raw);
        assert_eq!(parsed, vec!["llama3.2:3b", "mistral:latest"]);
    }

    #[test]
    fn test_parse_empty_or_malformed_models() {
        let empty = serde_json::json!({ "models": [] });
        assert_eq!(LocalModelDiscovery::parse_model_names(&empty), Vec::<String>::new());

        let malformed = serde_json::json!({ "error": "unknown model" });
        assert_eq!(LocalModelDiscovery::parse_model_names(&malformed), Vec::<String>::new());
    }
}
