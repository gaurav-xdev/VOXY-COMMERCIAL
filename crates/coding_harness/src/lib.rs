//! OSMOO Autonomous Senior Engineering Harness Runtime.
//!
//! Provides planning, context exploration, atomic patching, durable write-ahead transaction recovery,
//! Windows Job Object process confinement, independent code review, automated diagnosis,
//! multi-language toolchain verification, and human approval integration.

pub mod claude_cli_backend;
pub mod context_explorer;
pub mod debugger;
pub mod engine;
pub mod journal;
pub mod mcp_bridge;
pub mod patcher;
pub mod planner;
pub mod process_runner;
pub mod reviewer;
pub mod session_store;
pub mod skill_router;
pub mod symbol_index;
pub mod token_budget;
pub mod toolchain;
pub mod types;

pub use claude_cli_backend::ClaudeCliBackend;
pub use context_explorer::{ContextExplorer, GrepMatch};
pub use debugger::{DebugAnalysis, Debugger, Diagnostic, DiagnosticSeverity};
pub use engine::{AutonomousRepairResult, CodingEngine};
pub use journal::{
    ActiveTransaction, JournaledFile, TransactionJournal, TransactionManifest, TransactionStatus,
};
pub use mcp_bridge::{
    McpBridge, McpContentItem, McpServerInfo, McpStdioClient, McpToolCallResult, McpToolDefinition,
    NativeToolAdapter,
};
pub use patcher::Patcher;
pub use planner::TaskPlanner;
pub use process_runner::ProcessRunner;
pub use reviewer::Reviewer;
pub use session_store::SessionStore;
pub use skill_router::{SkillRouter, SkillSuggestion};
pub use symbol_index::{RepositoryMap, Symbol, SymbolIndexer, SymbolKind};
pub use token_budget::{
    BudgetConfig, ContextCompressor, EfficiencyMetrics, SelectedContextChunk,
    TargetedContextSelector, TokenEstimator,
};
pub use toolchain::{Language, LanguageToolchain, ToolchainDetector, ToolchainStatus};
pub use types::*;

#[cfg(test)]
mod tests;
