pub mod barge_in;
pub mod chunker;
pub mod endpointing;
pub mod intent_preprocessor;
pub mod preroll;
pub mod speculative;
pub mod transcript;

pub use barge_in::{rewind_target, BargeInAction, BargeInCoordinator, BargeInState};
pub use chunker::SentenceChunker;
pub use endpointing::{is_filler, EndpointDecision, TieredEndpointing};
pub use intent_preprocessor::{
    DesktopCommand, FastIntentAction, FastIntentResult, IntentPreprocessor, SystemQueryType,
};
pub use preroll::PreRollBuffer;
pub use speculative::{
    speculation_reuse_chars, speculative_prefix_matches, PrefillDecision, SpeculativePrefill,
};
pub use transcript::{join_carry, longest_common_prefix, TranscriptStabilizer};
