use std::collections::HashMap;

use codex_core::config::Config;
use codex_core::protocol::Event;
use codex_core::protocol::EventMsg;
use serde_json::json;

use crate::event_processor::EventProcessor;
use crate::event_processor::create_config_summary_entries;

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

    fn process_event(&mut self, event: Event) {
        match event.msg {
            EventMsg::AgentMessageDelta(_) | EventMsg::AgentReasoningDelta(_) => {
                // Suppress streaming events in JSON mode.
            }
            _ => {
                if let Ok(line) = serde_json::to_string(&event) {
                    println!("{line}");
                }
            }
        }
    }
}
