// DEV TEST INPUT — Terminal-based text input for VOXY
//
// This module provides a terminal-based text input path for testing the
// LLM → TTS → speaker pipeline with full ConversationMemory, SQLite persistence,
// and ToolRegistry support.
//
// Usage: Run the daemon with `--dev-text` flag.

use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::{mpsc, Mutex};
use tracing::info;
use voxy_provider_core::LlmProvider;
use voxy_security::SystemPromptBuilder;
use voxy_voice::VoicePipeline;

use crate::tools::ToolRegistry;
use crate::ConversationMemory;

pub async fn run(
    llm: Arc<dyn LlmProvider>,
    pipeline: Arc<VoicePipeline>,
    memory: Arc<Mutex<ConversationMemory>>,
    system_prompt: Arc<String>,
    tool_reg: Option<Arc<ToolRegistry>>,
) {
    info!("[DEV-TEXT] Developer text input active. Type messages at the prompt.");
    info!("[DEV-TEXT] Type 'quit' or 'exit' to stop.");

    let stdin = tokio::io::stdin();
    let reader = BufReader::new(stdin);
    let mut lines = reader.lines();

    loop {
        eprint!("VOXY > ");
        match lines.next_line().await {
            Ok(Some(line)) => {
                let text = line.trim().to_string();
                if text.is_empty() {
                    continue;
                }
                if text == "quit" || text == "exit" {
                    info!("[DEV-TEXT] Exiting dev text input.");
                    break;
                }

                info!("[DEV-TEXT] Input: {}", text);

                // 1. Store user turn in persistent memory
                let history = {
                    let mut mem = memory.lock().await;
                    mem.add_turn("user", &text);
                    mem.conversation_history()
                };

                // 2. Build full prompt with conversation history
                let prompt = format!(
                    "{}\n\nRecent conversation:\n{}\n\n{}",
                    system_prompt,
                    if history.is_empty() {
                        "No prior conversation.".to_string()
                    } else {
                        history
                    },
                    SystemPromptBuilder::format_user_message(&text)
                );

                if llm.supports_streaming() {
                    let (tx, mut rx) = mpsc::channel::<String>(4);
                    let llm_clone = llm.clone();
                    let prompt_clone = prompt.clone();

                    tokio::spawn(async move {
                        let (llm_tx, mut llm_rx) =
                            mpsc::channel::<voxy_provider_core::LlmChunk>(16);
                        let llm = llm_clone;
                        let prompt = prompt_clone;
                        tokio::spawn(async move {
                            let _ = llm.complete_streaming(&prompt, llm_tx).await;
                        });

                        let mut sentence_buffer = String::new();
                        let mut accumulated = String::new();

                        while let Some(chunk) = llm_rx.recv().await {
                            if chunk.done {
                                if !sentence_buffer.trim().is_empty() {
                                    let _ = tx.send(sentence_buffer.trim().to_string()).await;
                                }
                                break;
                            }
                            sentence_buffer.push_str(&chunk.text);
                            while let Some(pos) = sentence_buffer
                                .char_indices()
                                .find(|&(_, c)| c == '.' || c == '!' || c == '?' || c == '\n')
                                .map(|(i, _)| i)
                            {
                                let end = pos + 1;
                                let sentence: String = sentence_buffer.drain(..end).collect();
                                let trimmed = sentence.trim().to_string();
                                if !trimmed.is_empty() {
                                    let _ = tx.send(trimmed).await;
                                }
                            }
                            accumulated.push_str(&chunk.text);
                        }
                    });

                    let mut full_response = String::new();
                    while let Some(sentence) = rx.recv().await {
                        full_response.push_str(&sentence);
                        full_response.push(' ');
                        if sentence.trim().starts_with("TOOL:") {
                            continue; // Don't speak raw tool JSON to the user!
                        }
                        info!("[DEV-TEXT] Speaking: {}", sentence);
                        if let Err(e) = pipeline.speak(&sentence).await {
                            info!("[DEV-TEXT] TTS error: {e}");
                        }
                    }

                    // Check for tool call
                    let cleaned = full_response.trim().to_string();
                    if let Some(ref tr) = tool_reg {
                        if let Some((call, _remaining)) = ToolRegistry::parse_tool_call(&cleaned) {
                            info!("[DEV-TEXT] Executing tool: {} {:?}", call.tool, call.params);
                            let result = tr.execute(&call).await;
                            info!("[DEV-TEXT] Tool result: {}", result.message);
                            if let Err(e) = pipeline.speak(&result.message).await {
                                info!("[DEV-TEXT] Tool TTS error: {e}");
                            }
                        }
                    }

                    // Store assistant turn in persistent memory
                    {
                        let mut mem = memory.lock().await;
                        mem.add_turn("assistant", &cleaned);
                    }
                } else {
                    let raw_response = match llm.complete(&prompt).await {
                        Ok(raw) => {
                            let cleaned = raw
                                .trim()
                                .trim_start_matches("Assistant:")
                                .trim()
                                .to_string();
                            if cleaned.is_empty() {
                                "I'm not sure how to respond to that.".to_string()
                            } else {
                                cleaned
                            }
                        }
                        Err(e) => {
                            info!("[DEV-TEXT] LLM error: {e}");
                            "Sorry, I couldn't process that right now.".to_string()
                        }
                    };

                    let response = if let Some(ref tr) = tool_reg {
                        if let Some((call, remaining)) =
                            ToolRegistry::parse_tool_call(&raw_response)
                        {
                            info!("[DEV-TEXT] Executing tool: {} {:?}", call.tool, call.params);
                            let result = tr.execute(&call).await;
                            if !remaining.is_empty() {
                                format!("{}\n{}", remaining, result.message)
                            } else {
                                result.message
                            }
                        } else {
                            raw_response
                        }
                    } else {
                        raw_response
                    };

                    info!("[DEV-TEXT] Response: {}", response);
                    {
                        let mut mem = memory.lock().await;
                        mem.add_turn("assistant", &response);
                    }
                    if let Err(e) = pipeline.speak(&response).await {
                        info!("[DEV-TEXT] TTS/speak error: {e}");
                    }
                }
            }
            Ok(None) => {
                info!("[DEV-TEXT] stdin closed.");
                break;
            }
            Err(e) => {
                info!("[DEV-TEXT] stdin error: {e}");
                break;
            }
        }
    }
}
