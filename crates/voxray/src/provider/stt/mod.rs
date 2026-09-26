//! Speech-to-Text provider implementations.

pub mod assemblyai;
pub mod azure;
pub mod deepgram;
pub mod elevenlabs;
pub mod google;
pub mod groq;
pub mod local_sapi;
pub mod mock;

pub use assemblyai::AssemblyAISTTProvider;
pub use azure::AzureSTTProvider;
pub use deepgram::DeepgramSTTProvider;
pub use elevenlabs::ElevenLabsSTTProvider;
pub use google::GoogleSTTProvider;
pub use groq::GroqSTTProvider;
pub use local_sapi::LocalSapiSTTProvider;
pub use mock::MockSTTProvider;
