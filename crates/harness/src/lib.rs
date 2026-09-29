//! VOXY Autonomous Coding Harness.
//!
//! Provides controlled repository discovery, semantic symbol search, AST/patch generation,
//! safe modifications with rollback, and sandboxed command execution with emergency stop tokens.

pub mod patch_engine;
pub mod repo_indexer;
pub mod sandbox_runner;

pub use patch_engine::{PatchEngine, PatchError, PatchSnapshot};
pub use repo_indexer::{FileMetadata, RepositoryIndexer, SymbolInfo};
pub use sandbox_runner::{CommandOutput, RunnerError, SandboxedRunner};
