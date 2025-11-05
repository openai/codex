use wasm_bindgen::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// When the `wee_alloc` feature is enabled, use `wee_alloc` as the global
// allocator.
#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}

macro_rules! console_log {
    ($($t:tt)*) => (log(&format_args!($($t)*).to_string()))
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CodexCommand {
    pub command: String,
    pub args: Vec<String>,
    pub cwd: Option<String>,
    pub env: Option<HashMap<String, String>>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CodexResult {
    pub success: bool,
    pub output: String,
    pub error: Option<String>,
    pub exit_code: Option<i32>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ConversationData {
    pub id: String,
    pub messages: Vec<ConversationMessage>,
    pub model: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ConversationMessage {
    pub role: String,
    pub content: String,
    pub timestamp: String,
}

#[wasm_bindgen]
pub struct WasmCodex {
    conversations: HashMap<String, ConversationData>,
    current_conversation: Option<String>,
}

#[wasm_bindgen]
impl WasmCodex {
    #[wasm_bindgen(constructor)]
    pub fn new() -> WasmCodex {
        console_error_panic_hook::set_once();
        console_log!("WasmCodex initialized");

        WasmCodex {
            conversations: HashMap::new(),
            current_conversation: None,
        }
    }

    #[wasm_bindgen]
    pub fn execute_command(&self, command: &str, args: JsValue, cwd: Option<String>) -> Result<JsValue, JsValue> {
        console_log!("Executing command: {} with args: {:?}", command, args);

        let args: Vec<String> = serde_wasm_bindgen::from_value(args)
            .map_err(|e| JsValue::from_str(&format!("Failed to parse args: {}", e)))?;

        // Simulate command execution (in real implementation, this would call actual Codex CLI)
        let result = match command {
            "codex" => {
                if args.contains(&"version".to_string()) || args.contains(&"--version".to_string()) {
                    CodexResult {
                        success: true,
                        output: "Codex CLI v0.52.0 (WebAssembly)".to_string(),
                        error: None,
                        exit_code: Some(0),
                    }
                } else if args.contains(&"help".to_string()) || args.contains(&"--help".to_string()) {
                    CodexResult {
                        success: true,
                        output: r#"Codex - AI Assistant Platform

USAGE:
    codex [OPTIONS] [SUBCOMMAND]

OPTIONS:
    -h, --help       Print help information
    -v, --version    Print version information

SUBCOMMANDS:
    new         Create a new conversation
    list        List conversations
    resume      Resume a conversation
    agent       Manage AI agents
    research    Deep research functionality
    help        Print this message or the help of the given subcommand(s)
"#.to_string(),
                        error: None,
                        exit_code: Some(0),
                    }
                } else {
                    CodexResult {
                        success: false,
                        output: "".to_string(),
                        error: Some("Unknown command or arguments".to_string()),
                        exit_code: Some(1),
                    }
                }
            }
            _ => {
                CodexResult {
                    success: false,
                    output: "".to_string(),
                    error: Some(format!("Command '{}' not found", command)),
                    exit_code: Some(127),
                }
            }
        };

        Ok(serde_wasm_bindgen::to_value(&result)?)
    }

    #[wasm_bindgen]
    pub fn create_conversation(&mut self, model: &str) -> Result<JsValue, JsValue> {
        let id = format!("conv_{}", js_sys::Date::now() as u64);
        let conversation = ConversationData {
            id: id.clone(),
            messages: vec![],
            model: model.to_string(),
            created_at: js_sys::Date::now().to_string(),
            updated_at: js_sys::Date::now().to_string(),
        };

        self.conversations.insert(id.clone(), conversation.clone());
        self.current_conversation = Some(id);

        console_log!("Created conversation: {}", conversation.id);
        Ok(serde_wasm_bindgen::to_value(&conversation)?)
    }

    #[wasm_bindgen]
    pub fn send_message(&mut self, content: &str) -> Result<JsValue, JsValue> {
        let conv_id = self.current_conversation.as_ref()
            .ok_or_else(|| JsValue::from_str("No active conversation"))?;

        let conversation = self.conversations.get_mut(conv_id)
            .ok_or_else(|| JsValue::from_str("Conversation not found"))?;

        let user_message = ConversationMessage {
            role: "user".to_string(),
            content: content.to_string(),
            timestamp: js_sys::Date::now().to_string(),
        };

        conversation.messages.push(user_message.clone());

        // Simulate AI response
        let ai_response = ConversationMessage {
            role: "assistant".to_string(),
            content: format!("こんにちは！ {} についてお手伝いしますね。", content),
            timestamp: js_sys::Date::now().to_string(),
        };

        conversation.messages.push(ai_response.clone());
        conversation.updated_at = js_sys::Date::now().to_string();

        console_log!("Sent message to conversation: {}", conv_id);
        Ok(serde_wasm_bindgen::to_value(&conversation.messages)?)
    }

    #[wasm_bindgen]
    pub fn get_conversations(&self) -> Result<JsValue, JsValue> {
        let conversations: Vec<&ConversationData> = self.conversations.values().collect();
        Ok(serde_wasm_bindgen::to_value(&conversations)?)
    }

    #[wasm_bindgen]
    pub fn get_current_conversation(&self) -> Result<JsValue, JsValue> {
        if let Some(conv_id) = &self.current_conversation {
            if let Some(conversation) = self.conversations.get(conv_id) {
                return Ok(serde_wasm_bindgen::to_value(conversation)?);
            }
        }
        Ok(JsValue::NULL)
    }

    #[wasm_bindgen]
    pub fn switch_conversation(&mut self, conversation_id: &str) -> Result<(), JsValue> {
        if self.conversations.contains_key(conversation_id) {
            self.current_conversation = Some(conversation_id.to_string());
            console_log!("Switched to conversation: {}", conversation_id);
            Ok(())
        } else {
            Err(JsValue::from_str("Conversation not found"))
        }
    }

    #[wasm_bindgen]
    pub fn run_agent(&self, agent_type: &str, context: JsValue) -> Result<JsValue, JsValue> {
        console_log!("Running agent: {} with context: {:?}", agent_type, context);

        // Simulate agent execution
        let result = match agent_type {
            "code-reviewer" => {
                serde_json::json!({
                    "agent": "code-reviewer",
                    "status": "completed",
                    "findings": [
                        {
                            "type": "warning",
                            "message": "未使用の変数があります",
                            "line": 42
                        }
                    ]
                })
            },
            "test-generator" => {
                serde_json::json!({
                    "agent": "test-generator",
                    "status": "completed",
                    "tests": [
                        {
                            "name": "test_function_returns_expected_value",
                            "code": "assert function() == expected_value"
                        }
                    ]
                })
            },
            "security-audit" => {
                serde_json::json!({
                    "agent": "security-audit",
                    "status": "completed",
                    "vulnerabilities": [],
                    "score": 95
                })
            },
            "researcher" => {
                serde_json::json!({
                    "agent": "researcher",
                    "status": "completed",
                    "sources": [
                        {
                            "title": "Latest Research on Topic",
                            "url": "https://example.com",
                            "confidence": 0.85
                        }
                    ],
                    "summary": "研究結果の要約"
                })
            },
            _ => {
                serde_json::json!({
                    "agent": agent_type,
                    "status": "error",
                    "error": "Unknown agent type"
                })
            }
        };

        Ok(serde_wasm_bindgen::to_value(&result)?)
    }

    #[wasm_bindgen]
    pub fn save_to_specstory(&self, data: JsValue) -> Result<(), JsValue> {
        // In a real implementation, this would save to specstory
        console_log!("Saving to specstory: {:?}", data);

        // Simulate saving
        let specstory_data = serde_wasm_bindgen::from_value::<serde_json::Value>(data)
            .map_err(|e| JsValue::from_str(&format!("Failed to parse data: {}", e)))?;

        // Here we would normally save to specstory service
        // For now, just log it
        console_log!("Specstory data: {}", serde_json::to_string_pretty(&specstory_data).unwrap());

        Ok(())
    }

    #[wasm_bindgen]
    pub fn load_from_specstory(&self, id: &str) -> Result<JsValue, JsValue> {
        // In a real implementation, this would load from specstory
        console_log!("Loading from specstory: {}", id);

        // Simulate loading
        let mock_data = serde_json::json!({
            "id": id,
            "type": "conversation",
            "data": {
                "messages": [
                    {
                        "role": "user",
                        "content": "Hello",
                        "timestamp": "2025-01-01T00:00:00Z"
                    }
                ]
            }
        });

        Ok(serde_wasm_bindgen::to_value(&mock_data)?)
    }
}

#[wasm_bindgen(start)]
pub fn main() {
    console_log!("Codex WASM CLI loaded");
}

#[wasm_bindgen]
pub fn greet(name: &str) -> String {
    format!("Hello, {}! Welcome to Codex WebAssembly CLI!", name)
}

#[wasm_bindgen]
pub fn get_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}
