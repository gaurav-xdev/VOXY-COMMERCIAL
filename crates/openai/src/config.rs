use std::time::Duration;
use zeroize::Zeroize;

#[derive(Clone, Zeroize)]
pub struct OpenAIConfig {
    #[zeroize(skip)]
    pub api_key: zeroize::Zeroizing<String>,
    #[zeroize(skip)]
    pub base_url: String,
    #[zeroize(skip)]
    pub default_model: Option<String>,
    #[zeroize(skip)]
    pub timeout: Duration,
    #[zeroize(skip)]
    pub max_retries: u32,
    #[zeroize(skip)]
    pub organization: Option<String>,
}

impl OpenAIConfig {
    pub fn new(api_key: impl Into<String>, base_url: impl Into<String>) -> Self {
        Self {
            api_key: zeroize::Zeroizing::new(api_key.into()),
            base_url: base_url.into(),
            default_model: None,
            timeout: Duration::from_secs(60),
            max_retries: 3,
            organization: None,
        }
    }

    pub fn openai(api_key: impl Into<String>) -> Self {
        Self::new(api_key, "https://api.openai.com")
    }

    pub fn groq(api_key: impl Into<String>) -> Self {
        Self::new(api_key, "https://api.groq.com/openai")
    }

    pub fn openrouter(api_key: impl Into<String>) -> Self {
        Self::new(api_key, "https://openrouter.ai/api")
    }

    pub fn deepseek(api_key: impl Into<String>) -> Self {
        Self::new(api_key, "https://api.deepseek.com")
    }

    pub fn xai(api_key: impl Into<String>) -> Self {
        Self::new(api_key, "https://api.x.ai")
    }

    pub fn mistral(api_key: impl Into<String>) -> Self {
        Self::new(api_key, "https://api.mistral.ai")
    }

    pub fn together(api_key: impl Into<String>) -> Self {
        Self::new(api_key, "https://api.together.xyz")
    }

    pub fn fireworks(api_key: impl Into<String>) -> Self {
        Self::new(api_key, "https://api.fireworks.ai/inference")
    }

    pub fn cerebras(api_key: impl Into<String>) -> Self {
        Self::new(api_key, "https://api.cerebras.ai")
    }

    pub fn azure(api_key: impl Into<String>, resource: &str, deployment: &str) -> Self {
        Self::new(
            api_key,
            format!("https://{resource}.openai.azure.com/openai/deployments/{deployment}"),
        )
    }

    pub fn bedrock(_api_key: impl Into<String>) -> Self {
        Self::new("", "https://bedrock-runtime.us-east-1.amazonaws.com")
    }
}

impl std::fmt::Debug for OpenAIConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OpenAIConfig")
            .field("api_key", &"[REDACTED]")
            .field("base_url", &self.base_url)
            .field("default_model", &self.default_model)
            .field("timeout", &self.timeout)
            .field("max_retries", &self.max_retries)
            .field("organization", &self.organization)
            .finish()
    }
}

impl Default for OpenAIConfig {
    fn default() -> Self {
        Self {
            api_key: zeroize::Zeroizing::new(String::new()),
            base_url: "http://127.0.0.1:1234".into(),
            default_model: None,
            timeout: Duration::from_secs(60),
            max_retries: 3,
            organization: None,
        }
    }
}
