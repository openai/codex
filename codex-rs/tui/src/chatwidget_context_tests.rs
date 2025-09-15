// Integration tests for /context command in chatwidget
use super::*;
use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;
use codex_core::config::Config;
use codex_core::config::ConfigOverrides;
use codex_core::config::ConfigToml;
use codex_core::protocol::{Event, EventMsg, TaskStartedEvent, TokenUsage, TokenUsageInfo};
use codex_protocol::mcp_protocol::ConversationId;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use tokio::sync::mpsc::unbounded_channel;

#[cfg(test)]
mod context_command_tests {
    use super::*;

    fn test_config() -> Config {
        Config::load_from_base_config_with_overrides(
            ConfigToml::default(),
            ConfigOverrides::default(),
            std::env::temp_dir(),
        )
        .expect("config")
    }

    fn make_test_chatwidget() -> (
        ChatWidget,
        tokio::sync::mpsc::UnboundedReceiver<AppEvent>,
        tokio::sync::mpsc::UnboundedReceiver<Op>,
    ) {
        let (tx_raw, rx) = unbounded_channel::<AppEvent>();
        let app_event_tx = AppEventSender::new(tx_raw);
        let (op_tx, op_rx) = unbounded_channel::<Op>();
        let cfg = test_config();
        let bottom = BottomPane::new(BottomPaneParams {
            app_event_tx: app_event_tx.clone(),
            frame_requester: FrameRequester::test_dummy(),
            has_input_focus: true,
            enhanced_keys_supported: false,
            placeholder_text: "Ask Codex to do anything".to_string(),
            disable_paste_burst: false,
        });
        let widget = ChatWidget {
            app_event_tx,
            codex_op_tx: op_tx,
            bottom_pane: bottom,
            active_exec_cell: None,
            config: cfg.clone(),
            initial_user_message: None,
            token_info: None,
            stream: StreamController::new(cfg),
            running_commands: HashMap::new(),
            task_complete_pending: false,
            interrupts: InterruptManager::new(),
            reasoning_buffer: String::new(),
            full_reasoning_buffer: String::new(),
            conversation_id: None,
            frame_requester: FrameRequester::test_dummy(),
            show_welcome_banner: true,
            queued_user_messages: VecDeque::new(),
            suppress_session_configured_redraw: false,
        };
        (widget, rx, op_rx)
    }

    fn drain_history_cells(
        rx: &mut tokio::sync::mpsc::UnboundedReceiver<AppEvent>,
    ) -> Vec<Vec<ratatui::text::Line<'static>>> {
        let mut cells = Vec::new();
        while let Ok(ev) = rx.try_recv() {
            if let AppEvent::InsertHistoryCell(cell) = ev {
                let lines = cell.display_lines(80);
                cells.push(lines);
            }
        }
        cells
    }

    fn lines_to_string(lines: &[ratatui::text::Line<'static>]) -> String {
        let mut s = String::new();
        for line in lines {
            for span in &line.spans {
                s.push_str(&span.content);
            }
            s.push('\n');
        }
        s
    }

    #[test]
    fn test_context_command_dispatches_correctly() {
        let (mut chat, mut rx, _op_rx) = make_test_chatwidget();
        
        // Dispatch the Context command
        chat.dispatch_command(SlashCommand::Context);
        
        // Should emit a history cell
        let cells = drain_history_cells(&mut rx);
        assert!(!cells.is_empty(), "Context command should emit history cell");
        
        // Check the content includes the command header
        let content = lines_to_string(&cells[0]);
        assert!(content.contains("/context"), "Should show /context command");
    }

    #[test]
    fn test_context_command_shows_token_usage() {
        let (mut chat, mut rx, _op_rx) = make_test_chatwidget();
        
        // Set up token info with some usage
        chat.token_info = Some(TokenUsageInfo {
            model_context_window: Some(128000),
            total_token_usage: TokenUsage {
                input_tokens: 1000,
                output_tokens: 500,
                total_tokens: 1500,
                cache_read_tokens: 100,
                cache_write_tokens: 50,
            },
        });
        
        // Dispatch the Context command
        chat.dispatch_command(SlashCommand::Context);
        
        let cells = drain_history_cells(&mut rx);
        let content = lines_to_string(&cells[0]);
        
        // Should show token usage details
        assert!(content.contains("1500"), "Should show total tokens");
        assert!(content.contains("1000"), "Should show input tokens");
        assert!(content.contains("500"), "Should show output tokens");
        assert!(content.contains("100"), "Should show cache read tokens");
        assert!(content.contains("50"), "Should show cache write tokens");
    }

    #[test]
    fn test_context_command_shows_percentage() {
        let (mut chat, mut rx, _op_rx) = make_test_chatwidget();
        
        // Set up token info - 64000 tokens is 50% of 128000
        chat.token_info = Some(TokenUsageInfo {
            model_context_window: Some(128000),
            total_token_usage: TokenUsage {
                input_tokens: 44000,
                output_tokens: 20000,
                total_tokens: 64000,
                cache_read_tokens: 0,
                cache_write_tokens: 0,
            },
        });
        
        // Dispatch the Context command
        chat.dispatch_command(SlashCommand::Context);
        
        let cells = drain_history_cells(&mut rx);
        let content = lines_to_string(&cells[0]);
        
        // Should show percentage
        assert!(content.contains("50"), "Should show 50% usage");
        assert!(content.contains("%"), "Should include percentage sign");
    }

    #[test]
    fn test_context_command_with_no_tokens() {
        let (mut chat, mut rx, _op_rx) = make_test_chatwidget();
        
        // No token info set (fresh session)
        assert!(chat.token_info.is_none());
        
        // Dispatch the Context command
        chat.dispatch_command(SlashCommand::Context);
        
        let cells = drain_history_cells(&mut rx);
        let content = lines_to_string(&cells[0]);
        
        // Should handle gracefully
        assert!(content.contains("/context"), "Should show command");
        assert!(
            content.contains("No tokens") || content.contains("0"),
            "Should indicate no token usage"
        );
    }

    #[test]
    fn test_context_command_with_session_id() {
        let (mut chat, mut rx, _op_rx) = make_test_chatwidget();
        
        // Set a conversation ID
        let conv_id = ConversationId::new();
        chat.conversation_id = Some(conv_id.clone());
        
        // Dispatch the Context command
        chat.dispatch_command(SlashCommand::Context);
        
        let cells = drain_history_cells(&mut rx);
        let content = lines_to_string(&cells[0]);
        
        // Should show session info
        assert!(
            content.contains(&conv_id.to_string()) || content.contains("Session"),
            "Should show session information"
        );
    }

    #[test]
    fn test_context_command_available_during_task() {
        let (mut chat, mut rx, _op_rx) = make_test_chatwidget();
        
        // Start a task
        chat.bottom_pane.set_task_running(true);
        
        // Context command should still work
        chat.dispatch_command(SlashCommand::Context);
        
        let cells = drain_history_cells(&mut rx);
        assert!(!cells.is_empty(), "Context command should work during task");
        
        let content = lines_to_string(&cells[0]);
        assert!(content.contains("/context"), "Should show context output");
    }

    #[test]
    fn test_context_command_disabled_commands_during_task() {
        let (mut chat, mut rx, _op_rx) = make_test_chatwidget();
        
        // Start a task
        chat.bottom_pane.set_task_running(true);
        
        // Try a command that's disabled during tasks (e.g., Model)
        chat.dispatch_command(SlashCommand::Model);
        
        let cells = drain_history_cells(&mut rx);
        if !cells.is_empty() {
            let content = lines_to_string(&cells[0]);
            assert!(
                content.contains("disabled") || content.contains("not available"),
                "Should show error for disabled command"
            );
        }
        
        // Clear the receiver
        while rx.try_recv().is_ok() {}
        
        // But Context should still work
        chat.dispatch_command(SlashCommand::Context);
        let cells = drain_history_cells(&mut rx);
        assert!(!cells.is_empty(), "Context should work");
        let content = lines_to_string(&cells[0]);
        assert!(content.contains("/context"), "Should show context output");
    }

    #[test]
    fn test_context_command_formatting() {
        let (mut chat, mut rx, _op_rx) = make_test_chatwidget();
        
        // Set up realistic token usage
        chat.token_info = Some(TokenUsageInfo {
            model_context_window: Some(128000),
            total_token_usage: TokenUsage {
                input_tokens: 12345,
                output_tokens: 6789,
                total_tokens: 19134,
                cache_read_tokens: 1000,
                cache_write_tokens: 500,
            },
        });
        
        chat.conversation_id = Some(ConversationId::new());
        
        // Dispatch the Context command
        chat.dispatch_command(SlashCommand::Context);
        
        let cells = drain_history_cells(&mut rx);
        let content = lines_to_string(&cells[0]);
        
        // Check formatting elements
        assert!(content.contains("Context Window"), "Should have section header");
        assert!(content.contains("128"), "Should show context window size");
        assert!(content.contains("000"), "Should show full number");
    }

    #[test]
    fn test_context_vs_status_command_differences() {
        let (mut chat, mut rx, _op_rx) = make_test_chatwidget();
        
        // Set up token info
        chat.token_info = Some(TokenUsageInfo {
            model_context_window: Some(128000),
            total_token_usage: TokenUsage {
                input_tokens: 1000,
                output_tokens: 500,
                total_tokens: 1500,
                cache_read_tokens: 0,
                cache_write_tokens: 0,
            },
        });
        
        // Get Status output
        chat.dispatch_command(SlashCommand::Status);
        let status_cells = drain_history_cells(&mut rx);
        let status_content = lines_to_string(&status_cells[0]);
        
        // Get Context output
        chat.dispatch_command(SlashCommand::Context);
        let context_cells = drain_history_cells(&mut rx);
        let context_content = lines_to_string(&context_cells[0]);
        
        // Both should show token info but with different headers
        assert!(status_content.contains("/status"), "Status should have its header");
        assert!(context_content.contains("/context"), "Context should have its header");
        
        // Context should be more focused on token usage
        assert!(
            context_content.contains("Context Window") || context_content.contains("context window"),
            "Context command should emphasize context window"
        );
    }

    #[test]
    fn test_context_command_edge_cases() {
        let (mut chat, mut rx, _op_rx) = make_test_chatwidget();
        
        // Edge case: exactly 100% usage
        chat.token_info = Some(TokenUsageInfo {
            model_context_window: Some(128000),
            total_token_usage: TokenUsage {
                input_tokens: 100000,
                output_tokens: 28000,
                total_tokens: 128000,
                cache_read_tokens: 0,
                cache_write_tokens: 0,
            },
        });
        
        chat.dispatch_command(SlashCommand::Context);
        let cells = drain_history_cells(&mut rx);
        let content = lines_to_string(&cells[0]);
        
        assert!(content.contains("100"), "Should show 100% usage");
        
        // Edge case: over 100% usage (shouldn't happen but handle gracefully)
        chat.token_info = Some(TokenUsageInfo {
            model_context_window: Some(128000),
            total_token_usage: TokenUsage {
                input_tokens: 130000,
                output_tokens: 10000,
                total_tokens: 140000,
                cache_read_tokens: 0,
                cache_write_tokens: 0,
            },
        });
        
        chat.dispatch_command(SlashCommand::Context);
        let cells = drain_history_cells(&mut rx);
        let content = lines_to_string(&cells[0]);
        
        // Should handle over 100% gracefully
        assert!(content.contains("140000") || content.contains("109"), "Should show over-usage");
    }

    #[test]
    fn test_context_command_with_zero_cache_tokens() {
        let (mut chat, mut rx, _op_rx) = make_test_chatwidget();
        
        // Set up token info with zero cache tokens
        chat.token_info = Some(TokenUsageInfo {
            model_context_window: Some(128000),
            total_token_usage: TokenUsage {
                input_tokens: 1000,
                output_tokens: 500,
                total_tokens: 1500,
                cache_read_tokens: 0,
                cache_write_tokens: 0,
            },
        });
        
        chat.dispatch_command(SlashCommand::Context);
        let cells = drain_history_cells(&mut rx);
        let content = lines_to_string(&cells[0]);
        
        // Should either not show cache info or show it as 0
        // The implementation might choose to hide zero cache values
        if content.contains("cache") || content.contains("Cache") {
            assert!(content.contains("0"), "Should show 0 for cache if displayed");
        }
    }

    #[test]
    fn test_context_output_structure() {
        let (mut chat, mut rx, _op_rx) = make_test_chatwidget();
        
        // Set up comprehensive token info
        chat.token_info = Some(TokenUsageInfo {
            model_context_window: Some(128000),
            total_token_usage: TokenUsage {
                input_tokens: 50000,
                output_tokens: 25000,
                total_tokens: 75000,
                cache_read_tokens: 5000,
                cache_write_tokens: 2500,
            },
        });
        
        chat.conversation_id = Some(ConversationId::new());
        
        chat.dispatch_command(SlashCommand::Context);
        let cells = drain_history_cells(&mut rx);
        
        // Check that we got exactly one cell
        assert_eq!(cells.len(), 1, "Should emit exactly one history cell");
        
        let lines = &cells[0];
        
        // Should have multiple lines of output
        assert!(lines.len() > 3, "Should have multiple lines of output");
        
        // First line should be the command
        let first_line_text = lines[0]
            .spans
            .iter()
            .map(|s| s.content.as_ref())
            .collect::<String>();
        assert!(first_line_text.contains("context"), "First line should be command");
    }
}