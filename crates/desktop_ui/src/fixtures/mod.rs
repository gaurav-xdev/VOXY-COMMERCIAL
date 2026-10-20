//! Deterministic mock fixtures for OSMOO desktop frontend.
//!
//! Note: All data structures here are strictly frontend presentation fixtures
//! used to drive offline UI states, component testing, and catalog displays.
//! They do not represent live backend connections or stored user secrets.

/// Model catalog item for cloud and local AI runtimes.
#[derive(Clone, Debug, PartialEq)]
pub struct ModelCatalogItem {
    pub id: &'static str,
    pub name: &'static str,
    pub provider: &'static str,
    pub parameter_size: &'static str,
    pub context_window: &'static str,
    pub description: &'static str,
    pub latency_tier: &'static str,
    pub is_local: bool,
}

/// Cloud provider catalog entry.
#[derive(Clone, Debug, PartialEq)]
pub struct ProviderCatalogItem {
    pub id: &'static str,
    pub name: &'static str,
    pub category: &'static str,
    pub badge: &'static str,
    pub is_recommended: bool,
    pub requires_api_key: bool,
    pub docs_url: &'static str,
}

/// Voice synthesis / recognition model catalog item.
#[derive(Clone, Debug, PartialEq)]
pub struct VoiceModelItem {
    pub id: &'static str,
    pub name: &'static str,
    pub provider: &'static str,
    pub modality: &'static str,
    pub sample_rate_hz: u32,
    pub is_local: bool,
}

/// Subscription tier specification.
#[derive(Clone, Debug, PartialEq)]
pub struct SubscriptionTier {
    pub id: &'static str,
    pub name: &'static str,
    pub price: &'static str,
    pub cadence: &'static str,
    pub badge: &'static str,
    pub is_popular: bool,
    pub features: &'static [&'static str],
}

/// Static catalog of cloud AI providers.
pub const PROVIDER_CATALOG: &[ProviderCatalogItem] = &[
    ProviderCatalogItem {
        id: "ollama",
        name: "Ollama (Local Engine)",
        category: "Local Runtime",
        badge: "Zero-Cloud Privacy",
        is_recommended: true,
        requires_api_key: false,
        docs_url: "https://ollama.ai",
    },
    ProviderCatalogItem {
        id: "lmstudio",
        name: "LM Studio (Local Server)",
        category: "Local Runtime",
        badge: "OpenAI-Compatible",
        is_recommended: false,
        requires_api_key: false,
        docs_url: "https://lmstudio.ai",
    },
    ProviderCatalogItem {
        id: "openai",
        name: "OpenAI",
        category: "Cloud Provider",
        badge: "GPT-4o / o1",
        is_recommended: false,
        requires_api_key: true,
        docs_url: "https://platform.openai.com",
    },
    ProviderCatalogItem {
        id: "gemini",
        name: "Google Gemini",
        category: "Cloud Provider",
        badge: "Gemini 1.5 Pro",
        is_recommended: false,
        requires_api_key: true,
        docs_url: "https://ai.google.dev",
    },
    ProviderCatalogItem {
        id: "anthropic",
        name: "Anthropic Claude",
        category: "Cloud Provider",
        badge: "Claude 3.5 Sonnet",
        is_recommended: false,
        requires_api_key: true,
        docs_url: "https://console.anthropic.com",
    },
    ProviderCatalogItem {
        id: "deepseek",
        name: "DeepSeek",
        category: "Cloud Provider",
        badge: "DeepSeek V3 / R1",
        is_recommended: false,
        requires_api_key: true,
        docs_url: "https://platform.deepseek.com",
    },
    ProviderCatalogItem {
        id: "groq",
        name: "Groq LPU",
        category: "Cloud Provider",
        badge: "Ultra-Fast Inference",
        is_recommended: false,
        requires_api_key: true,
        docs_url: "https://console.groq.com",
    },
    ProviderCatalogItem {
        id: "nvidia_nim",
        name: "NVIDIA NIM",
        category: "Enterprise Cloud",
        badge: "Hardware-Optimized",
        is_recommended: false,
        requires_api_key: true,
        docs_url: "https://build.nvidia.com",
    },
];

/// Catalog of default models when offline or unconfigured.
pub const DEFAULT_MODELS: &[ModelCatalogItem] = &[
    ModelCatalogItem {
        id: "llama3.2:3b",
        name: "Llama 3.2 (3B)",
        provider: "Ollama",
        parameter_size: "3.2B",
        context_window: "128k",
        description: "Optimized for lightweight local desktop edge devices and quick companion responses.",
        latency_tier: "Instant (<25ms)",
        is_local: true,
    },
    ModelCatalogItem {
        id: "qwen2.5:7b",
        name: "Qwen 2.5 (7B)",
        provider: "Ollama",
        parameter_size: "7.6B",
        context_window: "32k",
        description: "High-accuracy multilingual reasoning and OS execution instructions.",
        latency_tier: "Fast (<45ms)",
        is_local: true,
    },
    ModelCatalogItem {
        id: "gpt-4o",
        name: "GPT-4o",
        provider: "OpenAI",
        parameter_size: "Undisclosed",
        context_window: "128k",
        description: "Omni multimodal model for high-complexity workspace coding and automation.",
        latency_tier: "Standard Cloud",
        is_local: false,
    },
    ModelCatalogItem {
        id: "claude-3-5-sonnet",
        name: "Claude 3.5 Sonnet",
        provider: "Anthropic",
        parameter_size: "Undisclosed",
        context_window: "200k",
        description: "State-of-the-art code generation, architecture planning, and tool orchestration.",
        latency_tier: "Standard Cloud",
        is_local: false,
    },
];

/// Default voice model offerings.
pub const VOICE_CATALOG: &[VoiceModelItem] = &[
    VoiceModelItem {
        id: "sherpa_onnx_whisper",
        name: "Sherpa-ONNX Whisper Tiny",
        provider: "Local (ONNX)",
        modality: "Speech-to-Text (STT)",
        sample_rate_hz: 16000,
        is_local: true,
    },
    VoiceModelItem {
        id: "piper_en_libritts",
        name: "Piper Local TTS (Amy Voice)",
        provider: "Local (Piper)",
        modality: "Text-to-Speech (TTS)",
        sample_rate_hz: 22050,
        is_local: true,
    },
    VoiceModelItem {
        id: "deepgram_nova_2",
        name: "Deepgram Nova-2 Medical/OS",
        provider: "Deepgram Cloud",
        modality: "Speech-to-Text (STT)",
        sample_rate_hz: 48000,
        is_local: false,
    },
    VoiceModelItem {
        id: "cartesia_sonic",
        name: "Cartesia Sonic Realtime",
        provider: "Cartesia Cloud",
        modality: "Text-to-Speech (TTS)",
        sample_rate_hz: 48000,
        is_local: false,
    },
];

/// Subscription plans presentation.
pub const SUBSCRIPTION_PLANS: &[SubscriptionTier] = &[
    SubscriptionTier {
        id: "free",
        name: "Community",
        price: "$0",
        cadence: "forever",
        badge: "Local-First",
        is_popular: false,
        features: &[
            "Unlimited Local LLM Inference (Ollama, LM Studio)",
            "Local Voice Capture & Offline Synthesis",
            "Windows OS Companion HUD & Particle Orb",
            "Local SQLite Permanent Memory Engine",
        ],
    },
    SubscriptionTier {
        id: "pro",
        name: "Pro Companion",
        price: "$19",
        cadence: "per month",
        badge: "Most Popular",
        is_popular: true,
        features: &[
            "All Community capabilities included",
            "Direct Cloud Router (OpenAI, Gemini, Anthropic)",
            "Real-time Multimodal Screen Understanding",
            "Low-latency Cloud Neural Voices (Cartesia, Deepgram)",
            "Autonomous Tool Execution with Rollback Safety",
        ],
    },
    SubscriptionTier {
        id: "max",
        name: "Enterprise Studio",
        price: "$49",
        cadence: "per seat / mo",
        badge: "Maximum Security",
        is_popular: false,
        features: &[
            "All Pro capabilities included",
            "NVIDIA NIM Enterprise Deployment Bridge",
            "Multi-Agent Coding Harness with Worktrees",
            "Team Memory Synchronization & Vault Policies",
            "Custom Dedicated Voice Cloning Enclaves",
        ],
    },
];
