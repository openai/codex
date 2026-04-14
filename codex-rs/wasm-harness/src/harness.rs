use crate::HarnessError;
use crate::responses::HarnessEvent;
use crate::responses::ResponsesFunctionCall;
use crate::responses::ResponsesRequest;
use crate::responses::ResponsesResponse;
use crate::responses::ResponsesTool;
use crate::responses::build_browser_instructions;
use crate::responses::tool_output_item;
use async_trait::async_trait;
use serde_json::Value;

const DEFAULT_MODEL: &str = "gpt-5.1";
const DEFAULT_MAX_TOOL_ROUNDS: usize = 8;

#[derive(Clone, Debug)]
pub struct HarnessConfig {
    pub model: String,
    pub instructions: String,
    pub max_tool_rounds: usize,
}

impl Default for HarnessConfig {
    fn default() -> Self {
        Self {
            model: DEFAULT_MODEL.to_string(),
            instructions: build_browser_instructions(),
            max_tool_rounds: DEFAULT_MAX_TOOL_ROUNDS,
        }
    }
}

pub trait EventSink {
    fn emit(&self, event: &HarnessEvent) -> Result<(), HarnessError>;
}

#[async_trait(?Send)]
pub trait ResponsesClient {
    async fn create_response(
        &self,
        request: ResponsesRequest,
    ) -> Result<ResponsesResponse, HarnessError>;
}

#[async_trait(?Send)]
pub trait ToolExecutor {
    fn tools(&self) -> Vec<ResponsesTool>;

    async fn execute(&self, function_call: &ResponsesFunctionCall) -> Result<String, HarnessError>;
}

pub struct EmbeddedHarness<C, T> {
    config: HarnessConfig,
    responses_client: C,
    tool_executor: T,
    next_turn_id: u32,
}

impl<C, T> EmbeddedHarness<C, T> {
    #[must_use]
    pub fn new(config: HarnessConfig, responses_client: C, tool_executor: T) -> Self {
        Self {
            config,
            responses_client,
            tool_executor,
            next_turn_id: 0,
        }
    }

    pub fn responses_client_mut(&mut self) -> &mut C {
        &mut self.responses_client
    }

    pub fn tool_executor_mut(&mut self) -> &mut T {
        &mut self.tool_executor
    }

    pub fn config_mut(&mut self) -> &mut HarnessConfig {
        &mut self.config
    }
}

impl<C, T> EmbeddedHarness<C, T>
where
    C: ResponsesClient,
    T: ToolExecutor,
{
    pub async fn submit_turn<E: EventSink>(
        &mut self,
        prompt: impl Into<String>,
        event_sink: &E,
    ) -> Result<String, HarnessError> {
        self.next_turn_id += 1;
        let turn_id = format!("browser-turn-{}", self.next_turn_id);
        let prompt = prompt.into();

        event_sink.emit(&HarnessEvent::TurnStarted {
            turn_id: turn_id.clone(),
            model_context_window: None,
            collaboration_mode_kind: "default".to_string(),
        })?;
        event_sink.emit(&HarnessEvent::UserMessage {
            turn_id: turn_id.clone(),
            message: prompt.clone(),
        })?;

        let result = self.run_turn(&turn_id, &prompt, event_sink).await;
        match result {
            Ok(last_agent_message) => {
                event_sink.emit(&HarnessEvent::TurnComplete {
                    turn_id,
                    last_agent_message: Some(last_agent_message.clone()),
                })?;
                Ok(last_agent_message)
            }
            Err(err) => {
                event_sink.emit(&HarnessEvent::TurnError {
                    turn_id: turn_id.clone(),
                    message: err.to_string(),
                })?;
                event_sink.emit(&HarnessEvent::TurnComplete {
                    turn_id,
                    last_agent_message: None,
                })?;
                Err(err)
            }
        }
    }

    async fn run_turn<E: EventSink>(
        &self,
        turn_id: &str,
        prompt: &str,
        event_sink: &E,
    ) -> Result<String, HarnessError> {
        let mut previous_response_id: Option<String> = None;
        let mut input = Value::String(prompt.to_string());
        let mut last_agent_message: Option<String> = None;

        for round in 0..self.config.max_tool_rounds {
            let tools = self.tool_executor.tools();
            let request = ResponsesRequest {
                model: self.config.model.clone(),
                instructions: self.config.instructions.clone(),
                input,
                previous_response_id: previous_response_id.clone(),
                tools: (!tools.is_empty()).then_some(tools),
                parallel_tool_calls: false,
            };
            let response = self.responses_client.create_response(request).await?;

            previous_response_id = response.id.clone();

            let agent_message = response.response_text();
            if !agent_message.is_empty() {
                event_sink.emit(&HarnessEvent::AgentMessageDelta {
                    turn_id: turn_id.to_string(),
                    delta: agent_message.clone(),
                })?;
                event_sink.emit(&HarnessEvent::AgentMessage {
                    turn_id: turn_id.to_string(),
                    message: agent_message.clone(),
                })?;
                last_agent_message = Some(agent_message);
            }

            let function_calls = response.function_calls()?;
            if function_calls.is_empty() {
                return Ok(last_agent_message.unwrap_or_else(|| {
                    "Responses API returned no assistant message.".to_string()
                }));
            }

            let response_id = previous_response_id.clone().ok_or_else(|| {
                HarnessError::new("Responses API omitted response.id for a tool-calling turn")
            })?;

            let mut tool_outputs = Vec::with_capacity(function_calls.len());
            for function_call in function_calls {
                event_sink.emit(&HarnessEvent::ToolCallStarted {
                    turn_id: turn_id.to_string(),
                    response_id: response_id.clone(),
                    call_id: function_call.call_id.clone(),
                    name: function_call.name.clone(),
                    arguments: function_call.arguments.clone(),
                })?;

                let output = self.tool_executor.execute(&function_call).await?;
                event_sink.emit(&HarnessEvent::ToolCallCompleted {
                    turn_id: turn_id.to_string(),
                    response_id: response_id.clone(),
                    call_id: function_call.call_id.clone(),
                    name: function_call.name.clone(),
                    output: output.clone(),
                })?;
                tool_outputs.push(tool_output_item(&function_call.call_id, output));
            }

            input = Value::Array(tool_outputs);

            if round + 1 == self.config.max_tool_rounds {
                return Err(HarnessError::new(
                    "turn exceeded the browser tool-round limit",
                ));
            }
        }

        Err(HarnessError::new("browser turn loop exited unexpectedly"))
    }
}

#[cfg(test)]
mod tests {
    use super::EmbeddedHarness;
    use super::EventSink;
    use super::HarnessConfig;
    use super::ResponsesClient;
    use super::ToolExecutor;
    use crate::HarnessError;
    use crate::responses::HarnessEvent;
    use crate::responses::ResponsesFunctionCall;
    use crate::responses::ResponsesRequest;
    use crate::responses::ResponsesResponse;
    use crate::responses::ResponsesTool;
    use async_trait::async_trait;
    use pretty_assertions::assert_eq;
    use std::cell::RefCell;
    use std::collections::VecDeque;

    struct RecordingEventSink {
        events: RefCell<Vec<HarnessEvent>>,
    }

    impl RecordingEventSink {
        fn new() -> Self {
            Self {
                events: RefCell::new(Vec::new()),
            }
        }

        fn events(&self) -> Vec<HarnessEvent> {
            self.events.borrow().clone()
        }
    }

    impl EventSink for RecordingEventSink {
        fn emit(&self, event: &HarnessEvent) -> Result<(), HarnessError> {
            self.events.borrow_mut().push(event.clone());
            Ok(())
        }
    }

    struct FakeResponsesClient {
        responses: RefCell<VecDeque<ResponsesResponse>>,
        requests: RefCell<Vec<ResponsesRequest>>,
    }

    impl FakeResponsesClient {
        fn new(responses: Vec<ResponsesResponse>) -> Self {
            Self {
                responses: RefCell::new(responses.into()),
                requests: RefCell::new(Vec::new()),
            }
        }
    }

    #[async_trait(?Send)]
    impl ResponsesClient for FakeResponsesClient {
        async fn create_response(
            &self,
            request: ResponsesRequest,
        ) -> Result<ResponsesResponse, HarnessError> {
            self.requests.borrow_mut().push(request);
            self.responses
                .borrow_mut()
                .pop_front()
                .ok_or_else(|| HarnessError::new("no fake response available"))
        }
    }

    struct FakeToolExecutor;

    #[async_trait(?Send)]
    impl ToolExecutor for FakeToolExecutor {
        fn tools(&self) -> Vec<ResponsesTool> {
            vec![ResponsesTool::exec_js()]
        }

        async fn execute(
            &self,
            function_call: &ResponsesFunctionCall,
        ) -> Result<String, HarnessError> {
            assert_eq!(function_call.name, "exec_js");
            Ok("Hello, world!".to_string())
        }
    }

    #[tokio::test]
    async fn embedded_harness_completes_tool_turn() {
        let responses = vec![
            serde_json::from_str(
                r#"{
                    "id": "resp_1",
                    "output": [
                        {
                            "type": "function_call",
                            "call_id": "call_1",
                            "name": "exec_js",
                            "arguments": "{\"code\":\"console.log('Hello, world!')\"}"
                        }
                    ]
                }"#,
            )
            .expect("response should deserialize"),
            serde_json::from_str(
                r#"{
                    "id": "resp_2",
                    "output_text": "Done."
                }"#,
            )
            .expect("response should deserialize"),
        ];
        let client = FakeResponsesClient::new(responses);
        let tool_executor = FakeToolExecutor;
        let sink = RecordingEventSink::new();
        let mut harness = EmbeddedHarness::new(HarnessConfig::default(), client, tool_executor);

        let result = harness
            .submit_turn("write hello world", &sink)
            .await
            .expect("turn should succeed");

        assert_eq!(result, "Done.");
        assert_eq!(
            sink.events(),
            vec![
                HarnessEvent::TurnStarted {
                    turn_id: "browser-turn-1".to_string(),
                    model_context_window: None,
                    collaboration_mode_kind: "default".to_string(),
                },
                HarnessEvent::UserMessage {
                    turn_id: "browser-turn-1".to_string(),
                    message: "write hello world".to_string(),
                },
                HarnessEvent::ToolCallStarted {
                    turn_id: "browser-turn-1".to_string(),
                    response_id: "resp_1".to_string(),
                    call_id: "call_1".to_string(),
                    name: "exec_js".to_string(),
                    arguments: r#"{"code":"console.log('Hello, world!')"}"#.to_string(),
                },
                HarnessEvent::ToolCallCompleted {
                    turn_id: "browser-turn-1".to_string(),
                    response_id: "resp_1".to_string(),
                    call_id: "call_1".to_string(),
                    name: "exec_js".to_string(),
                    output: "Hello, world!".to_string(),
                },
                HarnessEvent::AgentMessageDelta {
                    turn_id: "browser-turn-1".to_string(),
                    delta: "Done.".to_string(),
                },
                HarnessEvent::AgentMessage {
                    turn_id: "browser-turn-1".to_string(),
                    message: "Done.".to_string(),
                },
                HarnessEvent::TurnComplete {
                    turn_id: "browser-turn-1".to_string(),
                    last_agent_message: Some("Done.".to_string()),
                },
            ]
        );
    }
}
