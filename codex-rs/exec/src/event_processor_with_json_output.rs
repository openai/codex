use std::collections::HashMap;
use std::path::Path;

use codex_core::config::Config;
use codex_core::protocol::Event;
use codex_core::protocol::EventMsg;
use codex_core::protocol::TaskCompleteEvent;
use serde_json::json;

use crate::event_processor::EventProcessor;
use crate::event_processor::create_config_summary_entries;
use crate::event_processor_with_human_output::CodexStatus;

pub(crate) struct EventProcessorWithJsonOutput;

impl EventProcessorWithJsonOutput {
    pub fn new() -> Self {
        Self {}
    }
}

impl EventProcessor for EventProcessorWithJsonOutput {
    fn print_config_summary(&mut self, config: &Config, prompt: &str) {
        let entries = create_config_summary_entries(config)
            .into_iter()
            .map(|(key, value)| (key.to_string(), value))
            .collect::<HashMap<String, String>>();
        #[allow(clippy::expect_used)]
        let config_json =
            serde_json::to_string(&entries).expect("Failed to serialize config summary to JSON");
        println!("{config_json}");

        let prompt_json = json!({
            "prompt": prompt,
        });
        println!("{prompt_json}");
    }

    fn process_event(&mut self, event: Event, last_message_file: Option<&Path>) -> CodexStatus {
        match event.msg {
            EventMsg::AgentMessageDelta(_) | EventMsg::AgentReasoningDelta(_) => {
                // Suppress streaming events in JSON mode.
                CodexStatus::Running
            }
            EventMsg::TaskComplete(TaskCompleteEvent { last_agent_message }) => {
                match (last_agent_message, last_message_file) {
                    (Some(last_agent_message), Some(last_message_file)) => {
                        // Last message and a file to write to.
                        if let Err(e) = std::fs::write(last_message_file, last_agent_message) {
                            eprintln!("Error writing last message to file: {e}");
                        }
                    }
                    (None, Some(last_message_file)) => {
                        eprintln!(
                            "Warning: No last message to write to file: {}",
                            last_message_file.to_string_lossy()
                        );
                    }
                    (_, None) => {
                        // No last message and no file to write to.
                    }
                }
                CodexStatus::InitiateShutdown
            }
            EventMsg::Shutdown => CodexStatus::Shutdown,
            _ => {
                if let Ok(line) = serde_json::to_string(&event) {
                    println!("{line}");
                }
                CodexStatus::Running
            }
        }
    }
}
