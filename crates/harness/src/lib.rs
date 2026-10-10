//! VOXY Autonomous Coding Harness.
//!
//! Provides controlled repository discovery, semantic symbol search, AST/patch generation,
//! safe modifications with rollback, and sandboxed command execution with emergency stop tokens.

pub mod diagnostic_parser;
pub mod ide_launcher;
pub mod patch_engine;
pub mod repo_indexer;
pub mod sandbox_runner;
pub mod secret_scanner;

pub use diagnostic_parser::{
    DiagnosticItem, DiagnosticParser, DiagnosticReport, DiagnosticSeverity,
};
pub use ide_launcher::{IdeError, IdeLauncher, SupportedIde};
pub use patch_engine::{FilePatch, PatchEngine, PatchError, PatchSnapshot, PatchTransaction};
pub use repo_indexer::{FileMetadata, RepositoryIndexer, SymbolInfo};
pub use sandbox_runner::{CommandOutput, RunnerError, SandboxedRunner};
pub use secret_scanner::{DetectedSecret, SecretScanner};
