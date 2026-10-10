//! IDE Launch and Integration for OSMOO.
//!
//! Detects installed IDEs (VS Code, Cursor, Windsurf, JetBrains) on Windows,
//! and opens projects or specific files at target line numbers.

use std::path::Path;
use std::process::Command;
use thiserror::Error;
use tracing::info;

#[derive(Debug, Error)]
pub enum IdeError {
    #[error("IDE executable not found: {0}")]
    NotFound(String),

    #[error("Failed to launch IDE process: {0}")]
    LaunchFailed(#[from] std::io::Error),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SupportedIde {
    VSCode,
    Cursor,
    Windsurf,
    IntelliJ,
}

impl SupportedIde {
    pub fn executable_name(&self) -> &'static str {
        match self {
            Self::VSCode => "code",
            Self::Cursor => "cursor",
            Self::Windsurf => "windsurf",
            Self::IntelliJ => "idea64",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Self::VSCode => "Visual Studio Code",
            Self::Cursor => "Cursor",
            Self::Windsurf => "Windsurf",
            Self::IntelliJ => "IntelliJ IDEA",
        }
    }
}

pub struct IdeLauncher;

impl IdeLauncher {
    /// Detects available IDEs installed on the system PATH or standard locations.
    pub fn detect_available() -> Vec<SupportedIde> {
        let mut available = Vec::new();
        for &ide in &[
            SupportedIde::Cursor,
            SupportedIde::VSCode,
            SupportedIde::Windsurf,
            SupportedIde::IntelliJ,
        ] {
            if Self::is_available(ide) {
                available.push(ide);
            }
        }
        available
    }

    /// Checks if a specific IDE is accessible via command or standard Windows paths.
    pub fn is_available(ide: SupportedIde) -> bool {
        #[cfg(windows)]
        {
            if let Ok(output) = Command::new("where.exe")
                .arg(ide.executable_name())
                .output()
            {
                if output.status.success() {
                    return true;
                }
            }

            // Check common AppData/Local/Programs installs on Windows
            if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
                let local = Path::new(&local_app_data);
                let check_path = match ide {
                    SupportedIde::VSCode => local.join("Programs\\Microsoft VS Code\\Code.exe"),
                    SupportedIde::Cursor => local.join("Programs\\cursor\\Cursor.exe"),
                    SupportedIde::Windsurf => local.join("Programs\\Windsurf\\Windsurf.exe"),
                    SupportedIde::IntelliJ => local.join("Programs\\IntelliJ IDEA Ultimate\\bin\\idea64.exe"),
                };
                if check_path.exists() {
                    return true;
                }
            }
        }
        false
    }

    /// Opens the specified folder in the preferred or first available IDE.
    pub fn open_workspace(
        workspace_path: impl AsRef<Path>,
        preferred_ide: Option<SupportedIde>,
    ) -> Result<SupportedIde, IdeError> {
        let ide = preferred_ide
            .or_else(|| Self::detect_available().first().copied())
            .unwrap_or(SupportedIde::VSCode);

        let path_str = workspace_path.as_ref().to_string_lossy().to_string();

        info!("Opening workspace {} in {}", path_str, ide.display_name());

        #[cfg(windows)]
        {
            Command::new("cmd")
                .args(["/c", ide.executable_name(), &path_str])
                .spawn()?;
        }
        #[cfg(not(windows))]
        {
            Command::new(ide.executable_name())
                .arg(&path_str)
                .spawn()?;
        }

        Ok(ide)
    }

    /// Opens a specific file optionally focused at line/column.
    pub fn open_file(
        file_path: impl AsRef<Path>,
        line: Option<usize>,
        column: Option<usize>,
        preferred_ide: Option<SupportedIde>,
    ) -> Result<SupportedIde, IdeError> {
        let ide = preferred_ide
            .or_else(|| Self::detect_available().first().copied())
            .unwrap_or(SupportedIde::VSCode);

        let path_str = file_path.as_ref().to_string_lossy().to_string();
        let target_arg = match (line, column) {
            (Some(l), Some(c)) => format!("{}:{}:{}", path_str, l, c),
            (Some(l), None) => format!("{}:{}", path_str, l),
            _ => path_str,
        };

        info!("Opening file {} in {}", target_arg, ide.display_name());

        #[cfg(windows)]
        {
            let mut cmd = Command::new("cmd");
            cmd.args(["/c", ide.executable_name(), "-g", &target_arg]);
            cmd.spawn()?;
        }
        #[cfg(not(windows))]
        {
            let mut cmd = Command::new(ide.executable_name());
            cmd.args(["-g", &target_arg]);
            cmd.spawn()?;
        }

        Ok(ide)
    }
}
