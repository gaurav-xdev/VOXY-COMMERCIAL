#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SystemQueryType {
    CurrentTime,
    CurrentDate,
    BatteryLevel,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DesktopCommand {
    LockWorkstation,
    ShowDesktop,
    MinimizeAll,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FastIntentAction {
    StopSpeech,
    VolumeChange { delta: i32 },
    VolumeMute { mute: bool },
    OpenApplication { app_name: String, executable: String },
    SystemQuery(SystemQueryType),
    DesktopAction(DesktopCommand),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FastIntentResult {
    pub action: FastIntentAction,
    pub immediate_verbal_response: Option<String>,
}

pub struct IntentPreprocessor;

impl IntentPreprocessor {
    /// Inspects a raw transcription text and checks if it matches a fast deterministic intent.
    /// Returns Some(FastIntentResult) if matched, or None if it should be routed to an LLM.
    pub fn classify(raw_text: &str) -> Option<FastIntentResult> {
        let cleaned = Self::normalize(raw_text);
        if cleaned.is_empty() {
            return None;
        }

        // 1. Emergency stop / cancel
        if matches!(
            cleaned.as_str(),
            "stop" | "cancel" | "shut up" | "be quiet" | "pause" | "halt" | "exit" | "nevermind"
        ) {
            return Some(FastIntentResult {
                action: FastIntentAction::StopSpeech,
                immediate_verbal_response: Some("Stopping.".into()),
            });
        }

        // 2. Volume controls
        if cleaned == "mute" || cleaned == "mute audio" || cleaned == "mute volume" {
            return Some(FastIntentResult {
                action: FastIntentAction::VolumeMute { mute: true },
                immediate_verbal_response: Some("Muted.".into()),
            });
        }
        if cleaned == "unmute" || cleaned == "unmute audio" || cleaned == "unmute volume" {
            return Some(FastIntentResult {
                action: FastIntentAction::VolumeMute { mute: false },
                immediate_verbal_response: Some("Unmuted.".into()),
            });
        }
        if cleaned.contains("volume up") || cleaned.contains("increase volume") || cleaned.contains("louder") {
            return Some(FastIntentResult {
                action: FastIntentAction::VolumeChange { delta: 10 },
                immediate_verbal_response: Some("Volume raised.".into()),
            });
        }
        if cleaned.contains("volume down") || cleaned.contains("decrease volume") || cleaned.contains("quieter") || cleaned.contains("lower volume") {
            return Some(FastIntentResult {
                action: FastIntentAction::VolumeChange { delta: -10 },
                immediate_verbal_response: Some("Volume lowered.".into()),
            });
        }

        // 3. System time / date queries
        if cleaned == "what time is it"
            || cleaned == "what's the time"
            || cleaned == "current time"
            || cleaned == "tell me the time"
            || cleaned == "time please"
        {
            return Some(FastIntentResult {
                action: FastIntentAction::SystemQuery(SystemQueryType::CurrentTime),
                immediate_verbal_response: None, // Generated dynamically with system clock
            });
        }
        if cleaned == "what day is it"
            || cleaned == "what's the date"
            || cleaned == "today's date"
            || cleaned == "what is today's date"
            || cleaned == "tell me the date"
        {
            return Some(FastIntentResult {
                action: FastIntentAction::SystemQuery(SystemQueryType::CurrentDate),
                immediate_verbal_response: None,
            });
        }

        // 4. Desktop actions
        if cleaned == "lock screen" || cleaned == "lock computer" || cleaned == "lock pc" || cleaned == "lock workstation" {
            return Some(FastIntentResult {
                action: FastIntentAction::DesktopAction(DesktopCommand::LockWorkstation),
                immediate_verbal_response: Some("Locking workstation.".into()),
            });
        }
        if cleaned == "show desktop" || cleaned == "minimize all" || cleaned == "minimize everything" {
            return Some(FastIntentResult {
                action: FastIntentAction::DesktopAction(DesktopCommand::ShowDesktop),
                immediate_verbal_response: Some("Showing desktop.".into()),
            });
        }

        // 5. App launches ("open ...", "launch ...", "start ...")
        let prefixes = ["open ", "launch ", "start "];
        for prefix in prefixes {
            if let Some(target) = cleaned.strip_prefix(prefix) {
                let target = target.trim();
                if let Some(action) = Self::match_app(target) {
                    let app_display = match &action {
                        FastIntentAction::OpenApplication { app_name, .. } => app_name.clone(),
                        _ => target.to_string(),
                    };
                    return Some(FastIntentResult {
                        action,
                        immediate_verbal_response: Some(format!("Opening {}.", app_display)),
                    });
                }
            }
        }

        None
    }

    fn match_app(target: &str) -> Option<FastIntentAction> {
        let (name, exe) = match target {
            "chrome" | "google chrome" | "browser" => ("Chrome", "chrome.exe"),
            "edge" | "microsoft edge" => ("Edge", "msedge.exe"),
            "firefox" | "mozilla firefox" => ("Firefox", "firefox.exe"),
            "notepad" | "text editor" => ("Notepad", "notepad.exe"),
            "calculator" | "calc" => ("Calculator", "calc.exe"),
            "terminal" | "cmd" | "command prompt" => ("Terminal", "wt.exe"),
            "powershell" => ("PowerShell", "powershell.exe"),
            "settings" | "windows settings" => ("Settings", "ms-settings:"),
            "task manager" => ("Task Manager", "taskmgr.exe"),
            "file explorer" | "explorer" | "my computer" => ("File Explorer", "explorer.exe"),
            "spotify" => ("Spotify", "spotify.exe"),
            "vs code" | "vscode" | "code" => ("VS Code", "code.cmd"),
            _ => return None,
        };

        Some(FastIntentAction::OpenApplication {
            app_name: name.to_string(),
            executable: exe.to_string(),
        })
    }

    fn normalize(text: &str) -> String {
        text.trim()
            .to_lowercase()
            .chars()
            .filter(|c| c.is_alphanumeric() || c.is_whitespace() || *c == '\'')
            .collect::<String>()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_stop() {
        let res = IntentPreprocessor::classify("Stop!").unwrap();
        assert_eq!(res.action, FastIntentAction::StopSpeech);
    }

    #[test]
    fn test_classify_volume() {
        let up = IntentPreprocessor::classify("Please volume up!").unwrap();
        assert_eq!(up.action, FastIntentAction::VolumeChange { delta: 10 });

        let mute = IntentPreprocessor::classify("mute").unwrap();
        assert_eq!(mute.action, FastIntentAction::VolumeMute { mute: true });
    }

    #[test]
    fn test_classify_apps() {
        let chrome = IntentPreprocessor::classify("open chrome").unwrap();
        assert_eq!(
            chrome.action,
            FastIntentAction::OpenApplication {
                app_name: "Chrome".into(),
                executable: "chrome.exe".into()
            }
        );
        assert_eq!(chrome.immediate_verbal_response.unwrap(), "Opening Chrome.");

        let calc = IntentPreprocessor::classify("launch calculator").unwrap();
        assert_eq!(
            calc.action,
            FastIntentAction::OpenApplication {
                app_name: "Calculator".into(),
                executable: "calc.exe".into()
            }
        );
    }

    #[test]
    fn test_classify_system_queries() {
        let time = IntentPreprocessor::classify("What time is it?").unwrap();
        assert_eq!(
            time.action,
            FastIntentAction::SystemQuery(SystemQueryType::CurrentTime)
        );
    }

    #[test]
    fn test_non_intent_passes_to_llm() {
        assert!(IntentPreprocessor::classify("Can you write a poem about rust?").is_none());
        assert!(IntentPreprocessor::classify("How does quantum computing work?").is_none());
    }

    #[test]
    fn bench_intent_classification_latency() {
        let sample_inputs = [
            "open chrome",
            "what time is it",
            "mute volume",
            "tell me the date",
            "stop please",
            "launch calculator",
            "Can you explain the theory of relativity?",
            "volume up",
            "lock workstation",
            "show desktop",
        ];

        let mut latencies_nanos = Vec::with_capacity(10_000);
        for i in 0..10_000 {
            let input = sample_inputs[i % sample_inputs.len()];
            let start = std::time::Instant::now();
            let _ = IntentPreprocessor::classify(input);
            latencies_nanos.push(start.elapsed().as_nanos());
        }

        latencies_nanos.sort_unstable();
        let p50 = latencies_nanos[latencies_nanos.len() / 2];
        let p99 = latencies_nanos[latencies_nanos.len() * 99 / 100];

        // Classification must execute in under 100 microseconds even in debug mode (p99 < 100,000 ns)
        assert!(
            p99 < 100_000,
            "Intent classification P99 too high: {} ns (P50: {} ns)",
            p99,
            p50
        );
    }
}
