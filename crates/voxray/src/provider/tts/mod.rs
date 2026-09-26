//! Text-to-Speech provider implementations.

pub mod azure;
pub mod cartesia;
pub mod chatterbox;
pub mod deepgram;
pub mod elevenlabs;
pub mod google;
pub mod local_sapi;
pub mod mock;
pub mod openai;

pub use azure::AzureTTSProvider;
pub use cartesia::CartesiaTTSProvider;
pub use chatterbox::ChatterboxTTSProvider;
pub use deepgram::DeepgramTTSProvider;
pub use elevenlabs::ElevenLabsTTSProvider;
pub use google::GoogleTTSProvider;
pub use local_sapi::LocalSapiTTSProvider;
pub use mock::MockTTSProvider;
pub use openai::OpenAITTSProvider;
