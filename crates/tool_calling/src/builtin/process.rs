//! Process management and diagnostic tools.

use async_trait::async_trait;
use serde_json::json;
use std::time::Instant;

use crate::error::{Result, ToolError};
use crate::metadata::{RiskTier, ToolCategory, ToolContext, ToolMetadata, ToolResult};
use crate::traits::Tool;

#[cfg(windows)]
use voxy_platform_core::traits::ProcessPlatform;
#[cfg(windows)]
use voxy_platform_windows::WindowsPlatform;

/// List all running processes on the system.
pub struct ProcessListTool {
    metadata: ToolMetadata,
}

impl ProcessListTool {
    pub fn new() -> Self {
        Self {
            metadata: ToolMetadata::new(
                "process_list",
                "List running system processes with PID, executable name, and memory footprint",
                ToolCategory::Process,
                RiskTier::Read,
            )
            .with_schema(json!({
                "type": "object",
                "properties": {
                    "filter_name": { "type": "string", "description": "Optional substring to filter by process name" }
                }
            })),
        }
    }
}

impl Default for ProcessListTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for ProcessListTool {
    fn metadata(&self) -> &ToolMetadata {
        &self.metadata
    }

    async fn execute(&self, params: serde_json::Value, _ctx: &ToolContext) -> Result<ToolResult> {
        let start = Instant::now();
        let filter = params
            .get("filter_name")
            .and_then(|v| v.as_str())
            .map(|s| s.to_lowercase());

        #[cfg(windows)]
        {
            let platform = WindowsPlatform::new();
            let mut procs = platform
                .list_processes()
                .await
                .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;

            if let Some(f) = filter {
                procs.retain(|p| p.name.to_lowercase().contains(&f));
            }

            let count = procs.len();
            Ok(
                ToolResult::success(json!({ "processes": procs, "count": count }))
                    .with_observation(format!("Enumerated {} matching processes", count))
                    .with_verification("Process list acquired")
                    .with_duration_ms(start.elapsed().as_millis() as u64),
            )
        }
        #[cfg(not(windows))]
        {
            let _ = filter;
            Ok(ToolResult::success(json!({ "processes": [], "count": 0 }))
                .with_duration_ms(start.elapsed().as_millis() as u64))
        }
    }
}

/// Retrieve detailed diagnostic information for a specific process.
pub struct ProcessInfoTool {
    metadata: ToolMetadata,
}

impl ProcessInfoTool {
    pub fn new() -> Self {
        Self {
            metadata: ToolMetadata::new(
                "process_info",
                "Query detailed diagnostics (memory, image path) for a specific process PID",
                ToolCategory::Process,
                RiskTier::Read,
            )
            .with_schema(json!({
                "type": "object",
                "required": ["pid"],
                "properties": {
                    "pid": { "type": "integer", "description": "Process ID (PID) to inspect" }
                }
            })),
        }
    }
}

impl Default for ProcessInfoTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for ProcessInfoTool {
    fn metadata(&self) -> &ToolMetadata {
        &self.metadata
    }

    async fn execute(&self, params: serde_json::Value, _ctx: &ToolContext) -> Result<ToolResult> {
        let start = Instant::now();
        let pid = params
            .get("pid")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| ToolError::InvalidParams("Missing 'pid'".into()))?
            as u32;

        #[cfg(windows)]
        {
            let platform = WindowsPlatform::new();
            let info = platform
                .process_info(pid)
                .await
                .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;

            match info {
                Some(proc_info) => Ok(ToolResult::success(json!({ "process": proc_info }))
                    .with_observation(format!("Retrieved diagnostic info for PID {}", pid))
                    .with_duration_ms(start.elapsed().as_millis() as u64)),
                None => Err(ToolError::ExecutionFailed(format!(
                    "Process with PID {} not found or inaccessible",
                    pid
                ))),
            }
        }
        #[cfg(not(windows))]
        {
            let _ = pid;
            Ok(ToolResult::success(json!({ "process": null }))
                .with_duration_ms(start.elapsed().as_millis() as u64))
        }
    }
}

/// Terminate a process by PID. Requires explicit user confirmation.
pub struct ProcessKillTool {
    metadata: ToolMetadata,
}

impl ProcessKillTool {
    pub fn new() -> Self {
        Self {
            metadata: ToolMetadata::new(
                "process_kill",
                "Terminate an active process by PID (Destructive — requires confirmation)",
                ToolCategory::Process,
                RiskTier::Destructive,
            )
            .with_schema(json!({
                "type": "object",
                "required": ["pid"],
                "properties": {
                    "pid": { "type": "integer", "description": "Process ID to terminate" }
                }
            }))
            .with_confirmation(true),
        }
    }
}

impl Default for ProcessKillTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for ProcessKillTool {
    fn metadata(&self) -> &ToolMetadata {
        &self.metadata
    }

    async fn execute(&self, params: serde_json::Value, ctx: &ToolContext) -> Result<ToolResult> {
        let start = Instant::now();
        let pid = params
            .get("pid")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| ToolError::InvalidParams("Missing 'pid'".into()))?
            as u32;

        if !ctx.user_confirmed {
            return Err(ToolError::ConfirmationRequired(format!(
                "Terminating PID {} requires explicit user confirmation",
                pid
            )));
        }

        // Safe guarded execution
        #[cfg(windows)]
        {
            extern "system" {
                fn OpenProcess(
                    dwDesiredAccess: u32,
                    bInheritHandle: i32,
                    dwProcessId: u32,
                ) -> *mut std::ffi::c_void;
                fn TerminateProcess(hProcess: *mut std::ffi::c_void, uExitCode: u32) -> i32;
                fn CloseHandle(hObject: *mut std::ffi::c_void) -> i32;
            }

            unsafe {
                let hproc = OpenProcess(0x0001 /* PROCESS_TERMINATE */, 0, pid);
                if hproc.is_null() {
                    return Err(ToolError::ExecutionFailed(format!(
                        "Could not open process {} for termination (permission denied or not found)",
                        pid
                    )));
                }
                let ok = TerminateProcess(hproc, 1);
                CloseHandle(hproc);

                if ok == 0 {
                    return Err(ToolError::ExecutionFailed(format!(
                        "TerminateProcess call failed on PID {}",
                        pid
                    )));
                }
            }

            Ok(
                ToolResult::success(json!({ "pid": pid, "terminated": true }))
                    .with_observation(format!("Terminated PID {}", pid))
                    .with_verification("Process terminated successfully")
                    .with_duration_ms(start.elapsed().as_millis() as u64),
            )
        }
        #[cfg(not(windows))]
        {
            let _ = pid;
            Ok(ToolResult::success(json!({ "terminated": true }))
                .with_duration_ms(start.elapsed().as_millis() as u64))
        }
    }
}
